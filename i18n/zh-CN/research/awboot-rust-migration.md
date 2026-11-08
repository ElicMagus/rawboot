# 将 `szemzoa/awboot`（全志 T113-S3，ARMv7-A）迁移到 Rust —— 一手来源可行性报告

调研：2026 年。目标：具体针对全志 T113-S3。所有论断均以内联方式引用到确切的一手来源
（源文件、官方文档、crate 元数据或第一方仓库）。无法对照一手来源验证的条目标记为
**UNVERIFIED**。

方括号中的参考编号对应文末的**来源**章节。

---

## 1. 执行摘要

- awboot 是一个小型裸机 C 引导加载程序，面向 T113-S3/T113-S4/V851s，使用
  `arm-none-eabi` 工具链、`-mcpu=cortex-a7`、硬浮点、`-Os` 构建，产出四个 FEL/SPI/SD/eMMC
  镜像，每个都封装在全志 "eGON" BROM 头中 [1][2]。
- 启动链为：BROM → 从 SD 扇区 16（8 KB）加载一个很小的 stage；或在更新的 SoC（明确包括
  T113）上作为回退，从扇区 256（128 KB）加载；也可从 SPI NOR/NAND 或通过 USB FEL 加载；
  BROM 会校验 eGON 头校验和（种子 `0x5F0A6C39`）[1][4][40]。awboot 直接从 `start.S` 生成
  eGON 头，仅用 `mksunxi` 重新计算校验和、修正长度并填充镜像 [3][4]。
- ARMv7-A 上的 Rust 裸机**在 stable 上**可行：`armv7a-none-eabi` 和 `armv7a-none-eabihf`
  是 Tier-2 `no_std` 目标，存在于 rustc 1.99.0（2026-09-28）中，并可通过 rustup 安装
  [20][21]。**不存在** `armv7a-unknown-none-eabi` 目标；该名称并不存在 [21]。
  `rust-embedded/aarch32` 项目提供用于启动/异常向量的 `aarch32-rt` 0.5.0，以及用于 CPU
  支持的 `aarch32-cpu` 0.4.0（MIT/Apache-2.0，MSRV 1.93）[22][23]。
- **目前没有针对 ARMv7-A 全志 T113 的现有 Rust 项目。** 全志 Rust 生态完全属于 RISC-V
  （D1/D1s）或仅有 PAC：`rustsbi-d1` 和 oreboot 的全志移植都是 T-Head C906
  **RISC-V** [24][25][28]；`smaeul/sun20i_d1_spl` 是 C [42]。oreboot 仅在搁置的 QEMU
  仿真中支持 ARMv7-A [28]。
- "简单"外设和文件系统已有可复用的 Rust crate：`fatfs` 0.3.6（no_std，MIT）[31][32]、
  `embedded-sdmmc` 0.10.0（no_std，SD/MMC + FAT，MIT/Apache）[36]，以及 `fdt` crate
  （no_std，MPL-2.0）[34][35]。对于 Synopsys DesignWare MSHC（即 `sunxi_sdhci` IP），
  `dwmmc-host`（Apache-2.0，no_std，FIFO/IDMAC）和 `sdhci-host`（ADMA2）是较新的、直接
  相关的 `sdmmc-protocol` 后端 [37][38]。
- **主要风险是 DRAM 控制器初始化**（`dram.c`，约 40 KB）。它是无文档的寄存器/时序代码，
  自述源自反编译，其算法（自动 rank/位宽与容量检测、ZQ、DQS gating、PHY AC 重映射）无法
  简单直译 [14]。U-Boot mainline 带有经过重写的相同代码 [41]。务实的建议是首次移植时把
  这一个文件（以及 secure-monitor/PSCI 汇编）保留为 C，通过 FFI 调用，其余全部移植。

### 可行性结论

**可行，采用分阶段的 C/Rust 混合方案。** CPU/ABI/工具链问题已解决，外设驱动与 FAT/FDT
部分基本可复用，风险集中在两个孤立且有明确边界的模块（DRAM init；secure-monitor/PSCI）。
完整的"100% Rust"重写可行，但由于 DRAM 微码和 secure 状态下的 GIC/CPUCFG 操作，代价过高。

---

## 2. T113-S3 启动链

### 2.1 SoC 与工具链

- awboot 面向 "Allwinner T113-s3, T113-s4, V851s" [1]。其构建标志为
  `-mcpu=cortex-a7 -mthumb-interwork -mthumb -mno-unaligned-access -mfpu=neon-vfpv4
  -mfloat-abi=hard`，即带 NEON/VFPv4 和硬浮点 ABI 的 ARMv7-A Cortex-A7 [2]。
  （T113-S3 数据手册本身未直接获取；Cortex-A7 的判定基于 awboot 自身的 `-mcpu=cortex-a7`
  以及共享的 R528/T113 DRAM 代码——见 §4——并且**未对照厂商数据手册验证**。）
- 机器树位于 `arch/arm32/mach-t113s3/` [19]。另有独立的 `mach-t113s4` 和 `mach-v851s`
  树，共享完全相同的 `sdmmc.h`、`dram.h` 和 `memcpy.S` 二进制（由树列表中相同的 git blob
  SHA 确认）[19]。

### 2.2 FEL / USB

- FEL 启动用 `xfel` 将 awboot 上传到 FEL 加载地址并跳转；`fel.sh` 设置
  `BOOT_ADDR=0x00028000`，运行 `xfel ddr t113-s3`，将内核写到 `0x42000000`、DTB 写到
  128 MiB SDRAM 顶端稍下方，以及可选的 initrd，然后 `xfel exec` [5]。
