# loader 常驻 SRAM；payload 镜像为带极小头的原始二进制

loader 链接在 `0x00020000`（boot）/ `0x00028000`（FEL），必须保持在 64 KB SRAM
预算之内，因此它从不自我重定位：它直接将 payload 加载进 DRAM。payload
以原始二进制形式随固定 `#[repr(C)]` 头（magic、version、flags、entry、load
address、size、checksum）发布，而不是 ELF 或 FDT，这使 loader 保持小巧且与 payload 无关，
并符合“无 Linux 协议”的决策（ADR-0002）。若 SRAM 预算被突破，我们添加一个
显式的 DRAM 重定位阶段，而不是悄悄增长。

loader 的代码和数据保留在 SRAM，但在 DRAM 初始化之后它将自身栈移到
DRAM 顶部：FAT 路径需要超过镜像下方剩余几 KiB 的空间，而 SRAM 栈
溢出会悄悄破坏其后的只读数据。栈顶与 payload 在 handoff 时收到的是同一个，
因此该切换不会与更早的 handoff 冲突。
