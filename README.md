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
- `rawboot-image` — host-testable payload/eGON image and SD boot prefix tooling (`build`, `parse`,
  `seal`, `verify_sealed`, `build_sd_image`)
- `rawboot-fel` — host-side FEL USB transport and CLI (`version`, `read`, `write`, `exec`)

See `GLOSSARY.md` for terminology and `docs/adr/` for architecture decisions.
