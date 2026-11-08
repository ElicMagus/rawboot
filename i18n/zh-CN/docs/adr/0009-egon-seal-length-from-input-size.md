# eGON sealer 从输入大小而非头字段推导校验和长度

`seal`（`rawboot-image`）根据 loader 二进制的实际字节长度计算填充后的镜像长度，并在对带种子的校验和求和之前将该值写入 eGON 头的 `length` 字段。upstream 的 `mksunxi` 则从传入的头中读取 `length`，将其对齐，并对相应数量的字求和。两者只有在头中已经保存了真实大小时才一致。

我们有意识地偏离。我们的 runtime 放置一个固定的占位头（`length = 32768`），而真实的 loader 镜像已经超过了它（封装后的 FEL loader 约为 57 KiB），因此信任头字段会求和并声明比 BROM 必须加载的字节更少的字节——镜像的尾部会被忽略。从缓冲区推导长度不会与实际存在的字节脱节，并且 BROM 仍然接受结果，因为它恰好对头最终声明的 `length` 求和——也就是 `seal` 自己写入的值。

这是对 ADR-0007 parity 规则的有意例外：可观察行为（一个 BROM 有效的镜像）与 upstream 一致，但算法的输入不一致。替代方案——通过链接器符号将真实大小嵌入 `rawboot-rt`，如同 upstream `start.S` 用 `__spl_size` 所做的那样，并对头字段求和——被拒绝，因为它增加了更多机制却没有可观察的差异。如果将来有第二个 sealer 或 flash 工具必须产生与 `mksunxi` 字节完全相同的输出，请重新审视这一点。

后果：`seal` 从不读取头的 `length`；调用者不得将占位值视为有意义的。eGON 头布局及其常量仍然是 `rawboot-image::egon` 中的唯一共享定义，由 `rawboot-rt` 嵌入。
