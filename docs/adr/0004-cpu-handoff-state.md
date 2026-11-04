# Hand off in BROM's CPU state; no TrustZone transition or PSCI

The payload runs on CPU0 in the state BROM left it — ARMv7-A, secure, MMU and caches off, IRQs
masked — while CPU1 stays parked and the payload owns SMP bring-up. The C original's secure→non-secure
monitor (`nonsec_virt.S`, `psci_vectors.S`, secure-section relocation) exists only to serve a
non-secure OS through SMC; a self-contained microkernel does not need it, so we drop that whole
subsystem and add it back only on demand.
