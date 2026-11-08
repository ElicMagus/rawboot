# `rawboot` 的 SDHCI 移植 vs 当前 upstream `szemzoa/awboot` 的 `sunxi_sdhci.c`

调研时间：2026 年，状态在 issue #19 之后更新。目标：Allwinner T113-S3。upstream
固定在 `szemzoa/awboot` `main` HEAD `5380c00fc67c975433f25c57fb481aa2b91aebf8`
（2025-11-21）。本地代码按符号引用，依据 `docs/agents/domain.md`；行号会
失效。

方括号中的参考编号对应末尾的 **来源** 节。

---

## 1. 摘要

- 在固定 SHA 处，`arch/arm32/mach-t113s3/sunxi_sdhci.c` 有 972 行 / 28,936 B，
  `sunxi_sdhci.h` 有 148 行 / 5,574 B [1][2]。最后一个触及该驱动的 upstream 提交
  是 `385481ffab`（2025-11-15，"refactor sdmmc driver, create library for fatfs, fdt,
  loaders"）[9]。
- 旧的本地模块是较早 upstream Rust 快照（`embed/app/src/sdhci/mod.rs`）
  近乎逐行的移植，并继承了其仅 PIO 的数据路径：每次传输都经过
  FIFO。upstream 把大于 64 字节的 SD 传输移至控制器的
  内部 IDMAC，配合描述符链，仅对小传输保留 PIO [1:312-418,
  582-597]。
- issue #19 移植了当前 upstream 驱动行为：复位、电压字段处理、
  总线宽度、时钟（CCU M/N/后分频器、NTSR 新时序模式、采样延迟校准）、
  命令路径，以及 IDMAC/PIO 拆分。命名遵循 upstream 的 snake_case；类型按 ADR-0007
  保持 Rust 惯用风格。

### 结论

`sunxi_sdhci.c` 的范围内行为现在已移植：64 字节以上的 IDMAC、等于或
低于 64 字节的 PIO、upstream 命令标志、时钟/延迟设置，以及起始位检测。
有意差异是 Rust 类型/API 表达、缺失的 eMMC board 实例、
未实现的 upstream `sdhci_set_voltage` 声明、通过 `print!` 记录日志，以及
从主线 Linux 引入的 #43 冷启动复位/CMD0 初始化补充。

完整的 SD/MMC *卡* 协议 parity 仍属 issue #20；#19 只让 `sdcard` 适配新的
host API，并修复执行读取所需的状态传递。

---

## 2. 寄存器与常量表面

寄存器块按 upstream 字段名转录（`gctrl`、`clkcr`、`timeout`、
`width`、`blksz`、`bytecnt`、`cmd`、`arg`、`resp0..3`、`status`、`ftrglevel`、`ntsr`、`dmac`、
`dlba`、`idst`、`idie`、`dsbd`、`drv_dl`、`samp_dl`、`skew_ctrl`、`fifo`，……），
作为私有 `reg` 模块中的偏移常量 [1:18-77][2:18-77]。位名称保留 upstream 前缀
`SMHC_GCTRL_*`、`SMHC_CLKCR_*`、`SMHC_CMD_*`、`SMHC_RINT_*`、`SMHC_STATUS_*`、
`SMHC_IDMAC_*`、`SMHC_IDMAC_DES0_*` 和 `SDXC_*` [1:42-227]。

时钟选择器和总线宽度使用 upstream 的数值：

| 名称 | 值 | 含义 |
| --- | --- | --- |
| `MMC_CLK_400K` … `MMC_CLK_200M` | `0..6` | 供 `odly`/`sdly` 使用的时序索引 |
| `MMC_VDD_27_36` | `1` | 2.7–3.6 V 电压字段 |
| `MMC_VDD_165_195` | `2` | 1.65–1.95 V 电压字段 |
| `MMC_BUS_WIDTH_1` | `1` | 1-bit 总线 |
| `MMC_BUS_WIDTH_4` | `2` | 4-bit 总线 |