- awboot 的 FEL 镜像链接在 `__RAM_BASE=0x00028000`（128 KB SRAM 限制），而持久启动镜像
  链接在 `0x00020000`（启动模式下 160 KB）[2][6]。`link.ld` 声明一个 128 KB SRAM 区域
  以及位于 `0x40000000` 的 128 MiB DDR 区域 [6]。
- 内核/DTB/initrd 的交接使用位于 `0x43100000` 的四个 "mailbox"：initrd 大小（+0x0）、
  initrd 起始（+0x4）、DTB 地址（+0x8）、内核地址（+0xC）。`fel.sh` 用 `xfel write32`
  写入它们，`main.c` 在 `apply_fel_mailboxes()` 中读回 [5][8][7]。
- `xfel` 是一个 C 程序（仓库语言 "C"，MIT），支持 T113 以及用于 SPI 烧录的
  `spinor`/`spinand` 命令 [29][30]。

### 2.3 eGON 头（awboot 实际编码的内容）

- awboot 的 `start.S` **自行生成 eGON 头**，作为 `.text` 的首字节：ARM 分支字
  `0xea000016`、ASCII 魔数 `eGON.BT0`、占位校验和 `0x12345678`、SPL 大小符号
  `__spl_size`、启动头大小 `0x30`、启动头版本字 `0x30303033`（"3030"/"300"）、
  返回值 `0x00020000`、运行地址 `0x00028000`，以及平台字符串字节 `34 2e 30 00`
  （"4.0\0"）[3]。
- `tools/mksunxi.c` 定义了相同的 56 字节 `boot_head_t` 布局（`instruction`、`magic[8]`、
  `checksum`、`length`、`spl_signature[4]`、`fel_script_address`、`fel_uenv_length`、
  `dt_name_offset`、`reserved1`、`boot_media`、`string_pool[13]`）[4]。
- `mksunxi` 仅做：读取二进制，将 `length` 对齐到填充（块设备 512，flash 8192），以
  `0x5F0A6C39` 为校验和种子，在对齐后的长度上累加所有 32 位字，将校验和写回，并
  （可选地）将每个 2 KB 页扩展为 `pagesize`（对于大页 SPI flash，BROM 假定 2 KB 页）[4]。
- Makefile 对 FEL/SD/eMMC 调用 `mksunxi <bin> 512`，对 SPI 变体调用 `mksunxi <bin> 8192`
  并加 `8192 4096` [2]。

### 2.4 SD 卡与 SPI 布局

- awboot README：SD 启动用 `dd ... bs=1024 seek=8` 写入 `awboot-boot-sd.bin` —— MBR 偏移
  8 KB（扇区 16）——或对 GPT 使用 128 KB（扇区 256）[1]。
- linux-sunxi wiki 确认了机制：8 KB 偏移 "由 BROM 决定，它会在此位置检查有效的 eGON/TOC0
  头"，并且 "较新的 SoC（在 H2+、A64、H5、H6、T113 上测试）也可以从扇区 256（128KB）
  加载 SPL……如果在 8KB 处未找到有效的 eGON/TOC0 签名" [40]。这是对 awboot 8 KB/128 KB
  说法的首要来源佐证。
- SPI：SPI 镜像被写入三次（页 0、32 和 64，以 2 KB 为步长），连同第二次 `mksunxi` 调用
  生成的 `4k` 页大小变体，以容忍 BROM 的 2 KB 页假设和 flash 几何结构 [2][4]。
- BROM 的通用启动顺序（较老 sunxi 为 SD0 → NAND → SD2 → SPI → FEL，并有 FEL/BSP 引脚
  覆盖）记录在 linux-sunxi [39]；确切的 T113 顺序在 T113 专用的首要来源中**未验证**，
  但 T113 支持 SD/SPI/FEL 启动 [40]。
- 使用的内核源码是 `smaeul/linux` 的 `d1/all` 分支 [1][45]。

### 2.5 awboot 运行时启动流程

`main()` 依次执行：`sunxi_clk_init()`、`board_init()`（USART0 上的 UART，PE2/PE3 mux 6）、
`sunxi_dram_init()`、`sunxi_wdg_set(10)`，然后要么进行 SDHCI→SD/MMC→FatFs 加载，要么进行
SPI-NAND 加载，接着 `boot_image_setup()`（校验 Linux zImage 魔数并计算入口），然后修改 FDT
（bootargs、`/memory` reg、initrd 起始/结束），再禁用 MMU/D-cache/I-cache/中断并调用
`boot_linux_psci()` [7][8][9]。`boot_linux_psci()` 运行 `armv7_init_nonsec()`，然后以
r0..r2 中的内核入口执行 `_do_nonsec_entry` 跳板 [7][12]。

---

## 3. 2026 年 ARMv7-A Cortex-A7 上的 Rust 裸机（no_std）

### 3.1 rustc 目标与 tier 状态

- `armv7a-none-eabi` 和 `armv7a-none-eabihf`（以及 `thumbv7a-none-eabi{,hf}` 变体）在
  rustc book 的同一页上记录为 **Tier 2**、core+alloc、裸机 `#![no_std]`；`-eabi` 为软浮点，
  `-eabihf` 为硬浮点（最低需要 VFPv3-D16）[20]。
- 在本地工具链 `rustc 1.99.0 (b940084d7 2026-09-28)` 上验证：`rustc --print target-list`
  包含 `armv7a-none-eabi` 和 `armv7a-none-eabihf`；`rustup target list` 提供两者以供安装
  [21]。
- 名称 `armv7a-unknown-none-eabi` **不存在**，也不是有效目标 [21]。正确的裸机名称没有
  `unknown` 厂商部分。
- Tier 2 意味着 rustup 分发预编译的 `core`/`alloc`（以及适用时的 `std`），但没有宿主工具；
  Cortex-A7 **不需要**自定义目标 JSON（只有当你想要非随附的 CPU 特性默认值时才需要 JSON）。
  `+vfp4`、`+d32`、`+neon` 等目标特性通过 `-C target-feature=` 或 `.cargo/config.toml`
  启用 [20]。
