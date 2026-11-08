# AGENTS

## 项目

`rawboot` — 面向全志 T113-S3（ARMv7-A，双核 Cortex-A7）的第二阶段引导加载程序，100% 用 Rust
编写，移植自 `szemzoa/awboot`。它负责初始化 SoC，通过 SDHCI 从 SD 卡上的 FAT 分区读取原始 payload
镜像，将其加载到 DRAM，并通过带版本号的 `BootInfo` 契约执行 handoff。payload 是我们自己的固件，
而不是 Linux（没有 FDT，没有 PSCI）。

Cargo 工作区有九个成员：`rawboot`（loader）、`rawboot-rt`（runtime）、`sunxi-pl`（共享平台层：
MMIO、GPIO、定时器）、`sunxi-drivers`（位于 `sunxi-pl` 之上的共享驱动 crate：clock、DRAM、UART、
SDHCI、SD/MMC、FAT）、`payload`（loader 启动的闪烁测试 payload 镜像）、`rawboot-image`（可在主机上
测试的 payload/eGON、SD boot prefix 以及 flash job/status 工具）、`rawboot-fel`（主机端 FEL USB
transport 和 CLI）、`rawboot-flasher`（可由 FEL 调用的 DDR-init stub 和链接到 DRAM 的 flasher）以及
`xtask`（主机端构建工具）。`fatfs` 的 `no_std` `core_io` 依赖被补丁为 `vendor/core_io`，这是一个
最小的、兼容 stable 的 shim。目标平台 `armv7a-none-eabi`（软浮点），Rust edition 2024。

`embed/` 是旧 Rust 移植的被忽略快照，用于移植前对比；固定版本的 C upstream 由
`scripts/fetch-upstream.sh` 获取到 `target/upstream/` 中。请探索工作区 crate，而不是那些目录树。

搜索：使用 agent 的 Grep/Glob 工具；本机未安装 `rg`。

标准命令：

- 构建：`cargo build --release`（`xtask` 及其仅主机端的镜像工具被排除在裸机 `default-members` 之外；
  `rawboot-image` 仍作为 loader 的无分配依赖被构建。`rawboot-flasher` 也在 `default-members` 之外：
  它启用 `sunxi-drivers/write`，这会统一到 loader 中；`cargo xtask flasher` 会构建它）
- 测试（主机端、工具）：`cargo test -p rawboot-image -p rawboot-fel -p xtask -p sunxi-drivers --target x86_64-unknown-linux-gnu`
  （工作区默认目标是 `armv7a-none-eabi`，因此主机测试需要显式的 `--target`；包范围将仅硬件的
  裸机成员排除在主机构建之外。`sunxi-drivers` 有针对块适配器和基于模拟磁盘的 FAT 写入的主机单元
  测试；它们不需要板子。）
- Lint：`cargo clippy -- -D warnings`（`no_std` crate 没有主机测试目标，因此 `--all-targets` 无法在
  `armv7a-none-eabi` 上构建 libtest harness；`sunxi-drivers` 不是默认成员，所以它的两套特性集分别用
  `cargo clippy -p sunxi-drivers -- -D warnings` 和
  `cargo clippy -p sunxi-drivers --no-default-features -- -D warnings` 显式 lint；flasher 因为不是
  默认成员且启用了 write 特性，所以单独用 `cargo clippy -p rawboot-flasher -- -D warnings` lint；
  主机工具和可在主机上测试的驱动测试用
  `cargo clippy -p rawboot-image -p rawboot-fel -p xtask -p sunxi-drivers --target x86_64-unknown-linux-gnu --all-targets -- -D warnings`
  lint；`core_io` shim 用
  `cargo clippy --manifest-path vendor/core_io/Cargo.toml --target-dir target/core_io-shim -- -D warnings`
  lint）
- 格式化：`cargo fmt`（CI 运行 `cargo fmt --all -- --check`）
- 反汇编：`scripts/objdump.sh -d target/armv7a-none-eabi/release/rawboot`（封装 `llvm-tools` 组件中的
  `llvm-objdump`；没有 `cargo objdump`）
- 获取固定版本的 upstream：`scripts/fetch-upstream.sh`（将 parity 研究笔记使用的 C 树写入
  `target/upstream/awboot-<sha>/`）
