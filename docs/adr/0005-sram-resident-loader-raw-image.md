# Loader stays resident in SRAM; payload image is a raw binary with a tiny header

The loader is linked at `0x00020000` (boot) / `0x00028000` (FEL) and must stay inside the 64 KB SRAM
budget, so it never relocates itself: it loads the payload straight into DRAM instead. The payload
ships as a raw binary behind a fixed `#[repr(C)]` header (magic, version, flags, entry, load
address, size, checksum) rather than ELF or FDT, which keeps the loader small and payload-agnostic
and matches the "no Linux protocol" decision (ADR-0002). If the SRAM budget is ever blown we add an
explicit DRAM relocation stage rather than silently growing.