---

## 3. 逐函数比较

| Upstream | #19 之后的本地 | 状态 |
| --- | --- | --- |
| `sdhci_configure_start_bit_detection` [1:229-241] | 同名 | 相同的 `dsbd` 清除/设置，非 HS400 的半周期位。 |
| `smhc_get_ccu_params` [1:243-276] | 同名 | 相同的 `id` → SMHC0/1/2 时钟、门控和复位映射；返回 `Option<SmhcCcuParams>` 而不是出参。 |
| `set_read_timeout` [1:278-310] | 同名 | 相同的 `DTO_MAX` 计算和 `gctrl` 256x 超时位。 |
| `prepare_dma` [1:312-389] | 同名 | 相同的 4 KiB 分片、`size-1` 编码、`des_chain`/`own`/`dic`、首/末标志、`dlba >> 2`、IDMAC 软复位、fix-burst、IRQ 使能和触发级别。 |
| `wait_done` [1:391-418] | 同名 | 相同的超时/错误检查和 DMA 完成条件（`rint` 标志加 `idst` 接收位）；返回 `Result<u32, u32>`。 |
| `read_bytes` [1:420-478] | 同名 | 相同的 FIFO 空自旋、FIFO 级别排空、data-over/auto-stop 完成、剩余量检查。 |
| `write_bytes` [1:480-523] | 同名 | 相同的 FIFO 满自旋、字推送、完成和剩余量检查。 |
| `sdhci_transfer` [1:525-645] | 同名 | 相同的 stop 快捷路径、响应标志、在 `>= MMC_CLK_50M` 时的保持寄存器、数据设置、`rint` 清除、**64 字节 IDMAC/PIO 拆分**、完成等待、busy 等待、响应顺序和 IDMA 清理；另加 CMD0 上的 `SEND_INIT_SEQUENCE`（有意，#43）。 |
| `sdhci_reset` [1:647-651] | 同名 | 写入 `HARDWARE_RESET` 并轮询 `GCTRL` 直到自清除位消失（250 ms 上限）；有意偏离 upstream，#43。 |
| `sdhci_set_width` [1:653-694] | 同名 | 相同的宽度常量、DDR 门控、起始位检测、DDR 重新校准。 |
| `init_default_timing` [1:696-715] | 同名 | 相同的 `TM5_OUT_PH*` / `TM5_IN_PH*` 表。 |
| `config_delay` [1:717-787] | 同名 | 相同的 `drv_dl`、`ntsr` 采样延迟、硬件 skew 模式，以及 `> MMC_CLK_400K` 的 `samp_dl` 校准。 |
| `update_card_clock` [1:789-801] | 同名 | 相同的 `UPCLK_ONLY` 握手和 `rint` 清除。 |
| `sdhci_set_clock` [1:803-938] | 同名 | 相同的 CCU 后分频器补偿、DDR 2x 模块时钟、OSC24M/PLL_PERIPH1X 源、M/N 分频、新时序位、门控/复位序列、`clkcr` 分频器和更新握手。 |
| `sunxi_sdhci_init` [1:940-971] | 同名 | 相同的 GPIO mux/pull 顺序、默认时序、复位、`rint` 清除、DMA 触发级别和起始位检测。 |
| `sdhci_set_voltage` [2:142] | 同名 | upstream 只声明它；本地保留 `false` 存根，因为头文件没有实现，T113-S3 路径中也没有调用者。 |

---

## 4. #19 中修复的差异

1. **数据路径。** 旧模块总是通过 FIFO 使用 `read_bytes`/`write_bytes`。
   `blkcnt * blksz > 64` 的传输现在构建 IDMAC 描述符链并让 SMHC
   控制器搬运数据；不超过 64 字节的传输保留 upstream 的 PIO 路径。相同的
   `prepare_dma` 清理会清除 `idst`，禁用 `idie`、`dmac` 和 DMA 门控。
