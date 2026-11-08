# payload 入口契约

在 handoff 时，loader 以 ARM 状态、Supervisor（SVC）模式进入 payload，MMU 和缓存
关闭、IRQ/FIQ 被屏蔽；`r0` 保存 `BootInfo` 的地址，`r1` 为零。loader 将
`SP` 指向保留的 DRAM 栈，因此最小 payload（如 blink 测试镜像）无需自身的
启动代码即可开始；payload 拥有自己的内存映射，并可重定位 `SP`。payload 的加载
地址取自其镜像头，默认为 `0x42000000`。这个固定且文档化的契约
正是让 blink 测试镜像能够端到端证明整条链（SD → FAT → DRAM → handoff）的原因。
