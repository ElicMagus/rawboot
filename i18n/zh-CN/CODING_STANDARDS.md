# 编码标准

在代码评审期间阅读本文件。`scripts/check.sh` 强制执行机械规则（格式、clippy、主机测试、SPDX、
分支文档、特性图、冒烟构建）；本文件用于 linter 无法决定的判断。仓库的领域词汇表是 `GLOSSARY.md`；
决策在 `docs/adr/` 中；loader 契约是 `docs/spec-rawboot-v1.md`。

## 错误与状态

- 保留失败的阶段。如果调用方需要针对 create、truncate、write 和 flush 采取不同操作，不要把它们
  合并成一个 `failed` 结果。
- 返回调用方可以打印的上下文：哪个镜像、文件、步骤或范围失败了。
- 当调用方或测试会根据具体情形分支时，错误名称应是领域名称（`LoaderNotSealed`、
  `FlashLayoutError::PartitionTooEarly`），而不是裸字符串。
- `Drop` 中不得有可失败的副作用。如果清理可能失败，暴露一个返回错误的显式 `flush`/`unmount`。
  仅当在类型上有文档说明且不会改变成功报告时，才允许尽力而为的 `Drop`。

## 地址、大小与常量

- 地址、签名、扇区大小、时钟值和限制使用所属模块的常量。调用点不得出现字面量 `0x55aa`、`512`、
  `0x4000_0000` 或时钟数字。
- 跨 crate 的契约地址位于 `rawboot-image`（`flash`、`fel`、`payload`）；驱动使用各自的模块
  （`fat::partition`、`sdhci`、`sdcard`）。
- 新的默认工作区成员不得启用 `sunxi-drivers/write`。在归咎于链接器之前先运行
  `scripts/check-features.sh`；loader 的 64 KiB SRAM 预算是硬性契约。

## API 形状

- 超过两个同类型参数，或一起传递的一组参数，应改为具名的 request/plan 结构体（`FlashSdRequest`、
  `FlashPlan`）。位置参数切片容易导致转置错误。
- 保持 `no_std`/no-alloc 边界明确：调用方拥有的缓冲区，驱动或 flasher 代码中不得有隐藏分配。
- 优先使用共享构造函数，而不是重复结构体字面量；如果类型有不变量，构造函数是声明它的地方。

## 写入与回读

- 只有回读并比较写入的字节后，写入才算成功。用失败的步骤报告过短或损坏的写入。
- 绝不要写入预期目标之外的 MBR 或分区。第一次写卡之前会运行布局检查。
- 当部分失败有影响时，保持顺序：payload 文件在 boot loader 之前，MBR 最后（或永不）。

## 测试

- 当两个测试组装同一序列时，提取共享 fixture 或 step-builder。必须编辑两次的副本是迟早会出问题的
  bug。
- 在接触设备之前测试拒绝路径：过大的输入、未封装的 loader、不安全的布局、缺失的 DRAM。
- 对于字节契约，测试每个字段和每个被拒绝的位：magic、版本、大小、校验和、截断、对齐。
- 主机测试覆盖纯逻辑和脚本化传输；HIL 覆盖卡、LED 和冷启动。没有冷启动运行，就不要声称 HIL
  结果。

## 契约与文档

- 添加或重命名步骤/结果常量时，在同一次变更中更新名称表、主机打印器、ADR 列表和 runbook。
- 契约文档（`docs/spec`、`GLOSSARY`、关于公共 API 或 `default-members` 的 ADR）随代码分支走。
  Runbook、README 和 AGENTS 直接进入 master。
- 保持单一事实来源：链接到 runbook，而不是在 AGENTS 中重述其流程。

## 手动 HIL

- 每次冷启动只运行一次 DDR-init stub。同一电源周期内的重复运行不是经过验证的路径：HIL #52 看到它
  通过数次，然后在一次成功运行后挂起（USB 传输超时）。条件表位于 `docs/agents/hil.md`。在
  `fel-flash` 失败或成功之后，重试前请断电重启。
- 每个物理步骤问一个问题，并包含明确的确认；不要合并多个周期。
- 为 flasher 路径捕获 UART（`scripts/hil-flash.sh`）；仅靠状态步骤无法指出 `CMD24` 超时等驱动层
  失败。