- `asm!`/`global_asm!` 已稳定；启动代码不需要 nightly 特性。（只有特殊的
  `#![feature(...)]` 设置才需要 nightly。）

### 3.2 启动、链接脚本、cache/MMU

- awboot 的启动工作——向量基址（`VBAR`）、端序、禁用缓存/MMU、TLB/无效化、启用 VFP/NEON、
  栈、清零 BSS、`CNTFRQ=24 MHz`、跳转到 `main`——都是普通的 ARMv7-A 汇编，可用 Rust 的
  `global_asm!` 或 `aarch32-rt` runtime 表达 [3][23]。
- `rust-embedded/aarch32` 项目维护 `aarch32-rt` 0.5.0（runtime/启动，特性 `eabi-fpu`、
  `el2-mode`、`fpu-d32`、`svc-stack-interrupt`）和 `aarch32-cpu` 0.4.0（CPU/寄存器支持），
  两者均为 MIT/Apache-2.0、MSRV 1.93，于 2025-10-31 首次发布，并在 2026-09 期间持续更新
  [22][23]。它们是移植中 CR0/CR1 协处理器和异常向量处理的推荐基础。
- 链接脚本可以直接移植；它就是普通的 GNU ld（`OUTPUT_FORMAT("elf32-littlearm")`、
  `OUTPUT_ARCH(arm)`、MEMORY/SECTIONS，`__spl_size` 计算为 `__spl_end-__spl_start`）[6]。

---

## 4. 已有工作与可复用 crate

### 4.1 已有工作——存在什么，以及为什么都不是 T113 引导加载程序

| 项目 | 架构 | 功能 | 许可证 | 对 T113-S3 的可复用性 |
|---|---|---|---|---|
| `rustsbi/rustsbi-d1` | **RISC-V**（T-Head C906） | 面向 D1 板卡的 SBI 固件/SPL；使用 riscv hart/PLIC/trap CSR 以及一个 `magic.rs` DRAM blob | 仓库元数据未声明许可证 | 对 ARMv7-A **不适用**：`see/` 和 `spl/` crate 是 RISC-V（`riscv_spec.rs`、`hart_csr_utils.rs`、`plic.rs`）[24][25] |
| `rustsbi/rustsbi-allwinner-nezha` | RISC-V | "Bootloader environment for Allwinner Nezha ... XuanTie C906 RISC-V core" | 无；仓库**已归档且为空**（大小 0）[26] | 不适用（RISC-V，已死） |
| `oreboot` | x86、QEMU 中的 RISC-V 与 ARM | "coreboot without C"、LinuxBoot payload；全志 D1 移植为 RISC-V；ARMv7-A 被搁置（`src.broken` 中的 `qemu-armv7`） | GPL-2.0 | 没有 ARM sunxi/T113 主板；仅有架构模式和 DWMMC 知识 [27][28] |
| `smaeul/sun20i_d1_spl` | RISC-V | "Mainline-friendly SPL for D1" | 无 | C、RISC-V；不可复用 [42] |
| `xboot/xfel` | 宿主工具 | 支持 T113 的 FEL/flash 工具 | MIT | C；可直接使用，不是 Rust 移植目标 [29][30] |
| `Razican/aw-fel` + `fel-cli` | 宿主工具 | 纯 Rust FEL 库/CLI | — | 最后发布 0.5.2，2018 年；**T113 支持未验证** [43] |
| `duskmoon314/aw-pac` | D1/R528/V853 | 由 SVD 生成的外设访问 crate（`d1-pac`、`r528-pac`、`v853-pac`） | — | R528 是 T113 的 ARM 兄弟；寄存器偏移或许可迁移，但 PAC 面向 D1/RISC-V —— ARMv7-A 可用性**未验证** [43] |

结论：具体到 T113-S3，**没有先前的 Rust 引导加载程序**；移植需从 SoC 特定部分的第一性原理
开始。

### 4.2 可复用 crate（已验证的元数据）

- **CPU/runtime：** `aarch32-cpu` 0.4.0 和 `aarch32-rt` 0.5.0（MIT/Apache-2.0，MSRV 1.93）
  [22][23]。
- **FAT：** `fatfs` 0.3.6（MIT，通过 `default-features = false` 使用 no_std；最后发布于
  2023-01-17）[31][32]。注意：crates.io 上的 `embedded-fatfs` **不是**真正的 crate——它是
  一个 2023 年发布的 3 行 `0.0.0` 名称抢注占位符 [33]。活跃的嵌入式 FAT 项目就是 `fatfs`
  crate 本身。
- **SD/MMC + FAT：** `embedded-sdmmc` 0.10.0（MIT/Apache-2.0，no_std，MSRV 1.87），在
  `BlockDevice` trait 背后打包了 SD/MMC 协议驱动和 FAT 文件系统 [36]。
- **FDT：** `fdt` 0.1.5 稳定版 / 0.2.0-alpha3（MPL-2.0，纯 `#![no_std]`，2026-10 仍在
  积极维护）[34][35]。读与写的注意事项见 §7。
- **DWMSHC/SDHCI：** `dwmmc-host` 0.4.2（Apache-2.0，no_std，"FIFO/IDMAC"，
  `sdmmc-protocol` 的 DesignWare Mobile Storage Host Controller 后端）和 `sdhci-host`
  0.5.2（Apache-2.0，no_std，FIFO/ADMA2），均来自 `rcore-os/tgoskits`，edition 2024，
  发布/更新至 2026-09 [37][38]。sunxi `sunxi_sdhci` 寄存器映射（IDMAC
  `dmac/dlba/idst/idie`、偏移 0x200 的 FIFO、偏移 0x300 的版本）就是 DesignWare MSHC
  布局，因此 `dwmmc-host` 直接相关——但它面向 Rust OS/`no_std` 上下文，且不包含 sunxi
  时钟/PHY 的特殊处理 [15][37]。
