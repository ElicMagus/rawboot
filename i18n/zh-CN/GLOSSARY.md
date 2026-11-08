# rawboot — 全志 T113-S3 引导加载程序

面向全志 T113-S3（ARMv7-A，双核 Cortex-A7）的第二阶段引导加载程序，用 Rust 编写，移植自
`szemzoa/awboot`，但采用地道的 Rust 风格。它负责初始化 SoC、加载自定义固件镜像并移交控制权。

## 术语

**BROM**：
全志的片上第一阶段启动 ROM；它从 SD（扇区 16，回退 256）、SPI flash 或 USB FEL 加载并校验 eGON 镜像。

**FEL**：
全志的 USB 恢复/启动模式；主机工具将镜像上传到 SRAM/DRAM 并启动它。

**FEL transport**（crate `rawboot-fel`）：
我们对 BROM 的 FEL USB 协议的主机端实现：设备发现、版本与芯片 ID，以及内存读/写/执行，并带有用于手动
bring-up 的 CLI。`xfel` 仍是 HIL 脚本使用的外部工具。
_避免_：FEL library、xfel clone

**FEL mailbox**：
FEL 模式 SRAM 中的一个四字块（magic `RBFM`、payload 镜像地址、镜像大小、参数指针），主机在通过 FEL
启动 loader 之前写入。若存在，loader 会启动上传的 payload 镜像，而不是从 SD 卡读取 `payload.bin`。

**DDR-init stub**（镜像 `rawboot-ddr-init`）：
一个可由 FEL 调用的小程序，加载到 SRAM 的 `0x00028000`，用共享驱动初始化 DRAM，将结果留在 SRAM 中
并返回 BROM，从而保持 FEL 可用。它不携带 eGON 头，也不是第二阶段的 loader（ADR-0013）。

**flasher**（crate `rawboot-flasher`）：
链接到 DRAM 中 `0x40010000` 的、可由 FEL 调用的程序，它读取 flash job、执行该任务、写入 flash status
并返回 BROM。它响应 `NOOP` 任务以及 `FLASH_COMMAND_FLASH_SD` 任务，后者会把暂存的 payload 文件和
loader 写入卡中。不携带 eGON 头。

**flash job/status**：
位于固定 DRAM 地址的、带校验和与版本号的 `FlashJob`/`FlashStatus` 块，由主机与 flasher 交换
（`RBFJ`/`RBFS`、magic、版本、大小、校验和、序列、步骤、结果）。

**flash plan**：
位于 `0x40000200` 的、带校验和的 `FlashPlan`（`RBFP`）块，它指明暂存的 payload 镜像、可选的
`boot.cfg` 内容以及已封装的 SD/SPI loader 的长度和校验和。主机在执行 `FLASH_COMMAND_FLASH_SD` 任务前
写入它；flasher 会校验它以及暂存区域。

**FEL flash**（`cargo xtask fel-flash`）：
板内更新路径：构建或复用 loader、payload 和 flasher 镜像，校验已封装的 SD loader，通过 FEL 将所有
内容暂存到 DRAM，并让 flasher 写入现有的 FAT 根目录和扇区 16，而不触碰 MBR（ADR-0014）。

**eGON**：
全志的启动镜像头（magic `eGON.BT0`，用 `0x5F0A6C39` 进行 seed 校验和计算），前置到阶段二进制，以便
BROM 接受它。

**loader**（crate `rawboot`）：
我们的第二阶段程序：初始化 SoC，从存储读取 payload，并执行 handoff。
_避免_：app、SPL

**runtime**（crate `rawboot-rt`）：
启动层，负责放置 eGON 头、设置栈、清除 BSS/复制数据，并调用 loader 的 `main`。
_避免_：framework、OS、HAL

**`sunxi-pl`**（平台层）：
loader、`rawboot-rt` 和 payload 共享的硬件层：MMIO、GPIO、定时器。类 CMSIS，不携带启动代码。
_避免_：HAL、驱动 crate、平台

**`sunxi-drivers`**（驱动 crate）：
位于 `sunxi-pl` 之上的全志 T113-S3 驱动：clock/CCU、DRAM、UART、SDHCI、SD/MMC 以及 FAT 适配器
（在 loader 中只读，在 flasher 和主机测试中读写）。由 loader 和 flasher 共享；不携带启动代码、
eGON 头、链接脚本或全局分配器，并通过 `Console` trait 报告进度，因此不依赖 loader 的 `print!`。
_避免_：HAL、BSP、平台层

**payload**：
loader 启动的镜像——用户自己的固件（可能是微内核），包括诸如闪烁循环之类的简单测试镜像。不是 Linux。
_避免_：kernel、zImage

**image header**：
payload 镜像前部的固定 `#[repr(C)]` 头（magic、版本、标志、入口、加载地址、大小、校验和），由 loader
解析。

**SD boot prefix**：
主机构建的 `target/rawboot-sd-mbr.img`：一个指向现有 FAT 分区的 MBR，加上位于扇区 16 的已封装 SD/SPI
loader，其长度恰好等于分区起始位置（默认 1 MiB）。它不携带 FAT 卷，也不携带 payload，通过
`dd conv=notrunc` 写到卡上；格式化和 `payload.bin` 仍保留在分区上。
_避免_：SD image、full image

**BootInfo**：
loader 放入 DRAM 并在 handoff 时通过 `r0` 传给 payload 的、带版本号的 `#[repr(C)]` 描述符。

**boot.cfg**：
FAT 卷上的可选文件，loader 将其内容作为以 NUL 结尾的字符串复制到 DRAM，并通过 `BootInfo.args`
暴露；缺失时表示 `args` 指针为 null。

**handoff**：
在已定义的 CPU 与寄存器状态下，将控制权从 loader 移交给 payload。

**DRAM init**：
启动 T113 DDR3 控制器；没有它 DRAM 不可用。

**upstream**：
`rawboot` 移植自的 C 项目 `szemzoa/awboot`；我们所对齐的行为。
_避免_：original、reference、vendor

**parity**：
与 upstream 的行为等价——相同的步骤顺序、常量、分支和表——用我们自己的 Rust 表达实现。
_避免_：port、match、fidelity

**regs**：
从 T113 数据手册和 awboot 转写而来的具名 MMIO 寄存器常量与结构体。
