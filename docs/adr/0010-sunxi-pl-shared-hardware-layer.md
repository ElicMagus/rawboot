# sunxi-pl: a shared hardware layer for the loader, runtime and payload

The hardware code needed by both the loader and the payload (MMIO, GPIO, timer) is moved into a
shared no_std crate `sunxi-pl` — a CMSIS-like platform layer without startup (`_start`, eGON
header, panic). The loader and `rawboot-rt` take it directly (the loader through facade
re-exports), and the payload stops copying GPIO constants and timer reads. The crate is intended as
the foundation for the payload application (kernel), so its API is still raw and parity-oriented; a
typed HAL is separate future work.

## Considered Options

- **A shared crate** (accepted): a single source of hardware code for three consumers, following
  the already accepted pattern of `rawboot-image` (`no_std`, `default-features = false`).
- **A feature in `rawboot-rt`** that disables `_start` and the eGON header (rejected): it mixes the
  hardware layer with the startup layer and still requires moving `gpio`/`io` from the loader into
  the runtime.
- **Keep a copy in the payload** (rejected): the copy is small but drifts from the source.
