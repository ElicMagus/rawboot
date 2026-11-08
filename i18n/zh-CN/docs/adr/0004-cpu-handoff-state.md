# 在 BROM 的 CPU 状态下交接；无 TrustZone 转换或 PSCI

payload 在 CPU0 上以 BROM 留下的状态运行——ARMv7-A、安全、MMU 和缓存关闭、IRQ
被屏蔽——而 CPU1 保持停放，payload 负责 SMP 启动。原始 C 的 secure→non-secure
monitor（`nonsec_virt.S`、`psci_vectors.S`、secure-section 重定位）只是为了通过 SMC 服务
非安全 OS 而存在；自包含的微内核不需要它，因此我们移除整个
子系统，仅在需要时再加回。
