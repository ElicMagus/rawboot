# sunxi-drivers: the write feature, default-members and host tests

After #39 `sunxi-drivers` learned to write blocks and FAT files. Writing is needed by the flasher
(#42) but not by the loader: the write-cache code, file creation/truncation and flush do not fit in
the 64 KiB SRAM budget (ADR-0005). At the same time the write host tests do not require a board and
logically live next to the drivers rather than in neighboring crates.

Decision: writing is moved into the `write` feature of the `sunxi-drivers` crate. Within the crate
it is enabled by default: the crate itself is not an image, and its main write consumers are the
flasher and the host tests. The loader depends on `sunxi-drivers` with
`default-features = false`; the flasher too with `default-features = false`, but with the explicit
`write` feature. `sunxi-drivers` was removed from `default-members`, and `rawboot-flasher` was moved
out separately (ADR-0014). The reason for the crate: building it as a root package enables its
default `write` through feature unification, even though the loader asks for
`default-features = false`. The reason for the flasher: its explicit `write` is unified the same way
when built together with the loader. In both cases writing ends up in the loader and breaks linking
against the SRAM budget.

The driver host tests live in `sunxi-drivers` and use an in-memory `MockDisk` implementing the block
trait `BlockIo`. This is not a model of the SoC or the board: only block I/O is substituted —
reading and writing sectors plus error injection. SDHCI registers, card timings and electronics are
not emulated. The boundary "no hardware models" remains in force: mocks are allowed at byte and
block contracts but do not depict hardware. Host tests cover the block adapter and FAT writing, file
comparison and partition parsing.

## Considered Options

- **`write` enabled by default, the loader explicitly disables it** (accepted): the crate is not
  built as an image, so only the loader is at risk; the flasher and host tests get writing without
  extra flags, and the boundary is visible in the loader's manifest.
- **`write` disabled by default** (rejected): every write consumer, including the host tests, would
  have to enable the feature explicitly; a forgotten flag would give an unclear compile error or a
  silently trimmed API.
- **A separate crate for writing** (rejected): blocks and FAT are already shared with reading;
  splitting would duplicate the adapter and let the code drift.
- **Leave `sunxi-drivers` in `default-members`** (rejected): building the crate as a root package
  would enable the default `write`, and feature unification would pull it into the loader.
- **Model SDHCI in host tests** (rejected): difficult and fragile; the block contract is tested, and
  the real card remains with HIL.

## Consequences

- `sunxi-drivers` declares the `write` feature (enabled by default). The loader and the flasher
  depend on the crate with `default-features = false`; the flasher adds `features = ["write"]`.
- `sunxi-drivers` is not part of `default-members`: as a root package it would enable its default
  `write` through feature unification, and `cargo build --release` or `cargo clippy` would pull
  writing into the loader. `scripts/check.sh` lints both feature configurations with separate
  invocations, and host tests are run with an explicit `--target x86_64-unknown-linux-gnu`.
- Host tests for the block adapter and FAT writing live in `sunxi-drivers` and run on `MockDisk`
  without a board; `rawboot-image`, `rawboot-fel` and `xtask` keep their own host tests.
- The spec's Testing Decisions allow mocks at byte and block seams and clarify that SoC or board
  models still do not appear.
- The Consequences of ADR-0012 about the composition of `default-members` and the place of host
  tests are considered clarified by this ADR.