2. **时钟。** 旧的 `init` 硬编码 CCU 门控/复位写入和 `pclk = peri1x / 12`
   比率；`set_clock` 使用简单的 `pclk` 分频器。移植现在做 upstream 所做的事：
   选择 OSC24M 或 PLL_PERIPH1X，使用带 T113 后分频器
   补偿的 CCU M/N 分频器，强制 `NTSR` 新时序模式，使用固定的 `clkcr` 分频器（DDR 模式下为 2），
   并在每次时钟变更时运行 `config_delay`。
3. **复位与初始化。** `sunxi_sdhci_init` 现在只对引脚做 mux/pull，加载时序
   表，复位控制器，清除中断，设置 DMA 触发级别并启用
   起始位检测。门控/复位/时钟使能在 `sdhci_set_clock` 中发生，与 upstream 一致。
4. **总线宽度。** 宽度值现在使用 upstream 的 `MMC_BUS_WIDTH_1 = 1`、
   `MMC_BUS_WIDTH_4 = 2`，并且 `sdhci_set_width` 应用 DDR/边沿锁存 `gctrl` 位。
5. **命令路径。** `sdhci_transfer` 现在在写参数之前清除 `rint`，在 50 MHz 及以上
   设置 `USE_HOLD_REGISTER`，清除匹配的 `idst` 方向位，设置
   `DATA_EXPIRE`/`WAIT_PRE_OVER` 和 `SEND_AUTO_STOP`，并使用 upstream 的 100 ms 命令
   超时和 6000 ms 数据超时。
6. **起始位检测与延迟。** `dsbd` 编程、`drv_dl`/`samp_dl`/`skew_ctrl`
   校准和延迟表是转录而来，而非猜测。
7. **命名。** 驱动函数和寄存器/位常量现在与 upstream 一致；寄存器
   块通过命名偏移寻址，而不是魔法数字。
8. **PLL_peri 速率（阻塞性缺陷）。** `sunxi_clk_get_peri1x_rate` 在
   `u8` 中计算 `24 * plln`，所以 1200 MHz 的 PLL_peri0 读回为 100 MHz，`sdhci_set_clock(50M)`
   编程的分频器为 1 而不是 6。算术现在改为 `u32`，与 C 的提升规则一致；
   HIL 确认 `CCU_SMHC0_CLK_REG` 读回预期的 M/N 字段，卡在 50 MHz 下到达
   CMD16/读取。

---

## 5. 有意差异

- **类型与控制流。** `Sdhci`、`SdhciCmd`、`SdhciData` 和 `SdhciIdmaDesc` 取代
  C 结构体；`Option<&SdhciData>` 取代可为 `NULL` 的数据指针；`wait_done` 返回
  `Result<u32, u32>`；`smhc_get_ccu_params` 返回一个小结构体。行为和字段顺序
  保持 upstream 的（ADR-0007）。
- **Board 实例。** 只转录了 SD 实例（`sdhci0`、PF 引脚、4-bit、50 MHz）。
  eMMC 实例（`sdhci2`）超出 T113-S3 SD 特性范围，不
  实例化；CCU 辅助函数仍知道控制器 0..2。
- **电压。** `sdhci_set_voltage` 是 `false` 存根：upstream 在
  `sunxi_sdhci.h` 中声明它，但未在 `sunxi_sdhci.c` 中定义，而且 SD 路径只在
  `sdmmc.c` 中读取 `hci->voltage` [1][2]。
- **日志。** upstream 的 `trace`/`debug`/`warning`/`error` 变为 USART0 上的 `print!`，与
  loader 的其余部分一致。
- **PIO 缓冲区访问。** `read_bytes`/`write_bytes` 使用 `read_unaligned`/`write_unaligned`
  而不是 C 强制转换，因此调用者的字节缓冲区不必按字对齐。FIFO 仍
  搬运 32 位字，与 upstream 一致。