- **FEL 工具：** `aw-fel`/`fel-cli` 是 Rust，但已过时（2018 年）[43]。

---

## 5. 逐领域分析

### 5.1 DRAM init（`dram.c`，约 40 KB）——**最高风险**

- `dram.c` 为 40,623 字节 [19]。其文件头声明它是"一个重写版本"，其"基线……取自 awboot，
  后者似乎基于某种对原始全志代码片段的反编译"，并覆盖"Allwinner D1/D1s/R528/T113-sx
  DRAM 初始化"[14]。
- 它**不是**扁平的微码表：它实现算法——`auto_scan_dram_rank_width`、
  `auto_scan_dram_size`、`dqs_gate_detect`、`mctl_set_timing_params`（来自 `ns_to_t` 的
  DDR2/DDR3/LPDDR2/LPDDR3 时序）、`mctl_phy_ac_remapping`（由 fuse/chip-id 选择的 22 行
  重映射表）、`mctl_channel_init`（PIR/PGSR 握手）、`ccu_set_pll_ddr_clk`、ZQ 校准，
  以及读写大小测试 [14]。这是无文档、对时序敏感的寄存器代码。
- 相同逻辑也在 U-Boot mainline 的 `drivers/ram/sunxi/dram_sun20i_d1.c` 中，明确归功于
  awboot，`sunxi_dram_init()` 既可独立使用，也可通过驱动模型使用 [41]。对移植而言，这是
  比 awboot 变体更清晰的参考，并且它确认该代码是可移植的 C，不依赖 ARM/RISC-V 核心
  （该控制器在 RISC-V D1 与 ARM R528/T113 之间共享）。
- **可移植性：** 该代码是自包含的 C，使用 `read32/write32`/`udelay` shim；除
  `abort`/无 `memcpy` 的算术外，没有线程、分配或 libc [14]。可以原样复用。
- **Rust 先例：** 未发现 Allwinner DDR3 init 的 Rust 实现（对 crates.io 和全志包空间的
  搜索只返回 RISC-V SVD PAC 和 FEL 工具）[43]。
- **Rust 中的 C 过渡：** 完全现实。将 `dram.c`（及其 `reg-*.h` 兄弟文件）编译为普通 C
  目标文件/`staticlib`，暴露 `extern "C"` 的 `sunxi_dram_init()`/`dram_get_top()`；
  Rust 在安全包装后调用。这是最有价值的"保留在 C"候选。

### 5.2 SDHCI（`sunxi_sdhci.c`，28.9 KB）

- 寄存器布局是 Synopsys DesignWare MSHC：`gctrl/clkcr/timeout/width/blksz/bytecnt/cmd/arg/
  resp*`、IDMAC 块（`dmac/dlba/idst/idie`）、偏移 `0x200` 处的 FIFO、`0x300` 处的版本，
  外加 sunxi 专用延迟寄存器（`drv_dl`、`samp_dl`、skew 寄存器）[15]。
- 驱动在 `0x04020000` 实例化 `sdhci0`（SD/MMC，PF 引脚），在 `0x04022000` 实例化
  `sdhci2`（eMMC，PC 引脚），并带有 `sdhci_idma_desc_t` 描述符环 [9][15]。
- **可复用 Rust：** `dwmmc-host` 是支持 FIFO/IDMAC 的 no_std DWMSHC 后端，配有
  `sdmmc-protocol` 高层；`sdhci-host` 是 ADMA2 兄弟。`embedded-sdmmc` 还提供带
  `BlockDevice` 抽象的 SD/MMC 层 + FAT [37][38][36]。集成工作是**中等**：sunxi
  时钟/电压/时序表（`odly/sdly`、`set_clock`、`set_voltage`）以及 eMMC 硬件复位
  （`hwrst`）和 pinmux 仍需 SoC 粘合代码。

### 5.3 SPI NOR / SPI NAND（`sunxi_spi.c`，23.9 KB）

- awboot 的 SPI 路径检测并读取 SPI NAND（`spi_nand_detect`、`spi_nand_read`），使用
  **固定的 flash 偏移**而非文件系统：DTB 在 `128*2048`，内核在 `256*2048` [17][8]。
- **可复用 Rust：** SPI-NOR 只有小型/实验性 crate（`norbert`、`rflasher`），没有针对
  sunxi SPI 控制器或 SPI-NAND 的 [43]。预计需要**中到高**工作量的从零驱动，或初期保留在
  C。它仅 `spi` 变体需要；对于 T113-S3 的 SD/eMMC 启动可以推迟。

### 5.4 DMA（`sunxi_dma.c`，7.4 KB）

- awboot 有一个小型 DMA 引擎，供 SPI-NAND 路径使用（`dma_init`、`dma_test`）[7]。不存在
  专门的 sunxi DMA Rust crate [43]。**低到中等**工作量，但仅在移植 SPI-NAND 时才需要。

### 5.5 CCU 时钟（`sunxi_clk.c`，7.5 KB）、GPIO（`sunxi_gpio.c`，2.7 KB）

- `sunxi_clk_init()` 和 `sunxi_clk_get_fail_addr()` 在 `main()` 中最先运行；GPIO 处理
  UART、MMC、SPI 和板载 LED 的 pinmux/pull [7][8][9]。寄存器定义位于 `reg-ccu.h`
  （12.4 KB）和 `reg-r-ccu.h`。