- 检查所有内容：`scripts/check.sh`（fmt、早期的 `scripts/check-features.sh` 特性图守卫、构建、裸机、
  flasher、驱动和主机 clippy 通过、主机测试、SPDX 头、分支文档规则、loader 无写入符号守卫、
  `cargo xtask image`/`payload`/`flasher` 冒烟构建、`cargo xtask fel-flash --dry-run` 以及
  `cargo xtask sd-image`）
- 封装镜像：`cargo xtask image`（构建 loader 两次，并写出用于 FEL 的 `target/rawboot-fel.bin`（位于
  `0x00028000`）和用于 SD/SPI 启动的 `target/rawboot-boot-sd.bin`（位于 `0x00020000`）；`xtask`
  别名在 `.cargo/config.toml` 中固定主机三元组）
- 发布公共仓库：重写后的 `master` 历史（截至 `v0.1.0`）和标签会发布到 GitHub（ADR-0016）；旧的
  单提交 `scripts/export-public.sh` 已退役。一次性迁移和手动修复发布流程位于
  `docs/agents/export-public.md`
- 构建 payload 镜像：`cargo xtask payload`（构建 `payload` crate 并写出 `target/rawboot-payload.img`，
  其扁平二进制被包裹在 payload image header 中）
- 构建 FEL flasher 镜像：`cargo xtask flasher`（写出 `target/rawboot-ddr.bin`，即位于 `0x00028000`
  的会返回的 DDR-init stub，以及 `target/rawboot-flasher.bin`，链接到 DRAM 中的 `0x40010000`；两者
  都不携带 eGON 头；ADR-0013）
- 在板内刷写卡：`cargo xtask fel-flash [--payload <path>] [--boot-cfg <path>] [--loader <path>]
  [--flasher <path>] [--ddr-stub <path>] [--sequence <n>] [--skip-build] [--dry-run] [--yes]`
  （构建或复用镜像，检查已封装的 SD loader 并拒绝 FEL 变体，通过 FEL 写入 payload 文件和 loader 而
  不触碰 MBR，并回读所有内容。`--dry-run` 在不打开 FEL 的情况下验证并打印计划；实际写入需要
  `--yes`；ADR-0014；runbook `docs/agents/hil.md`）
- FEL CLI：`cargo fel version|read|write|exec|bringup ...`（主机端通过 USB 的 FEL transport，不依赖
  系统 `libusb`；`version` 识别 T113-S3，`read <addr> <len>` 以十六进制转储或写入 `--output`，
  `write <addr> <file>` 上传字节，`exec <addr>` 跳转，`bringup <ddr-stub> <flasher>` 运行会返回的
  stub，检查 DRAM pattern 并读取 flasher status；`fel` 别名固定主机三元组）
- 构建 SD boot prefix：`cargo xtask sd-image --partition-sectors <n> [--start-lba <n>]
  [--loader <path>]`（在 `cargo xtask image` 之后；写出 `target/rawboot-sd-mbr.img`，一个指向现有
  FAT 分区并加上位于扇区 16 的 SD loader 的 MBR，用于 `dd conv=notrunc` 到卡上；它不格式化任何
  内容，也不复制 payload；ADR-0011）
- 准备 SD 卡：`scripts/flash-sd.sh [--device /dev/sdX] [--copy-payload] [--format]`（以你的用户身份
  运行，不要通过 `sudo`；自动检测单个可移动/USB/MMC 磁盘，`--list` 显示候选，构建并写入 boot
  prefix，可选地仅格式化/重新填充分区；运行 `--dry-run` 而不触碰任何内容；拒绝已挂载、根、GPT
  或多分区磁盘；runbook `docs/agents/hil.md`）
- HIL：`docs/agents/hil.md` 是权威 runbook。在 `code-review` 之后，在代码分支上：
  `scripts/hil-check.sh target/rawboot-fel.bin <label>`；SD 抖动（#43）-> `scripts/hil-sd-retry.sh`；
  板内卡更新 -> `scripts/hil-flash.sh`（每次尝试一次冷启动：重复的 DDR-init stub 可能会挂起；参见
  runbook 中的条件表）。HIL #42 的 CMD24 阻塞项（#47）已修复（PR #49）。`scripts/check.sh` 仅覆盖
  主机检查。

## CI

`.gitea/workflows/ci.yml` 在每次 push 和 pull request 时运行 `scripts/check.sh`。它需要一个以
`ubuntu-latest` 标签注册的 `act_runner`；在注册之前，请在推送前手动运行该脚本。

