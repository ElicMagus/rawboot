> 状态：草稿。

## 问题陈述

T113-S3 引导加载程序 `rawboot` 是 C 版 `szemzoa/awboot` 的部分 Rust 移植。
它已经初始化了时钟、UART、GPIO 和 DRAM，但是：其 SD/SDHCI 路径无法工作；它围绕 Linux
（zImage/FDT/initrd）设计，尽管 payload 将是用户自己的固件；它仍然带有未命名的 magic
number；并且其唯一的构建路径是 Windows `.bat` 脚本，这些脚本封装了预构建的 `.exe` 工具
（`checksum.exe`、`dd.exe`、`xfel.exe`）。用户想要一个干净、符合惯用法、100% Rust 的第二阶段
loader，它能在 T113-S3 上从 SD 卡引导用户自己的固件（不是 Linux），可在 Linux 上构建并进行
单元测试，同时不丢失现有的 Windows 路径。

## 解决方案

将项目改造为一个 Cargo workspace——包含 loader crate（`rawboot`）和 runtime crate
（`rawboot-rt`）——它会启动 T113-S3，通过 SDHCI 从 SD 卡上的 FAT 分区读取 raw payload 镜像，
将其加载到 DRAM，并通过一个小的、带版本的 `BootInfo` 约定移交控制权。寄存器布局从 T113
数据手册（并以 awboot 作为参考）转写为具名常量，绝不使用 magic number。跨平台的 Rust xtask
使用 Allwinner eGON 头封签镜像，以便 BROM 接受它们，在 Linux 和 Windows 上均可。一个简单的
blink payload 在真实硬件上验证整条链路（SD → FAT → DRAM → handoff）。

## 用户故事

1. 作为固件开发者，我希望在 Linux 上使用稳定版 Rust 构建 loader，这样我就不再依赖 Windows
   或预构建的 `.exe` 工具。
2. 作为固件开发者，我希望 runtime 和 loader 位于同一个 Cargo workspace 中，这样它们可以用
   一条命令一起构建。
3. 作为固件开发者，我希望寄存器定义以具名常量表示，这样我再也不会读到裸的 magic number。
4. 作为固件开发者，我希望 loader 保持 100% Rust，这样我没有 C 代码需要维护。
5. 作为固件开发者，我希望 loader 初始化 DRAM，这样 payload 可以从 DRAM 地址空间运行。
6. 作为固件开发者，我希望 loader 初始化 UART，这样我可以读取启动日志。
7. 作为固件开发者，我希望 loader 驱动板载 LED，这样没有串口线我也能得到生命信号。
8. 作为固件开发者，我希望 loader 读取 SD 卡上的 FAT 分区，这样我可以把 payload 文件放到
   卡上，而不必裸刷镜像。
9. 作为固件开发者，我希望 SDHCI/SD 可靠工作，这样 SD 卡成为主要的启动介质。
10. 作为固件开发者，我希望有固定的 payload 文件名，这样启动流程可预测。
11. 作为固件开发者，我希望 FAT 分区上有一个可选的 `boot.cfg`，这样我可以在不重新构建
    loader 的情况下向 payload 传递参数。
12. 作为固件开发者，我希望 loader 遵循 payload 镜像头中的加载地址，这样 payload 控制自己
    被放置的位置。
13. 作为固件开发者，我希望镜像头中有校验和，这样损坏的 payload 会被拒绝而不是被执行。
14. 作为固件开发者，我希望有一个带版本的 `BootInfo` 结构体，这样 handoff 约定可以演进而
    不破坏旧的 payload。
15. 作为固件开发者，我希望记录 handoff 时的 CPU 和寄存器状态，这样我可以编写 payload 的
    启动序列。
16. 作为固件开发者，我希望 loader 为 payload 设置栈，这样最小 payload 完全不需要启动代码。
17. 作为固件开发者，我希望有一个 blink 测试 payload，这样我可以在真实硬件上验证端到端
    handoff。
