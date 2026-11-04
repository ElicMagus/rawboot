# Hand off via a custom BootInfo, not the Linux protocol

The payload is our own firmware, not Linux, so the loader does not implement the Linux boot
protocol: no `zImage` magic checks, no FDT parsing/mutation, no initrd hand-off. Instead the loader
places a small versioned `#[repr(C)]` `BootInfo` struct in DRAM and passes its address in `r0` at
handoff, with MMU and caches off and IRQs masked. This is far less code than an FDT-based contract
and keeps the loader payload-agnostic; image encryption or richer metadata can be added later behind
a `BootInfo` version bump.