## 工作约定

- **Git 历史语言为俄语，公共跟踪器为双语。** 标题、issue 正文、评论和提交信息用俄语撰写。发布到
  GitHub 归档时，英文翻译在上，俄语原文放在 `<details>` 中。与项目所有者沟通（包括问题、澄清和
  跟踪器评论）使用俄语。
- **公共 GitHub 是带有真实历史的镜像（ADR-0016）。** Gitea 仍是规范跟踪器和事实来源；GitHub 上
  发布带标签的 `master` 历史，以及全部代码、环境和决策。`.gitea/`、`scripts/export-public.sh`、
  `scripts/issue-view.sh`、`target/` 和 `embed/` 保持内部。未合并的分支不发布：先合并，再进行
  单独的手动操作。
- **文档：英文是首要来源。** 英文文件位于当前路径；翻译镜像到 `i18n/ru/` 和 `i18n/zh-CN/` 中的
  相同相对路径。agent 只阅读和编辑英文文件，不阅读翻译；如有冲突，以英文为准。API 名称、寄存器
  名称和词汇表术语不翻译；翻译草稿由 agent 自行完成，无需单独评审。
- **issue 的解决分支要推送，但不要创建 PR。** 解决问题后，创建分支
  （`feat/<N>-<slug>` / `fix/<N>-<slug>`），提交并推送到 `origin`。Pull request 只由项目所有者
  打开。没有 issue 的工具性修改（包括回顾）直接进入 `master`；只有 issue 或明确要求 PR 时才需要
  单独分支。
- **文档直接提交，不走分支。** ADR、`GLOSSARY.md`、`research/` 和 `docs/` 提交到当前分支（通常是
  `master`）；分支（`feat/`/`fix/`）用于代码。例外：没有它们合并就会改变契约的文档（关于模块
  边界、公共 API 或 `default-members` 的 spec/GLOSSARY/ADR）走代码分支。提交前，将新文档与旧 ADR、
  词汇表和代码核对；发现矛盾时，先进行 grilling（`grill-with-docs`），再提交。
- **推送前运行评审——skill `code-review`（Standards + Spec）。**
- **HIL 与文档的顺序。** 检查说明（runbook、命令）可以在 HIL 之前提交；关于结果的声明（PASS、
  确认的 LED）只能在 HIL 确认后提交。
- **手动 HIL 步骤通过对话进行。** 如果测试需要人工操作（power-cycle、插入卡、FEL/BOOT、确认
  LED），在每个步骤或循环之前通过 `question` 工具提出一个问题并等待回答。未经确认，不要启动下一
  个循环，也不要进入下一步；不要将多个循环合并到一个问题中。在问题中给出确切的物理过程和预期
  结果，运行后给出 PASS/FAIL、关键日志行和下一步。在一次 `question` 调用中提出两个问题选项卡：
  步骤本身和控制性的“确定吗？”，选项为“是”和“否”。仅当步骤已确认且在“确定吗？”中选择了“是”
  时才执行操作；否则返回步骤问题。这是为了防止误触。
- **推送后给出检查说明**——构建、通过 FEL 和/或 SD 启动以及观察 payload 的 LED 的步骤。
- **合并前列出 issue 的 acceptance。** 未完成的条目要么让 issue 保持打开，要么转移到带有 native
  blocking link 和明确标记的 follow-up issue 中。某一部分的 PASS（例如读取）不能替代另一部分的
  PASS（例如写入）。
- **确认后——将分支合并到 `master`，关闭 issue，删除分支。**
- **发现的缺陷应作为单独的 issue 提交**，并带有 `needs-triage` 标签。

## Agent 技能

### Issue 跟踪器

Issues 位于 Gitea 的 `ElicMagus/rawboot-internal`。参见 `docs/agents/issue-tracker.md`。

### 分诊标签

两个类别 + 五个状态角色，映射到此仓库的标签字符串。参见 `docs/agents/triage-labels.md`。

### 领域文档

单上下文。Spec 位于 `docs/spec-rawboot-v1.md`，研究笔记位于 `research/`，词汇表位于
`GLOSSARY.md`，决策位于 `docs/adr/`。参见 `docs/agents/domain.md`。

评审者的判断准则位于 `CODING_STANDARDS.md`；在 `code-review` 期间阅读它。
