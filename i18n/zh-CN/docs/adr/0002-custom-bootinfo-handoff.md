# 通过自定义 BootInfo 交接，而非 Linux 协议

payload 是我们自己的固件，不是 Linux，因此 loader 不实现 Linux 启动
协议：不检查 `zImage` magic，不解析/修改 FDT，不交接 initrd。取而代之，loader
在 handoff 时将一个小型带版本的 `#[repr(C)]` `BootInfo` 结构体放入 DRAM，并在
交接时于 `r0` 中传递其地址，此时 MMU 和缓存关闭、IRQ 被屏蔽。这比基于 FDT 的契约
代码少得多，并保持 loader 与 payload 无关；镜像加密或更丰富的元数据可以稍后通过
提升 `BootInfo` 版本在背后添加。
