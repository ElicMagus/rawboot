# 在 CC-BY-4.0 下将 awboot 移植为 100% Rust

`szemzoa/awboot` 是面向 T113-S3 的 C 引导加载器；我们的后继者是干净的 100% Rust 重写
（无 C，连 DRAM init 都不用 C，DRAM init 已用 Rust）。我们只从 awboot
和数据手册中转录*事实*——地址、寄存器偏移、位位置、常量名——并自己以地道的方式
编写行为（自有类型、trait、`&raw` 访问器）。该移植以
`CC-BY-4.0`（SPDX `CC-BY-4.0`）发布，该许可允许复用和修改但要求
署名。我们刻意避免从 awboot 的 GPL-2.0+/GPL-2.0-only 和
无许可文件中复制代码正文，因为我们的许可无法使这些条款安全。