- 没有 sunxi CCU/GPIO 的 Rust crate；R528/D1 PAC 是最近的寄存器参考，但面向 RISC-V [43]。
  这些是**低工作量**（主要是寄存器写入），是很好的早期 Rust 移植目标。注意 DRAM 代码
  内部会触及 CCU（DDR PLL、MBUS/DRAM 门控），因此 CCU 与 DRAM 耦合 [14]。

### 5.6 UART（`sunxi_usart.c`，2.3 KB）、watchdog（`sunxi_wdg.c`，2.8 KB）、定时器（`arch_timer.c`）

- UART 是在 USART0 上以 115200 进行的简单轮询初始化（USART3/5 上也有替代）[9]；watchdog
  是简单的 `sunxi_wdg_set(seconds)` [7]。`arch_timer.c`（1.3 KB）使用 ARM 通用定时器和
  `CNTFRQ`。这些是**风险最低**的模块，也是天然的首次 Rust 移植（UART 能立即提供调试
  输出）。`synopsys-dw-uart` 存在，但 sunxi UART 是简单的 DesignWare APB UART，且
  awboot 自己的驱动很小，因此直接移植很容易 [43]。`aarch32-cpu` 提供通用定时器/CP15
  原语 [22]。

### 5.7 FDT（`lib/fdt.c`，17.3 KB）——读**与写**

- awboot 的 FDT 代码不只是解析器：它就地修改 blob——`fdt_update_bootargs`
  （`/chosen:bootargs`）、`fdt_update_initrd`（`linux,initrd-start/end`，含 cells 处理）、
  `fdt_update_memory`（`/memory:reg`、`device_type`），包括插入新属性以及增长/移动
  `dt_struct`/`dt_strings`（`of_blob_move_dt_struct`、`of_add_property`）[16]。
- Rust `fdt` crate 明确"用于解析 Flattened Devicetrees"；其 README 演示只读访问
  （`model`、`compatible`、`memory`、`chosen.bootargs`、`find_node`），并未宣传可修改
  [35]。因此**只读 `fdt` 移植会留下缺口**：awboot 必须在启动前*写入* bootargs、内存大小
  和 initrd 范围。选项：(a) 使用面向写入器的 crate（`vm-fdt` "crate for writing FDT
  blobs" [43]）；(b) 手写 FDT 修改（有界，约几百行）；或 (c) 通过 FFI 保留 `lib/fdt.c`。
  这是**中等**工作量，且有许可证注意事项：`lib/fdt.c` 带有 Atmel BSD 声明 [16]，而
  `fdt` crate 是 MPL-2.0 [34]。

### 5.8 FAT 文件系统

- awboot 在 `f_read`/`f_lseek(CREATE_LINKMAP)` 流式适配器上使用 ChaN FatFs
  （`lib/fatfs/ff.c` 为 259,696 字节；`ffunicode.c` 为 1,980,820 字节；`ffconf.h`
  12 KB），读取块为 32 KB [17][19]。这是迄今为止最大的源码组件。
- Rust 替代品 `fatfs` 0.3.6 是 no_std、MIT，支持 FAT12/16/32 + LFN，是事实上的嵌入式
  Rust FAT 库 [31][32]。集成需要一个 `Read/Write/Seek` 块设备适配器（或使用已经耦合
  SD/MMC 和 FAT 的 `embedded-sdmmc`）[36][32]。**低到中等**工作量，相比 2 MB 生成的
  `ffunicode.c` 是明显的胜利。

### 5.9 PSCI / secure-to-nonsecure / 异常向量——**高风险**

- awboot 的 PSCI 取自 U-Boot/Allwinner：`psci.c`（8 KB）实现 `psci_cpu_on/off`、
  `psci_fiq_enter`（monitor 模式下的 GIC FIQ）、`psci_arch_init`（GICD/GICC
  group/priority/FIQEn、SCR），以及 R528 CPUCFG 路径（软入口 `0x1c8`、C0 reset/status、
  PRCM 电源门控）[10]。
- monitor 模式切换在汇编中：`nonsec_virt.S`（5.4 KB）设置 `_monitor_vectors`，处理
  `smc #0`，编程 SCR（NS/AW/FW 位），可选进入 HYP 模式（`CONFIG_ARMV7_VIRT`），重置
  `CNTVOFF`，设置 `MVBAR`，并在 `_nonsec_init` 中完成 GIC/NSACR/CNTFRQ 设置；
  `_do_nonsec_entry` 是入口跳板 [11]。`armv7_nonsec.c` 驱动它（通过 ID_PFR1 检查 SEC
  扩展、GICD 使能、secure-section 重定位）[12]。`psci_vectors.S`（8.5 KB）保存 monitor
  向量和异常保存/恢复；`start.S` 保存 IRQ/FIQ/abort 向量表和 `save_regs`/`restore_regs`
  宏 [13][3]。
- Rust 可以表达这些：monitor 向量/保存恢复用 `global_asm!`，SCR/SMC/MVBAR 操作用
  `asm!`。`aarch32-cpu` 提供许多寄存器原语 [22]。但要注意 awboot 将 secure 代码放在
  重定位的 `._secure.text`/`._secure.data` 段（`relocate_secure_section`）中，并将
  PSCI 栈放在专用的 `NOLOAD .psci_stack` 区域 [6][12]。在 Rust 中正确复现段放置和
  重定位很微妙。异常*stub* 则很简单（`exception.c`，XBOOT MIT，仅打印寄存器并停机）
  [13]。

### 5.10 启动链、eGON 头与工具

- eGON 头可以由 Rust `global_asm!` 完全像 `start.S` 那样生成，或由 Rust 宿主工具生成。
  `mksunxi.c` 约 3.5 KB，其工作（在对齐长度上以 `0x5F0A6C39` 计算校验和、填充
  512/8192、可选的 2 KB→page 扩展）很容易用 Rust 的 `build.rs`/xtask 重新实现 [3][4]。