18. 作为固件开发者，我希望通过 FEL 启动，这样我可以在不接触 SD 卡的情况下快速迭代。
19. 作为固件开发者，我希望 eGON 封签以 Rust xtask 实现，这样镜像在 Linux 上对 BROM 有效。
20. 作为固件开发者，我希望保留现有的 Windows 构建脚本，这样需要时我仍可在 Windows 上构建。
21. 作为固件开发者，我希望针对镜像封签和头解析有主机单元测试，这样无需开发板就能捕获格式
    错误。
22. 作为固件开发者，我希望对 `BootInfo` 有编译期布局断言，这样 ABI 破坏无法悄悄发布。
23. 作为固件开发者，我希望有 clippy 和 CI，这样移植在增长过程中保持干净。
24. 作为固件开发者，我希望有一条尽力而为的 JTAG 调试路径，这样当 UART 不够时我可以调试。
25. 作为固件开发者，我希望 SoC 特定代码隔离在寄存器/驱动模块之后，这样以后可以在不重写的
    情况下添加另一个 Allwinner SoC。
26. 作为固件开发者，我希望有意省略 FDT 和 PSCI，这样 loader 保持小巧且与 payload 无关。
27. 作为固件开发者，我希望 loader 以 CC-BY-4.0 发布，这样它可以在署名的情况下被复用。
28. 作为固件开发者，我希望使用软浮点目标，这样不会有 VFP 依赖渗入 loader。

## 实现决策

- **Workspace 形态。** `rawboot` 仓库是一个单独的 Cargo workspace，其成员包括
  `rawboot`（loader）、`rawboot-rt`（runtime）、`sunxi-pl`（共享硬件层）、
  `sunxi-drivers`（其上的共享驱动：clock/CCU、DRAM、UART、SDHCI、SD/MMC 以及 FAT
  适配器，在 loader 中只读，可由 flasher 和主机测试写入）、`payload`（blink 测试 payload）、
  `rawboot-image`（可在主机上测试的
  payload、eGON、SD boot prefix 以及 flash 任务/状态工具）、`rawboot-fel`（主机侧 FEL USB
  传输和 CLI）、`rawboot-flasher`（可由 FEL 调用的 DDR-init stub 和链接到 DRAM 的 flasher）
  以及 `xtask`（主机构建工具）。驱动模块位于 `sunxi-drivers`；`rawboot` 保留
  启动编排及其 `print!`，而 payload 头解析和 eGON 封签由
  `rawboot-image` 负责。由 loader、runtime 和 payload 共享的硬件层——volatile 访问器、
  GPIO 和定时器——不包含启动代码（ADR-0010）；驱动 crate 不包含启动
  代码、eGON 头、链接脚本或分配器（ADR-0012）。领域文档（`GLOSSARY.md`、
  `docs/adr/`）位于仓库根目录。
- **Runtime crate。** 在 head 段中以静态变量形式铺设 eGON 头（magic `eGON.BT0`、分支字、头
  字段），定义 `_start`（栈、清除 BSS、复制 data），并携带链接脚本。已现代化，使用 `&raw`
  指针消除 `static_mut_refs` 隐患。
- **Driver crate。** `sunxi-drivers` 包含 `clk`/`ccu`、`uart`、`dram`、`sdhci`、`sdcard` 和
  `fat`。驱动进度通过 `Console` trait 输出；loader 安装一个 USART0 sink，而未安装任何 sink
  的二进制仍会以静默方式获得驱动行为。
- **Loader 模块。** `print` 和 `boot`（编排、`BootInfo`、handoff）保留在
  `rawboot` 中。MMIO 常量和寄存器布局定义在使用它们的驱动旁边，
  而不是放在共享的 `regs` 模块中。payload 头解析和 eGON 封签位于
  `rawboot-image` crate 中，loader 以 `default-features = false` 依赖它。
  volatile 访问器（`io`）和 `gpio` 位于 `sunxi-pl` 中，并由 loader
  重新导出（ADR-0010）。
