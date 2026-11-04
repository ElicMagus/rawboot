# Loader stays resident in SRAM; payload image is a raw binary with a tiny header

The loader is linked at `0x00020000` (boot) / `0x00028000` (FEL) and must stay inside the 64 KB SRAM
budget, so it never relocates itself: it loads the payload straight into DRAM instead. The payload
ships as a raw binary behind a fixed `#[repr(C)]` header (magic, version, flags, entry, load
address, size, checksum) rather than ELF or FDT, which keeps the loader small and payload-agnostic
and matches the "no Linux protocol" decision (ADR-0002). If the SRAM budget is ever blown we add an
explicit DRAM relocation stage rather than silently growing.

The loader's code and data stay in SRAM, but after DRAM initialisation it moves its own stack to the
top of DRAM: the FAT path needs more than the few KiB left below the image, and an SRAM stack
overflow silently corrupts the read-only data that follows it. The stack top is the same one the
payload receives at handoff, so the switch cannot collide with an earlier handoff.