- `xfel` 仍是实用的 FEL/flash 工具，且是 C/MIT；T113 没有完整的 Rust 等价物（2018 年的
  `aw-fel` 早于 T113）[29][30][43]。工具应被**移植或包装**，不能想当然。
- awboot 自身的许可证**不明确**：GitHub 仓库未报告许可证，而各个文件带有混合声明
  （`dram.c` 中为 GPL-2.0+，`lib/fdt.c` 中为 BSD，`exception.c` 中为 MIT）
  [18][14][16][13]。在分发式 Rust 移植中进行整体代码复用前，必须解决这一点。

---

## 6. 风险与工作量排序

| 领域 | 工作量 | 风险 | 备注 / 复用 |
|---|---|---|---|
| UART、GPIO、WDG、通用定时器、异常 stub | S | 低 | 寄存器简单；先移植用于 bring-up [9][13] |
| eGON 头 + `mksunxi` + 构建/工具 | S | 低 | Rust 汇编/xtask；xfel 保持外部 [3][4][29] |
| CCU 时钟（非 DRAM） | S–M | 低–中 | 寄存器写入；与 DRAM PLL 耦合 [8][14] |
| FAT（替换 ChaN FatFs，2 MB） | M | 低 | `fatfs` / `embedded-sdmmc` [31][36] |
| FDT（替换 `lib/fdt.c`） | M | 中 | `fdt` 解析；**写入** bootargs/initrd/memory 需谨慎 [16][35] |
| SDHCI / DWMSHC + SD/MMC | M | 中 | `dwmmc-host`/`sdmmc-protocol`/`embedded-sdmmc` [15][37][36] |
| SPI NOR/NAND（+ DMA） | M–L | 中–高 | 没有成熟的 sunxi Rust 驱动；除非需要 SPI 启动，否则推迟 [17][43] |
| 启动汇编 / 链接器 / cache-MMU | M | 中 | `aarch32-rt`；移植 link.ld [6][23] |
| PSCI / secure monitor / 重定位 | L | 高 | 汇编 + secure section 重定位 [10][11][12] |
| **DRAM init（`dram.c`，约 40 KB）** | **L** | **最高** | 无文档的时序代码；通过 FFI 保留在 C，或从 U-Boot 移植 [14][41] |

---

## 7. 推荐的增量迁移策略

1. **工具链 spike（数天）。** 为 `armv7a-none-eabi`/`eabihf` 构建一个链接到
   `0x00020000` 的 `no_std` Rust blob，通过 `mksunxi` + `xfel` 驱动它，并翻转一个
   GPIO / 在 USART0 上打印。这可在任何真正移植前验证目标、链接器和 eGON 集成
   [2][3][6][20][21]。
2. **先移植叶子驱动（UART、GPIO、WDG、timer）。** 它们很小并能提供可观察的输出；使用
   `aarch32-cpu`/`aarch32-rt` 处理 CPU/异常管道 [22][23][9][13]。
3. **将 DRAM 和 secure-monitor 通过 FFI 保留在 C。** 将 `dram.c` + `reg-*.h` 和
   `nonsec_virt.S`/`psci_vectors.S` 编译为目标文件；暴露 `extern "C"` 的
   `sunxi_dram_init()`、`dram_get_top()`、`boot_linux_psci()`。Rust 拥有控制流；C 拥有
   两个高风险模块。这能立即降低项目风险 [14][11][41]。
4. **移植启动流程 + eGON + FDT + FAT。** 将 `main.c` 的编排迁移到 Rust；使用 `fatfs`
   或 `embedded-sdmmc` crate 处理 FAT；使用 `fdt` 解析，并用一个小型写入器（或保留
   `lib/fdt.c`）进行修改。用 Rust 重新实现 `mksunxi`（很小），但保持 `xfel` 外部
   [16][31][36][35][4]。
5. **移植 SDHCI/卡协议**，使用 `dwmmc-host`/`sdmmc-protocol`/`embedded-sdmmc` 加上
   sunxi 时钟/电压/pinmux 粘合 [15][37][36]。
6. **然后移植 DRAM**（对全 Rust 目标而言优先级最低），以 U-Boot 清理后的
   `dram_sun20i_d1.c` 为参考，将其视为逐字节行为移植，并进行硬件在环回归测试 [41]。
7. **最后将 PSCI/secure monitor 移植**到 `global_asm!`，仔细复现重定位的 secure section
   和 monitor 向量表 [10][11][12][6]。

任何全 Rust 终态都应先解决**许可证**问题，因为 awboot 混合了 GPL-2.0+、BSD 和 MIT 声明，
且没有顶层 LICENSE [18][14][16][13]。

---

## 8. 面向用户的开放问题

1. **范围：** 目标是*完整*的 Rust 重写，还是保留 DRAM 控制器（可能还有 monitor 模式汇编）
   在 C 中的 Rust/C 混合？（强烈推荐混合方案。）
2. **启动介质优先级：** 必须先交付哪条路径——SD 卡（SDHCI+FAT）、eMMC、SPI NOR/NAND，
   还是 FEL？这会完全重排 SPI/DMA 工作的顺序。
3. **用户现有的移植：** 它使用哪个旧 awboot 修订版和哪个 rustc edition/目标？是
   `armv7a-none-eabi` 还是 `eabihf`？已经能编译什么/？（用于避免重复工作并审计任何复制
   代码的许可证。）
4. **许可证：** Rust 移植将采用什么许可证？这限制了对 GPL-2.0+ `dram.c` 以及 MPL-2.0
   `fdt` / Apache-2.0 `dwmmc-host` 的复用。