- **描述符保护。** `prepare_dma` 拒绝需要超过 `Sdhci` 中 32 个描述符的传输，
  而不是越界写入数组。
- **FIFO 级别刷新。** `read_bytes` 在 FIFO 空自旋之后重新读取 `status`，再
  计算级别，而不是使用自旋之前采样的值。
- **复位完成。** `sdhci_reset` 轮询 `GCTRL` 直到 `HARDWARE_RESET` 位清除
  （250 ms 上限），与主线 Linux `sunxi_mmc_reset_host` 一致。upstream 在写入后
  立即返回，所以在 BROM 移交之后，时钟/时序编程可能与进行中的复位产生竞态。
  为 #43 采纳；见 `research/sd-cold-start-brom.md`。
- **CMD0 初始化序列。** `sdhci_transfer` 为
  `MMC_GO_IDLE_STATE` 设置 `SEND_INIT_SEQUENCE`，与主线 Linux 一致。随后 SMHC 硬件
  提供 SD 物理层在 CMD0 之前要求的 74+ 个时钟；upstream 则依赖调用者的
  CMD0 后延迟。为 #43 采纳；见 `research/sd-cold-start-brom.md`。

### 非必需项

- **缓存维护。** loader 在关闭 MMU 和缓存的情况下运行（ADR-0004/ADR-0006）；
  IDMAC 描述符和缓冲区是普通 SRAM/DRAM 访问，因此无需 clean/invalidate 步骤。
- **DMA 引擎、SPI-NAND、看门狗和异常辅助。** 由父 issue
  （#14）推迟并单独跟踪（#21）。
- **T113-S4/V851s。** 超出范围；只保留 T113-S3 引脚/时钟表。

---

## 6. `sdcard` 影响

issue #19 只更改了新 host API 所强制要求的内容；issue #20 随后把完整卡
协议与 `sdmmc.c` 对齐（见 `research/sdcard-port-vs-upstream.md`）。#19 的更改是：

- `Sdhci` 现在是非 `Copy`，并由 `Sdcard_data` 拥有；卡和 HCI 以
  `&mut`/`&` 传递而不是按值，因此响应、RCA、CSD 和时钟/宽度状态在
  检测之后仍保持。
- `sdhci_cmd_t::cmdidx/cmdarg` 变为 `SdhciCmd::idx/arg`；`hci.transfer(...)` 变为
  `sdhci_transfer(..., Option<&SdhciData>)`。
- 宽度检查使用 upstream 的 `MMC_BUS_WIDTH_*` 值，最终时钟使用
  `hci.clock_wanted`。
- 补上了当前 `sdmmc_detect` 中缺失的 CMD0 后 2 ms 延迟；若没有它，
  冷启动时第一次 CMD8 会超时。
- CMD55 重试循环有界，因此卡缺失时检测会失败，而不是永远自旋
  打印 `cmd55 timeout`。
- 检测以 100 ms 暂停重试，与 `sdmmc_init` 一致；这消除了
  第一次 CMD8 在卡就绪之前到达的冷启动不稳定。

CID/CSD/EXT_CSD 解析和命令时序在 #20 之前仍是旧移植；完整卡协议 parity
见 `research/sdcard-port-vs-upstream.md`。

---

## 7. 硬件确认

在干净地断电重启进入 FEL 之后，在 T113-S3 上用 `scripts/hil-check.sh target/rawboot-fel.bin iss19`
完成验证。脚本确认了安静的 UART、从代码分支
（`f31d65a`，dirty=false）构建的镜像、一个 `Hello`，以及所有必需断言：

```
Hello
CLK: CPU PLL=PLL_CPU FREQ=1008MHz
CLK: PLL_peri (2X)=1200MHz, (1X)=600MHz, (800M)=800MHz
CLK: PLL_ddr=432MHz
trefi:7.8ms
DRAMC read ODT  off.
DRAM ODT value: 0x42.
DRAM simple test OK.
SD/MMC card found
SD: read 1 block(s), first bytes [00, 00, 00, 00]
S=0x4
PASS: UART assertions
```

