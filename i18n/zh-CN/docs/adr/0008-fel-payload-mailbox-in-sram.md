# FEL payload mailbox 位于 SRAM，而非 DRAM

FEL 快速迭代路径允许主机通过 USB 上传 loader 和 payload 镜像，
无需 SD 卡。主机在 `xfel exec` 之前写入一个四字 mailbox；loader 在自身 DRAM init
之后读取它，并通过与 SD 路径相同的放置、校验和和
`BootInfo` handoff 引导已暂存的镜像。

mailbox 和 payload 镜像位于 FEL 模式 SRAM 的空闲部分（mailbox 在 `0x00044000`，
payload 从 `0x00039000` 开始，可选 NUL 结尾参数在 `0x00038000..0x00039000`），
magic 为 `RBFM`。upstream
awboot 将其 Linux mailbox 放在 DRAM 的 `0x43100000`，在 `xfel ddr` 之后。我们不能这样做：
主机必须在 loader 运行之前写入请求，而 loader 正是初始化 DRAM 的组件。
先运行 `xfel ddr` 目前会使 loader 自身的 DRAM init 挂起（issue
#23），因此 DRAM mailbox 要么需要预初始化 DRAM，要么需要修复该 bug。

后果：FEL payload 镜像必须容纳在约 44 KiB 的 SRAM 窗口内，这对
blink 镜像和快速测试足够；SD 路径仍是较大 payload 的主要启动介质。
此路径的 `BootInfo.boot_medium` 为 `2`（`1` 仍为 SD），`BootInfo.args` 携带
mailbox 参数指针或零。一旦 DRAM 初始化变为幂等，就可以在
`BootInfo` 版本提升或一个 mailbox 标志之后添加 DRAM 暂存窗口，而无需改变当前契约。
