# sunxi-drivers: shared drivers above sunxi-pl

The loader's drivers (`ccu`/`clk`, `dram`, `uart`, `sdhci`, `sdcard`, `fat`) lived inside the bin
crate `rawboot`. A second image (for example, the flasher from #34) could not reuse them without
copying code; a copy inevitably drifts from the loader. The lower layer — MMIO, GPIO and the
timer — had already been moved into `sunxi-pl` (ADR-0010), but the drivers above it remained in the
bootloader.

Decision: a `no_std` crate `sunxi-drivers` above `sunxi-pl` was introduced. It contains `ccu`,
`clk`, `dram`, `uart`, `sdhci`, `sdcard` and `fat` and carries no startup code, eGON header, linker
scripts or global allocator. `rawboot` depends on it and keeps for itself the boot orchestration
(`boot`), `BootInfo` and its own `print!`. The crate's public boundaries are: `Sdhci`, `Sdcard`,
`DramPara` (parameters with public fields), `fat::mount` with the block adapter
`fat::block::BlockDevice`/`fat::partition::Partition`; `fat::read_boot_files` remains the loader's
convenience wrapper over them.

Driver output goes through the narrow trait `console::Console`, not through the loader's `print!`:
the binary installs its sink (the loader directs it to USART0), and without a sink the drivers run
silently. This preserves `rawboot::print!` for boot logs and does not pull a specific UART into the
crate.

## Considered Options

- **A shared driver crate** (accepted): a single source of drivers for the loader and future
  images, following the already accepted pattern of `sunxi-pl` (ADR-0010) and `rawboot-image`. The
  crate can be included without pulling in `_start` and eGON.
- **Keep the drivers in `rawboot` and add a lib target** (rejected): the bin crate would mix boot
  policy (the `payload.bin`/`boot.cfg` paths, `BootInfo`, handoff) with the drivers, and the
  boundary of "what can be reused" would remain implicit.
- **A copy of the drivers in the second image** (rejected): the code drifts from the loader; fixes
  would have to be made twice.
- **The `log` facade instead of a custom trait** (rejected): driver messages pulled in `log`
  infrastructure and pushed the image past the 64 KiB SRAM budget. A `Console` trait with a single
  `#[inline(never)]` formatting path is cheaper and requires no global logger.

## Consequences

- `rawboot` no longer contains driver modules: `rawboot/src` keeps `main.rs`, `boot.rs` and
  `print.rs`. At the time of the decision `sunxi-drivers` was part of the bare-metal
  `default-members`, and host tests lived in `rawboot-image`, `rawboot-fel` and `xtask`.
- `fatfs` and `core_io` moved into `sunxi-drivers`' dependencies; `log` with `max_level_off`
  remains only to compile `fatfs`'s trace/debug calls into nothing.
- The 64 KiB SRAM budget (ADR-0005) is met: the sealed image grew by about 0.2 KiB; the linker
  keeps checking both the image size and the minimum stack.
- HIL FEL and HIL SD must confirm the previous log and handoff; this is not claimed before the run.
- `GLOSSARY.md` gains the term "sunxi-drivers" so the driver crate is not confused with the
  platform layer `sunxi-pl`.

> Update #39/#45: the FAT adapter can write behind the `write` feature; `sunxi-drivers` is no
> longer part of `default-members`, and the driver host tests live in the crate itself on a mock
> block disk. See ADR-0015.