- `SD/MMC card found` 意味着 CMD0/CMD8/ACMD41/CID/CSD/RCA/select 以及 50 MHz 4-bit 切换
  都在一次冷启动中成功。
- 512 字节读取高于 64 字节阈值，因此走 IDMAC；返回的
  块被零填充，因为测试卡是空白的。
- `S=0x4` 是读取之后 LED 循环的状态行。
- 不执行缓存维护；loader 在关闭缓存的情况下运行。
- PD22 上的 LED 是脚本的人工检查项，在同一个循环中闪烁。

---

## 8. 未决问题

1. **IDMAC 与 SRAM。** 描述符数组位于 loader 常驻 SRAM 的 `Sdhci`
   值中；缓冲区位于 DRAM。upstream 把 `sdhci0` 放在其全局数据中。HIL
   确认 512 字节 IDMAC 读取在此布局下可用。
2. **`set_read_timeout` 字段类型。** upstream 把 `clock_active`（一个时序*索引*，不是
   Hz）除以 1000，所以计算出的值通常为零，代码回退到
   最大超时。移植转录该行为，而不是悄悄"修复"它；
   值得对照未来的 upstream 更改加以确认。
3. **eMMC 路径。** `sdhci_set_width` 只知道 1-bit 和 4-bit 模式，与 T113-S3
   upstream 头文件一致；8-bit eMMC 实例需要推迟的 `sdhci2` 工作。
4. **卡读取时序。** IDMAC 接收路径在关闭缓存的情况下于 T113-S3 上完成了 512 字节读取；
   不需要缓存维护。

---

## 来源

1. upstream `arch/arm32/mach-t113s3/sunxi_sdhci.c`，位于 `5380c00fc67c975433f25c57fb481aa2b91aebf8` —
   <https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/sunxi_sdhci.c>
2. upstream `arch/arm32/mach-t113s3/sunxi_sdhci.h`，同一 SHA —
   <https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/sunxi_sdhci.h>
3. upstream `arch/arm32/mach-t113s3/sdmmc.h`，同一 SHA（命令/响应宽度）—
   <https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/sdmmc.h>
4. upstream `arch/arm32/mach-t113s3/sdmmc.c`，同一 SHA（卡协议；issue #20）—
   <https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/sdmmc.c>
5. upstream `board.c`，同一 SHA（sdhci0 实例和 PF 引脚）—
   <https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/board.c>
6. upstream `arch/arm32/mach-t113s3/reg-ccu.h`，同一 SHA（SMHC 时钟/门控寄存器）—
   <https://raw.githubusercontent.com/szemzoa/awboot/5380c00fc67c975433f25c57fb481aa2b91aebf8/arch/arm32/mach-t113s3/reg-ccu.h>
7. 本地 `rawboot/src/sdhci/mod.rs`、`rawboot/src/sdcard/mod.rs`、
   `rawboot/src/ccu/mod.rs`、`sunxi-pl/src/cpu.rs`（定时器，由 `rawboot-rt` 重新导出）。
8. 随仓库附带的移植前参考 `embed/app/src/sdhci/mod.rs`。
9. GitHub API，`arch/arm32/mach-t113s3/sunxi_sdhci.c` 的提交历史 —
   <https://api.github.com/repos/szemzoa/awboot/commits?path=arch/arm32/mach-t113s3/sunxi_sdhci.c&per_page=20>
10. ADR-0007，`docs/adr/0007-align-behaviour-to-upstream.md`。
11. Issue #19 "Parity SD-хоста: sdhci (IDMAC)" 与父 issue #14。
12. 具有相同格式的先前研究笔记：`research/gpio-io-uart-port-vs-upstream.md`、
    `research/dram-port-vs-upstream.md`。
