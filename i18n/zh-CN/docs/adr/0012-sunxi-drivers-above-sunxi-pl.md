# sunxi-drivers：sunxi-pl 之上的共享驱动

loader 的驱动（`ccu`/`clk`、`dram`、`uart`、`sdhci`、`sdcard`、`fat`）原本位于 bin crate `rawboot` 内部。第二个镜像（例如来自 #34 的 flasher）无法在不复制代码的情况下复用它们；副本不可避免会偏离 loader。底层——MMIO、GPIO 和定时器——已经移入 `sunxi-pl`（ADR-0010），但其上的驱动仍留在引导加载程序中。

决策：在 `sunxi-pl` 之上出现了一个 `no_std` crate `sunxi-drivers`。它包含 `ccu`、`clk`、`dram`、`uart`、`sdhci`、`sdcard` 和 `fat`，不携带启动代码、eGON 头、链接脚本和全局分配器。`rawboot` 依赖它，并保留自己的启动编排（`boot`）、`BootInfo` 和自己的 `print!`。该 crate 的公开边界：`Sdhci`、`Sdcard`、`DramPara`（带公开字段的参数）、带块适配器 `fat::block::BlockDevice`/`fat::partition::Partition` 的 `fat::mount`；`fat::read_boot_files` 仍然是 loader 在其之上的便利包装。

驱动的输出通过一个窄 trait `console::Console` 进行，而不是 loader 的 `print!`：二进制设置自己的接收端（loader 将其指向 USART0），没有接收端时驱动静默工作。这为 boot 日志保留了 `rawboot::print!`，并且不会将特定的 UART 拖入 crate。

## 考虑过的选项

- **共享驱动 crate**（已采用）：loader 和未来镜像的单一驱动来源，延续了已被采用的 `sunxi-pl`（ADR-0010）和 `rawboot-image` 模式。可以在不引入 `_start` 和 eGON 的情况下接入该 crate。
- **将驱动留在 `rawboot` 中并添加 lib target**（已拒绝）：bin crate 会将启动策略（`payload.bin`/`boot.cfg` 路径、`BootInfo`、handoff）与驱动混在一起，而“哪些可以复用”的边界仍会不明确。
- **在第二个镜像中复制驱动**（已拒绝）：代码会偏离 loader；修复将不得不做两次。
- **用 `log` 门面代替自己的 trait**（已拒绝）：驱动消息会接入 `log` 基础设施，使镜像超出 64 KiB SRAM 预算。带单一 `#[inline(never)]` 格式化路径的 `Console` trait 更廉价，且不需要全局 logger。

## 后果

- `rawboot` 不再包含驱动模块：`rawboot/src` 中保留 `main.rs`、`boot.rs` 和 `print.rs`。在决策时，`sunxi-drivers` 属于 bare-metal `default-members`，而 host 测试位于 `rawboot-image`、`rawboot-fel` 和 `xtask` 中。
- `fatfs` 和 `core_io` 已移入 `sunxi-drivers` 的依赖；`log` 配合 `max_level_off` 仅用于将 `fatfs` 的 trace/debug 调用编译为空。
- 64 KiB SRAM 预算（ADR-0005）得到遵守：sealed 镜像增长约 0.2 KiB；链接器继续检查镜像大小和最小栈。
- HIL FEL 和 HIL SD 必须确认此前的日志和 handoff；在运行之前不作此声明。
- `GLOSSARY.md` 中新增术语“sunxi-drivers”，以免将驱动 crate 与平台层 `sunxi-pl` 混淆。

> 更新 #39/#45：FAT 适配器在 `write` feature 之后具备写入能力；`sunxi-drivers` 不属于 `default-members`，而驱动的 host 测试位于该 crate 自身，运行在 mock 块磁盘上。参见 ADR-0015。