5. **T113-S3 vs T113-S4 vs V851s：** 保留全部三个机器树，还是仅 T113-S3（如所述）？
6. **硬件在环：** 是否有 T113-S3 板卡可用于持续的 DRAM/PSCI 回归测试？（DRAM 移植的
   必要条件。）
7. **目标特性集：** 确认是否要求 Cortex-A7 + NEON/VFPv4 + 硬浮点 parity，或者软浮点
   （`-eabi`）对引导加载程序是否可接受。
8. **工具：** 可以保留 `xfel`（C）作为 FEL/flash 宿主工具，还是所有宿主工具都必须用
   Rust？

---

## 来源

1. awboot README —— https://raw.githubusercontent.com/szemzoa/awboot/main/README.md
2. awboot Makefile —— https://raw.githubusercontent.com/szemzoa/awboot/main/Makefile
3. awboot `start.S` —— https://raw.githubusercontent.com/szemzoa/awboot/main/arch/arm32/mach-t113s3/start.S
4. awboot `tools/mksunxi.c` —— https://raw.githubusercontent.com/szemzoa/awboot/main/tools/mksunxi.c
5. awboot `tools/fel.sh` —— https://raw.githubusercontent.com/szemzoa/awboot/main/tools/fel.sh
6. awboot `link.ld` —— https://raw.githubusercontent.com/szemzoa/awboot/main/arch/arm32/mach-t113s3/link.ld
7. awboot `main.c` —— https://raw.githubusercontent.com/szemzoa/awboot/main/main.c
8. awboot `board.h` —— https://raw.githubusercontent.com/szemzoa/awboot/main/board.h
9. awboot `board.c` —— https://raw.githubusercontent.com/szemzoa/awboot/main/board.c
10. awboot `psci.c` —— https://raw.githubusercontent.com/szemzoa/awboot/main/arch/arm32/mach-t113s3/psci.c
11. awboot `nonsec_virt.S` —— https://raw.githubusercontent.com/szemzoa/awboot/main/arch/arm32/mach-t113s3/nonsec_virt.S
12. awboot `armv7_nonsec.c` —— https://raw.githubusercontent.com/szemzoa/awboot/main/arch/arm32/mach-t113s3/armv7_nonsec.c
13. awboot `exception.c` —— https://raw.githubusercontent.com/szemzoa/awboot/main/arch/arm32/mach-t113s3/exception.c
14. awboot `dram.c` —— https://raw.githubusercontent.com/szemzoa/awboot/main/arch/arm32/mach-t113s3/dram.c
15. awboot `sunxi_sdhci.h` —— https://raw.githubusercontent.com/szemzoa/awboot/main/arch/arm32/mach-t113s3/sunxi_sdhci.h
16. awboot `lib/fdt.c` —— https://raw.githubusercontent.com/szemzoa/awboot/main/lib/fdt.c
17. awboot `lib/loaders.c` —— https://raw.githubusercontent.com/szemzoa/awboot/main/lib/loaders.c
18. awboot 仓库元数据（无许可证）—— https://api.github.com/repos/szemzoa/awboot
19. awboot 完整树（文件大小）—— https://api.github.com/repos/szemzoa/awboot/git/trees/main?recursive=1
20. rustc book：`{arm,thumb}v7a-none-eabi{,hf}`（Tier 2）—— https://doc.rust-lang.org/rustc/platform-support/armv7a-none-eabi.html
21. 本地工具链 `rustc 1.99.0 (b940084d7 2026-09-28)`：`rustc --print target-list`、`rustup target list`
22. `aarch32-cpu` crate 元数据 —— https://crates.io/api/v1/crates/aarch32-cpu
23. `aarch32-rt` crate 元数据 —— https://crates.io/api/v1/crates/aarch32-rt
24. `rustsbi/rustsbi-d1` 仓库 —— https://api.github.com/repos/rustsbi/rustsbi-d1
25. `rustsbi/rustsbi-d1` 树（RISC-V `see/`、`spl/`）—— https://api.github.com/repos/rustsbi/rustsbi-d1/git/trees/main?recursive=1
26. `rustsbi/rustsbi-allwinner-nezha` 仓库（已归档，为空）—— https://api.github.com/repos/rustsbi/rustsbi-allwinner-nezha
27. `oreboot` 仓库 —— https://api.github.com/repos/oreboot/oreboot
28. `oreboot` README（D1 RISC-V，搁置的 `qemu-armv7`）—— https://raw.githubusercontent.com/oreboot/oreboot/main/README.md
29. `xboot/xfel` 仓库（C，MIT）—— https://api.github.com/repos/xboot/xfel
30. `xfel` README —— https://raw.githubusercontent.com/xboot/xfel/master/README.md
31. `fatfs` crate 元数据 —— https://crates.io/api/v1/crates/fatfs
32. `rust-fatfs` README（no_std 用法，MIT）—— https://raw.githubusercontent.com/rafalh/rust-fatfs/master/README.md
33. `embedded-fatfs` crate 元数据（0.0.0 占位符）—— https://crates.io/api/v1/crates/embedded-fatfs
34. `fdt` crate 元数据 —— https://crates.io/api/v1/crates/fdt
35. `fdt` README（仅解析示例）—— https://raw.githubusercontent.com/repnop/fdt/main/README.md
36. `embedded-sdmmc` crate 元数据 —— https://crates.io/api/v1/crates/embedded-sdmmc
37. `dwmmc-host` crate 元数据 —— https://crates.io/api/v1/crates/dwmmc-host
38. `sdhci-host` crate 元数据 —— https://crates.io/api/v1/crates/sdhci-host
39. linux-sunxi wiki：BROM（经由 archive）—— https://web.archive.org/web/2024id_/https://linux-sunxi.org/BROM
40. linux-sunxi wiki：可启动 SD 卡，8 KB / 128 KB 与 T113（经由 archive）—— https://web.archive.org/web/2023id_/https://linux-sunxi.org/Bootable_SD_card
41. U-Boot mainline `dram_sun20i_d1.c`（归功于 awboot）—— https://raw.githubusercontent.com/u-boot/u-boot/master/drivers/ram/sunxi/dram_sun20i_d1.c
42. `smaeul` 仓库（sun20i_d1_spl 是 C；没有 Rust T113）—— https://api.github.com/users/smaeul/repos
43. 用于缺口分析的 crates.io 搜索：`sunxi`、`allwinner`、`sdhci`、`dwmmc`、`designware`、`spi-nor`、`devicetree` —— 例如 https://crates.io/api/v1/crates?q=allwinner 和 https://crates.io/api/v1/crates?q=devicetree
44. `rcore-os/tgoskits`（`dwmmc-host`/`sdhci-host`/`sdmmc-protocol` 的源仓库）—— https://github.com/rcore-os/tgoskits
45. `smaeul/linux` 的 `d1/all` 分支（awboot 使用的内核）—— https://github.com/smaeul/linux/tree/d1/all