- **eGON 封签算法。** 用 `0x5F0A6C39` 初始化头部校验和字段，然后对填充后的镜像累加
  `length_aligned / 4` 个小端 32 位字（32 位回绕），并将和写回；块设备的填充为 512，flash
  的填充为 8192；当 BROM 页大小大于 2 KB 时，将每个 2 KB 逻辑页散布到一个该大小的零填充页中。
- **Payload 镜像头。** payload 前部有一个固定的 `#[repr(C)]` 头：
  `magic, version, flags, entry, load_addr (default 0x42000000), size, checksum`。loader 遵循
  `load_addr`，并在跳转前验证 `checksum`。`flags` 保留给诸如压缩或加密之类的未来特性。
- **`BootInfo`。** 一个带版本的 `#[repr(C)]` 结构体，由 loader 放置在 DRAM 中并通过 `r0`
  传给 payload：`magic, version, boot_medium, ram_size, image load address/size/entry, args
  pointer`。`r1` 为零。
- **Handoff 状态约定。** 以 ARM 状态、Supervisor（SVC）模式进入 payload，MMU 和缓存关闭，
  IRQ/FIQ 屏蔽，`SP` 指向保留的 DRAM 栈。CPU0 完全按照 BROM 留下的状态（secure）移交；
  CPU1 保持停放，payload 负责 SMP bring-up。没有 TrustZone 转换，也没有 PSCI。
- **没有 Linux 协议。** 不解析/修改 FDT，不处理 zImage/initrd。
- **驻留。** loader 常驻 SRAM（硬预算 ≤ 64 KB），从不重定位自身；payload 直接加载到 DRAM。
  如果超出预算，则添加显式的 DRAM 重定位阶段，而不是悄悄增长。
- **存储。** SD/eMMC 通过 SDHCI（Synopsys DesignWare MSHC）控制器驱动；FAT 文件系统由
  `fatfs` crate 通过块设备适配器提供，取代内置的 2 MB ChaN FatFs。
- **构建工具。** 跨平台的 Rust xtask 组装并 eGON 封签镜像（`rawboot-fel.bin`、
  `rawboot-boot-sd.bin`、`rawboot.img`）。loader 由同一份代码链接两次：
  `rawboot-fel.bin` 链接到 `0x00028000` 供 `xfel` 使用，`rawboot-boot-sd.bin` 链接到
  `0x00020000` 供 BROM 从扇区 16 进行 SD/SPI 启动。`cargo xtask sd-image
  --partition-sectors <n>` 写出 `rawboot-sd-mbr.img`，即 SD boot prefix：一个指向从
  `--start-lba`（默认 2048，1 MiB）开始的 FAT 分区的 MBR，加上位于扇区 16 的
  `rawboot-boot-sd.bin`（ADR-0011）。它不格式化任何东西，不复制 payload，只写出一个文件。
  `cargo xtask flasher` 构建可返回的 DDR-init stub 和链接到 DRAM 的 flasher（ADR-0013）。
  `cargo xtask fel-flash` 构建或复用这些产物，并通过 FEL 将 `payload.bin`、可选的 `boot.cfg`
  和封签后的 SD loader 写入留在板上的卡中，保持 MBR 和分区不变，并读回所有内容
  （ADR-0014）。现有的 Windows 脚本和 `.exe` 工具作为可用的 Windows 路径保留。
  `rawboot-fel`（CLI `cargo fel`）直接与 BROM 的 FEL USB 协议通信以进行手动 bring-up；
  `cargo fel bringup` 运行 stub、检查 DRAM 并读取 flash 状态。`xfel` 仍是
  `scripts/hil-check.sh` 使用的外部工具。
- **工具链。** 目标 `armv7a-none-eabi`（软浮点；经测量，对本代码而言比硬浮点变体更小），
  Rust edition 2024，由 `rust-toolchain.toml` 固定。
- **许可证。** `CC-BY-4.0`，文件中带有 SPDX 头。仅从 awboot 转写事实（地址、
  偏移、常量名）；不复制 GPL-2.0+/无许可证的 awboot 源码。
