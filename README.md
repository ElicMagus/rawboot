[English](README.md) | [简体中文](i18n/zh-CN/README.md) | [Русский](i18n/ru/README.md)

# rawboot

A second-stage bootloader for the Allwinner T113-S3 (ARMv7-A, dual Cortex-A7),
written in 100% Rust and ported from [`szemzoa/awboot`](https://github.com/szemzoa/awboot).
It brings up the SoC, reads a raw payload image from a FAT partition on an SD
card over SDHCI, loads it into DRAM, and hands off control through a versioned
`BootInfo` contract. The payload is our own firmware, not Linux: there is no
FDT, no PSCI and no initrd.

## Workspace

Cargo workspace members:

- `rawboot` — the loader (SoC bring-up, storage, handoff)
- `rawboot-rt` — the runtime (eGON header, `_start`, linker script)
- `sunxi-pl` — the shared platform layer (MMIO, GPIO, timer) used by loader, runtime and payload
- `sunxi-drivers` — the shared drivers above `sunxi-pl` (clock, DRAM, UART, SDHCI, SD/MMC, FAT)
- `payload` — the blink test payload image the loader boots
- `rawboot-image` — host-testable payload/eGON image, SD boot prefix and flash contract
  (`build`, `parse`, `seal`, `verify_sealed`, `build_sd_image`, `FlashJob`, `FlashStatus`,
  `FlashPlan`, `check_sd_layout`)
- `rawboot-fel` — host-side FEL USB transport and CLI (`version`, `read`, `write`, `exec`,
  `bringup`), plus `flash_sd_card` for the in-board update path
- `rawboot-flasher` — the FEL-callable DDR-init stub and DRAM-linked flasher; it writes the
  payload files and SD loader over FEL
- `xtask` — host build tooling (`image`, `payload`, `flasher`, `sd-image`, `fel-flash`)

Target: `armv7a-none-eabi` (soft-float), Rust edition 2024, pinned by
`rust-toolchain.toml`.

## Build and test

```sh
cargo build --release
```

Run the host tests (the workspace default target is bare-metal, so the host
triple is explicit):

```sh
cargo test -p rawboot-image -p rawboot-fel -p xtask -p sunxi-drivers \
  --target x86_64-unknown-linux-gnu
```

Seal the loader images:

```sh
cargo xtask image
```

The command writes `target/rawboot-fel.bin` for FEL and
`target/rawboot-boot-sd.bin` for SD/SPI boot. `scripts/check.sh` runs the full
local check suite: formatting, lint, builds, host tests and image smoke builds.

To update a card left in the board, put the board into FEL and run
`cargo xtask fel-flash --yes`; it builds or reuses the images, writes
`payload.bin` and the loader, leaves the MBR and FAT partition in place and
reads everything back. `cargo xtask fel-flash --dry-run` validates and prints
the plan without opening FEL.

See `GLOSSARY.md` for terminology and `docs/adr/` for architecture decisions.

## Licence

This project is released under the
[Creative Commons Attribution 4.0 International licence](https://creativecommons.org/licenses/by/4.0/)
(SPDX: `CC-BY-4.0`). See [`LICENSE`](LICENSE) for the full legal text. Reuse
and modification are allowed with attribution.

## Attribution

This project was developed with the help of an AI agent, DeepSeek V4.1 Flash,
and the [matt-pocock-skills](https://github.com/mattpocock/skills) skill set. It
was inspired by [`szemzoa/awboot`](https://github.com/szemzoa/awboot).
