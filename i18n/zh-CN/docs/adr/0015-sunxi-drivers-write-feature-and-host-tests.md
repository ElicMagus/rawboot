# sunxi-drivers：write feature、default-members 和 host 测试

在 #39 之后，`sunxi-drivers` 学会了写入块和 FAT 文件。写入是 flasher（#42）所需的，但 loader 不需要：write-cache、创建/截断文件和 flush 的代码无法放入 64 KiB SRAM 预算（ADR-0005）。同时，写入的 host 测试不需要板子，逻辑上位于驱动附近，而不是相邻的 crates 中。

决策：写入被移到 `sunxi-drivers` crate 的 `write` feature 中。在该 crate 内部，它默认启用：该 crate 本身不是镜像，而其主要的写入使用者是 flasher 和 host 测试。loader 以 `default-features = false` 接入 `sunxi-drivers`；flasher 也是 `default-features = false`，但带有显式的 `write` feature。`sunxi-drivers` 从 `default-members` 中移除，而 `rawboot-flasher` 单独移出（ADR-0014）。对该 crate 的原因：将其作为根包构建会通过 feature unification 启用默认的 `write`，尽管 loader 请求 `default-features = false`。对 flasher 的原因：其显式的 `write` 在与 loader 一起构建时也会同样被统一。在两种情况下，写入都会进入 loader 并破坏 SRAM 预算下的链接。

驱动的 host 测试位于 `sunxi-drivers`，使用内存中的 `MockDisk`，实现块 trait `BlockIo`。这不是 SoC 或板子的模型：只替换块 I/O——sector 读写以及错误注入。SDHCI 寄存器、卡时序和电气特性不被模拟。“没有硬件模型”的边界仍然有效：mock 在字节和块契约上是允许的，但不描绘硬件。host 测试覆盖块适配器和 FAT 写入、文件比较以及分区解析。

## 考虑过的选项

- **`write` 默认启用，loader 显式关闭它**（已采用）：该 crate 不作为镜像构建，因此只有 loader 有风险；flasher 和 host 测试无需额外标志即可获得写入，而边界在 loader 的清单中可见。
- **`write` 默认关闭**（已拒绝）：每个写入使用者，包括 host 测试，都必须显式启用该 feature；忘记标志会导致令人困惑的编译错误或静默缩减的 API。
- **单独的写入 crate**（已拒绝）：块和 FAT 已经与读取共享；拆分将复制适配器并允许代码偏离。
- **将 `sunxi-drivers` 保留在 `default-members` 中**（已拒绝）：将该 crate 作为根包构建会启用默认的 `write`，而 feature unification 会将其拖入 loader。
- **在 host 测试中建模 SDHCI**（已拒绝）：复杂且脆弱；测试的是块契约，而真实卡留给 HIL。

## 后果

- `sunxi-drivers` 声明 `write` feature（默认启用）。loader 和 flasher 以 `default-features = false` 依赖该 crate；flasher 添加 `features = ["write"]`。
- `sunxi-drivers` 不属于 `default-members`：作为根包，它会通过 feature unification 启用其默认的 `write`，而 `cargo build --release` 或 `cargo clippy` 会将写入拖入 loader。`scripts/check.sh` 通过单独调用对两种 feature 配置进行 lint，而 host 测试使用显式的 `--target x86_64-unknown-linux-gnu` 运行。
- 块适配器和 FAT 写入的 host 测试位于 `sunxi-drivers`，在 `MockDisk` 上无需板子运行；`rawboot-image`、`rawboot-fel` 和 `xtask` 保留各自的 host 测试。
- 规范的 Testing Decisions 允许在字节和块接缝处使用 mock，并明确说明仍然不出现 SoC 或板子模型。
- ADR-0012 关于 `default-members` 组成和 host 测试位置的后果被视为由本 ADR 进一步明确。