- **调试。** 通过 USB-JTAG 适配器的尽力而为的 `openocd` + `gdb` 路径；FEL 是主要的
  快速迭代加载路径，可通过 `xfel` 或树内的 `rawboot-fel` 传输。

## 测试决策

- 好的测试检验的是**外部行为，而不是实现细节**——对本项目而言，这意味着镜像工具上的
  字节进/字节出，以及硬件上可观察的 LED 行为。不存在 SoC 或开发板的模型。mock 只位于字节
  和块接缝处：FEL 传输 trait 背后的脚本化 USB bulk 传输（issue #40）检查线缆帧，
  `sunxi-drivers` 中的内存块磁盘替代扇区读取、写入和注入的失败，而不模拟
  SDHCI、卡时序或电气行为。
- **单元接缝（主机，`cargo test`）：** 镜像工具。`seal()` 必须产生 BROM 有效的头
  （使用 `0x5F0A6C39` 种子对填充后的镜像计算校验和、正确的 512/8192 填充、正确的
  2 KB→N KB 页散布）；`parse()` 必须接受格式良好的头，并拒绝错误的 magic、错误的
  校验和或截断/超长的长度。这些是作用于 `&[u8]` 的纯函数。SD boot
  prefix 同样是字节进/字节出：MBR 字段、loader 放置、前缀长度，以及拒绝
  未封签、重叠或溢出的输入。FEL SD 路径以同样的方式检查：
  `FlashPlan` 往返并拒绝错误的长度/校验和，`check_sd_layout` 拒绝位于扇区 16 处或内部的
  分区、伸入分区的 loader 以及超出卡的分区，`fat::file_matches` 逐字节比较写入的文件。
- **驱动接缝（主机，`cargo test`）：** `sunxi-drivers` 在实现块 `BlockIo` trait 的内存
  `MockDisk` 上测试块适配器和 FAT 写入。该 mock 替代扇区 I/O 和可注入的读/写失败；
  它不模拟 SDHCI 寄存器、卡时序或卡本身。测试无需开发板即可覆盖
  `create_or_replace_file`/`file_matches` 和分区解析。
- **FEL 传输接缝（主机，`cargo test`）：** `rawboot-fel` 构造 BROM 的 `AWUC`/FEL
  请求帧，并通过 `Transport` trait 验证其应答。脚本化的 bulk 帧覆盖
  version/read/write/exec、64 KiB 分块、短传输和传输失败，`flash_sd_card` 暂存
  plan、payload、`boot.cfg` 和 loader 的顺序，以及在接触设备之前的拒绝行为；
  开发板检查仍是最终裁决。
- **编译期接缝：** 对 `BootInfo`/镜像头大小和字段偏移的 `const` 断言，这样 ABI
  破坏会导致构建失败。
- **硬件在环验收接缝：** 使用 xtask 构建 blink payload，通过 FEL 和 SD 启动 loader，
  并观察 blink 模式。这是端到端验收测试。
- **先例：** 仓库中尚无先例；为工具 crate 建立主机 `cargo test` 目标是本工作的一部分。

## 范围之外

- FDT 解析/修改；Linux 启动协议；zImage/initrd。
- PSCI、secure→non-secure monitor 和 SMP bring-up。
- SPI-NOR/NAND 启动和 DMA 引擎（后续里程碑）；eMMC。
- T113-S4、V851s 及其他 SoC。
- payload 镜像加密和替代文件系统。
- 最终将成为 payload 的微内核/固件。

## 补充说明

- SD 目前**不能**工作；修复/验证 SDHCI + 卡协议路径是第一个阻塞性 ticket。JTAG 和逻辑
  分析仪是为此准备的预期工具。
- 保持 blink 模式简单但有辨识度（例如短-长节奏），以便人能区分“payload 正在运行”和任何
  启动前的 LED 活动。
- `research/awboot-rust-migration.md` 中的研究报告和 ADR `0001`–`0006` 是上述决策的主要
  来源。