### 关于简报中预先收集的事实的说明
- "约 40 KB 的 dram.c 寄存器微码"：大小已确认（40,623 B）[19]；它**并非**纯粹的
  微码表——而是算法化的寄存器/时序 C [14]。
- `ff.c` 大小确认为 259,696 B；`ffunicode.c` 为 1,980,820 B [19]。
- `psci.c`/`psci_vectors.S`/`nonsec_virt.S`/`memcpy.S`/`link.ld`/`reg-*.h` 均确认存在并
  已确定大小 [19]。
- 简报中的目标名称 `armv7a-unknown-none-eabi` 不正确；有效目标是
  `armv7a-none-eabi` / `armv7a-none-eabihf` [20][21]。
- `armv7_nonsec.c` 存在（3,970 B）；简报遗漏了它 [19][12]。

---

## 附录——对用户现有 Rust SPL（`embed/app` + `embed/rt`）的审计

来源：本地工作副本 `<workspace>/rust/awboot/embed/`（`https://github.com/ElicMagus/rawboot`
的镜像，历史截至提交 `2f35515 "Tested Dram"`）。这实质性地改变了上文的两项结论。

- **移植早已远超 "hello world"。** 两个 crate：`rt`（自定义 runtime——eGON 头由 Rust
  静态数据在 `.head.head` 中生成，`_start` 在 `.head.start` 中，BSS/data 初始化，
  `link.x` 链接脚本，`cpu.rs` 含 `wfe`/CPSR/SP/通用定时器/`udelay`）和 `app`（SPL：
  `clk`、`ccu`、`gpio`、`uart`、`io`、`print`、`dram`、`sdhci`、`sdcard`）。`План.txt`
  将 LED、UART、CLK、DRAM 前魔数和 DRAM 标记为完成；SDHCI/SD 卡工作已存在但**未提交**
  （步骤 6–9：挂载 FS、加载内核、跳转 = 仍需完成）。
- **它在 stable 上构建**，与任何 nightly 假设相反：`rustup target add armv7a-none-eabi`
  + `cargo build --release` 在 `rustc 1.99.0 (b940084d7 2026-09-28)` 上成功，仅有
  `rt/src/lib.rs` 中的 `static_mut_refs` 警告（Rust 2024 lint）。输出 ELF 约 65 KB。
- **DRAM 已在 Rust 中，且"反编译"并未丢失。** `app/src/dram/mod.rs`（工作副本，63 KB，
  1,926 行，22 个函数）是可读的 Rust 移植——`auto_scan_dram_size`、
  `auto_scan_dram_rank_width`、`dqs_gate_detect`、`mctl_channel_init`、
  `auto_set_timing_para`、`mctl_phy_ac_remapping`、`ccm_set_pll_ddr_clk`、
  `eye_delay_compensation`、`auto_cal_timing` 等。`app/123.txt`（1,381 行）是相同代码
  的早期可读快照。在五个已提交的 `dram/mod.rs` 修订版（检查了 `cde25bd` 骨架 →
  `c014bb9`/`d7b3cda`/`c85111e`/`2f35515`）中，没有任何 `include_bytes!` 或超大字面量
  数组替换可读代码。唯一类似 "blob" 的数据是七个 22 字节的 `cfg1..cfg7` AC 重映射
  表——确属表格数据，且 mainline U-Boot 中也有 [41]。
- **后果：** 报告中"最大单一风险——将 `dram.c` 通过 FFI 保留在 C"**已被消除**。一个
  基本完整的 Rust 终态比 §7 建议的混合基线更具吸引力，DRAM 工作量从 L/High 重新归类为
  完成。
- 剩余移植面是 §5 仍标出的部分：**SDHCI DWMSHC + SD/MMC 卡协议**（本地进行中）、
  **FAT**（替换 2 MB 的 ChaN FatFs——使用 `fatfs` crate）、**FDT 读+写**、
  **secure-monitor/PSCI 汇编**，以及**构建打包**（`build.bat` 通过 shell 调用 C 工具
  `checksum.exe`/`xfel.exe`/`dd.exe`；将 `mksunxi` 校验和重新实现为 Rust xtask）。
- **代码暴露的开放架构问题：** 需要现代化的过时模式——`rt` crate 仍使用
  `#[no_mangle]` 静态数据 + `static mut`（Rust 2024 `static_mut_refs`），`cc` 构建
  依赖未使用（`build.rs` 中 `asm.s` 编译被注释掉），`asm.s` 为 0 字节，且
  `.cargo/config` 固定了原始 `-Tlink.x`。在扩大移植规模前值得解决。
