# Payload entry contract

At handoff the loader enters the payload in ARM state, Supervisor (SVC) mode, with MMU and caches
off and IRQ/FIQ masked; `r0` holds the address of `BootInfo` and `r1` is zero. The loader points
`SP` at a reserved DRAM stack so a minimal payload (such as the blink test image) starts with no
startup code of its own; the payload owns its memory map and may relocate `SP`. The payload's load
address is taken from its image header, defaulting to `0x42000000`. This fixed, documented contract
is what lets the blink test image prove the whole chain (SD → FAT → DRAM → handoff) end to end.
