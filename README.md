# rawboot-internal — rawboot

A second-stage bootloader for the Allwinner T113-S3 (ARMv7-A, dual Cortex-A7),
written in Rust and ported from [`szemzoa/awboot`](https://github.com/szemzoa/awboot).
It brings up the SoC, reads a raw payload image from a FAT partition on an SD
card over SDHCI, loads it into DRAM, and hands off control through a versioned
`BootInfo` contract. The payload is our own firmware, not Linux.

Cargo workspace:

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

To update a card left in the board, put the board into FEL and run
`cargo xtask fel-flash --yes`; it builds or reuses the images, writes `payload.bin` and the
loader, leaves the MBR and FAT partition in place and reads everything back.
`cargo xtask fel-flash --dry-run` validates and prints the plan without opening FEL. See
`docs/agents/hil.md` for the safety checks, the UART log and the cold-boot check; in-board HIL
uses `scripts/hil-flash.sh`.

See `GLOSSARY.md` for terminology and `docs/adr/` for architecture decisions.
