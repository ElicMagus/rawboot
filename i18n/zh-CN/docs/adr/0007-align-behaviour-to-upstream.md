# 将 loader 行为对齐到 upstream awboot，保留我们自己的 Rust 表达

loader 的硬件逻辑对齐到 `szemzoa/awboot` 的*行为*，锚定在一个固定的
upstream 提交上，而不是其代码：我们匹配步骤顺序、常量、分支结构和查找
表，但在 CC-BY-4.0（ADR-0001）下保留我们自己的表达。命名采用 upstream 的
`snake_case`，以便共享词汇得以延续；类型、trait、`Result`/`?` 和寄存器
访问器保持地道 Rust。

范围外：upstream 的 Linux/FDT/PSCI/secure-monitor/FatFs 路径和非 T113-S3 SoC 路径
（T113-S4、V851s），我们的自定义固件契约（ADR-0002）均不使用这些。仅重新对齐我们
已有的驱动；我们缺少的 upstream 驱动（DMA、SPI-NAND、watchdog、cache/exception
helpers）推迟到某个特性需要它们时。parity 通过对 T113-S3 上针对差异清单的
硬件在环冒烟测试验证。

## 命名

upstream 名称适用于函数、常量、寄存器字段和行为。Rust 类型、结构体
字段和枚举变体是我们的：`SdhciCmd.idx` 是 upstream `cmd.idx` 的地道 Rust 写法，
包装字段可以按其含义命名，而不是照抄 C 结构体。
当移植的类型形状改变时，重命名字段不构成 parity 违规。

## 工单边界

一个 parity 工单可以使其重新对齐的驱动的直接调用者适配新 API，并修复
使其自身验收标准可测试所需的阻塞 bug。这些调用者的完全 parity
仍属于其自己的工单；此类适配不是范围蔓延。
