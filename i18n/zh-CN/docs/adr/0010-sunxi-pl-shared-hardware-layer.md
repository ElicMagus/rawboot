# sunxi-pl：loader、runtime 和 payload 的共享硬件层

loader 和 payload 都需要的硬件代码（MMIO、GPIO、定时器）被移入共享的 no_std crate `sunxi-pl`——一个类似 CMSIS 的平台层，不含启动代码（`_start`、eGON 头、panic）。loader 和 `rawboot-rt` 直接使用它（loader 通过门面重导出），payload 不再复制 GPIO 常量和定时器读取。该 crate 被设想为 payload 应用（内核）的基础，因此 API 目前还是原始的对齐版本；类型化 HAL 是未来单独的工作。

## 考虑过的选项

- **共享 crate**（已采用）：三个使用者的单一硬件代码来源，延续了已被采用的 `rawboot-image` 模式（`no_std`、`default-features = false`）。
- **`rawboot-rt` 中的 feature**，用于关闭 `_start` 和 eGON 头（已拒绝）：将硬件层与启动层混在一起，而且仍然需要把 `gpio`/`io` 从 loader 迁移到 runtime。
- **在 payload 中保留副本**（已拒绝）：副本很小，但会偏离来源。
