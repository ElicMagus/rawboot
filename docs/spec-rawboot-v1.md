> Status: draft. To be published as a Gitea issue and labelled `ready-for-agent`.

## Problem Statement

The T113-S3 bootloader (the user's `rawboot-internal`) is a partial Rust port of the C `szemzoa/awboot`.
It already initialises clocks, UART, GPIO and DRAM, but: its SD/SDHCI path does not work; it is
shaped around Linux (zImage/FDT/initrd) even though the payload will be the user's own firmware; it
still carries un-named magic numbers; and its only build path is Windows `.bat` scripts wrapping
prebuilt `.exe` tools (`checksum.exe`, `dd.exe`, `xfel.exe`). The user wants a clean, idiomatic,
100%-Rust second-stage loader that boots their own firmware (not Linux) from an SD card on the
T113-S3, built and unit-tested on Linux, without losing the existing Windows path.

## Solution

Rework the project into a Cargo workspace — a loader crate (`rawboot`) and a runtime crate
(`rawboot-rt`) — that brings up the T113-S3, reads a raw payload image from a FAT partition on an SD
card over SDHCI, loads it into DRAM, and hands off control through a small versioned `BootInfo`
contract. Register layouts are transcribed as named constants from the T113 datasheet (and awboot
as a reference), never as magic numbers. A cross-platform Rust xtask seals images with the
Allwinner eGON header so BROM accepts them, on Linux as well as Windows. A trivial blink payload
proves the whole chain (SD → FAT → DRAM → handoff) on real hardware.

## User Stories

1. As a firmware developer, I want to build the loader on Linux with stable Rust, so that I no
   longer depend on Windows or prebuilt `.exe` tools.
2. As a firmware developer, I want runtime and loader in one Cargo workspace, so that they build
   together with one command.
3. As a firmware developer, I want register definitions as named constants, so that I never read a
   bare magic number again.
4. As a firmware developer, I want the loader to stay 100% Rust, so that I have no C to maintain.
5. As a firmware developer, I want the loader to initialise DRAM, so that payloads can run from
   DRAM address space.
6. As a firmware developer, I want the loader to initialise UART, so that I can read boot logs.
7. As a firmware developer, I want the loader to drive the board LED, so that I get a life sign
   without a serial cable.
8. As a firmware developer, I want the loader to read a FAT partition on the SD card, so that I can
   drop a payload file on the card instead of raw-flashing an image.
9. As a firmware developer, I want SDHCI/SD to work reliably, so that the SD card is the primary
   boot medium.
10. As a firmware developer, I want a fixed payload filename, so that the boot flow is predictable.
11. As a firmware developer, I want an optional `boot.cfg` on the FAT partition, so that I can pass
    arguments to the payload without rebuilding the loader.
12. As a firmware developer, I want the loader to honour the load address in the payload's image
    header, so that the payload controls where it is placed.
13. As a firmware developer, I want a checksum in the image header, so that a corrupted payload is
    rejected instead of executed.
14. As a firmware developer, I want a versioned `BootInfo` struct, so that the handoff contract can
    evolve without breaking old payloads.
15. As a firmware developer, I want the CPU and register state at handoff documented, so that I can
    write a payload's startup sequence.
16. As a firmware developer, I want the loader to set up a stack for the payload, so that a minimal
    payload needs no startup code at all.
17. As a firmware developer, I want a blink test payload, so that I can prove end-to-end handoff on
    real hardware.
18. As a firmware developer, I want to boot via FEL, so that I can iterate fast without touching the
    SD card.
19. As a firmware developer, I want eGON sealing implemented in a Rust xtask, so that images are
    BROM-valid on Linux.
20. As a firmware developer, I want the existing Windows build scripts kept, so that I can still
    build on Windows when needed.
21. As a firmware developer, I want host unit tests for image sealing and header parsing, so that
    format bugs are caught without a board.
22. As a firmware developer, I want compile-time layout assertions on `BootInfo`, so that an ABI
    break cannot ship silently.
23. As a firmware developer, I want clippy and CI, so that the port stays clean as it grows.
24. As a firmware developer, I want a best-effort JTAG debug path, so that I can debug when UART is
    not enough.
25. As a firmware developer, I want SoC-specific code isolated behind register/driver modules, so
    that another Allwinner SoC could be added later without a rewrite.
26. As a firmware developer, I want FDT and PSCI deliberately left out, so that the loader stays
    small and payload-agnostic.
27. As a firmware developer, I want the loader released as public domain, so that it can be reused
    freely.
28. As a firmware developer, I want the soft-float target, so that no VFP dependency creeps into the
    loader.

## Implementation Decisions

- **Workspace shape.** The `rawboot-internal` repository is a single Cargo workspace with members
  `rawboot` (the loader), `rawboot-rt` (the runtime), `sunxi-pl` (the shared hardware layer),
  `sunxi-drivers` (the shared drivers above it: clock/CCU, DRAM, UART, SDHCI, SD/MMC and the
  read-only FAT adapter), `payload` (the blink test payload), `rawboot-image` (host-testable
  payload, eGON, SD boot prefix and flash job/status tooling), `rawboot-fel` (host-side FEL USB
  transport and CLI), `rawboot-flasher` (the FEL-callable DDR-init stub and DRAM-linked flasher)
  and `xtask` (host build tooling). Driver modules live in `sunxi-drivers`; `rawboot` keeps the
  boot orchestration and its `print!`, and payload-header parsing and eGON sealing are
  `rawboot-image`. The hardware layer shared by loader, runtime and payload — volatile accessors,
  GPIO and the timer — carries no startup code (ADR-0010); the driver crate carries no startup
  code, eGON header, linker script or allocator (ADR-0012). Domain docs (`GLOSSARY.md`,
  `docs/adr/`) live at the repository root.
- **Runtime crate.** Lays down the eGON header (magic `eGON.BT0`, branch word, header fields) as
  statics in a head section, defines `_start` (stack, BSS clear, data copy), and carries the linker
  script. Modernised to remove `static_mut_refs` hazards using `&raw` pointers.
- **Driver crate.** `sunxi-drivers` holds `clk`/`ccu`, `uart`, `dram`, `sdhci`, `sdcard` and
  `fat`. Driver progress goes through the `Console` trait; the loader installs a USART0 sink, and
  a binary that installs none still gets the driver behaviour, silently.
- **Loader modules.** `print` and `boot` (orchestration, `BootInfo`, handoff) remain in
  `rawboot`. MMIO constants and register layouts are defined next to the drivers that use them,
  not in a shared `regs` module. Payload-header parsing and eGON sealing live in the
  `rawboot-image` crate, which the loader depends on with `default-features = false`. The
  volatile accessors (`io`) and `gpio` live in `sunxi-pl` and are re-exported by the loader
  (ADR-0010).
- **eGON sealing algorithm.** Seed the header checksum field with `0x5F0A6C39`, then sum
  `length_aligned / 4` little-endian 32-bit words over the padded image (32-bit wraparound) and
  write the sum back; padding is 512 for block devices or 8192 for flash; when the BROM page size
  is larger than 2 KB, scatter each 2 KB logical page into a zero-filled page of that size.
- **Payload image header.** A fixed `#[repr(C)]` header at the front of the payload:
  `magic, version, flags, entry, load_addr (default 0x42000000), size, checksum`. The loader honours
  `load_addr` and verifies `checksum` before jumping. `flags` is reserved for future features such
  as compression or encryption.
- **`BootInfo`.** A versioned `#[repr(C)]` struct placed by the loader in DRAM and passed to the
  payload in `r0`: `magic, version, boot_medium, ram_size, image load address/size/entry, args
  pointer`. `r1` is zero.
- **Handoff state contract.** Enter the payload in ARM state, Supervisor (SVC) mode, MMU and caches
  off, IRQ/FIQ masked, `SP` pointing at a reserved DRAM stack. CPU0 is handed off exactly as BROM
  left it (secure); CPU1 stays parked and the payload owns SMP bring-up. No TrustZone transition
  and no PSCI.
- **No Linux protocol.** No FDT parsing/mutation, no zImage/initrd handling.
- **Residency.** The loader stays resident in SRAM (hard budget ≤ 64 KB) and never relocates
  itself; the payload is loaded straight into DRAM. If the budget is exceeded, an explicit DRAM
  relocation stage is added rather than silently growing.
- **Storage.** SD/eMMC is driven through the SDHCI (Synopsys DesignWare MSHC) controller; the FAT
  filesystem is provided by the `fatfs` crate over a block-device adapter, replacing the vendored
  2 MB ChaN FatFs.
- **Build tooling.** A cross-platform Rust xtask assembles and eGON-seals images (`rawboot-fel.bin`,
  `rawboot-boot-sd.bin`, `rawboot.img`). The loader is linked twice from the same code:
  `rawboot-fel.bin` at `0x00028000` for `xfel`, and `rawboot-boot-sd.bin` at `0x00020000` for
  BROM SD/SPI boot from sector 16. `cargo xtask sd-image --partition-sectors <n>` writes
  `rawboot-sd-mbr.img`, the SD boot prefix: an MBR pointing at the FAT partition starting at
  `--start-lba` (default 2048, 1 MiB) plus `rawboot-boot-sd.bin` at sector 16 (ADR-0011). It
  formats nothing, copies no payload and only writes a file. `cargo xtask flasher` builds the
  returning DDR-init stub and the DRAM-linked flasher (ADR-0013). The existing Windows scripts
  and `.exe` tools are kept as a working Windows path. `rawboot-fel` (CLI `cargo fel`) talks to
  the BROM's FEL USB protocol directly for manual bring-up; `cargo fel bringup` runs the stub,
  checks DRAM and reads a flash status. `xfel` stays the external tool used by
  `scripts/hil-check.sh`.
- **Toolchain.** Target `armv7a-none-eabi` (soft-float; measured as smaller than the hard-float
  variant for this code), Rust edition 2024, pinned by `rust-toolchain.toml`.
- **Licence.** Public domain via `Unlicense`, with SPDX headers in files. Only facts (addresses,
  offsets, constant names) are transcribed from awboot; GPL-2.0+/unlicensed awboot sources are not
  copied.
- **Debugging.** A best-effort `openocd` + `gdb` path over the USB-JTAG adapter; FEL is the primary
  fast-iteration load path, through either `xfel` or the in-tree `rawboot-fel` transport.

## Testing Decisions

- A good test exercises **external behaviour, not implementation details** — for this project that
  means byte-in/byte-out on the image tooling, and observable LED behaviour on hardware. There are
  no models of the SoC or board: the only mock is the scripted USB bulk transport behind the FEL
  transport trait (issue #40), which checks wire frames rather than emulating hardware.
- **Unit seam (host, `cargo test`):** the image tooling. `seal()` must produce a BROM-valid header
  (checksum over the padded image with the `0x5F0A6C39` seed, correct 512/8192 padding, correct
  2 KB→N KB page scattering); `parse()` must accept a well-formed header and reject bad magic, bad
  checksum, or truncated/oversized lengths. These are pure functions over `&[u8]`. The SD boot
  prefix is likewise byte-in/byte-out: MBR fields, loader placement, prefix length, and rejection
  of an unsealed, overlapping or overflowing input.
- **FEL transport seam (host, `cargo test`):** `rawboot-fel` frames the BROM's `AWUC`/FEL
  requests and validates its replies over a `Transport` trait. Scripted bulk frames cover
  version/read/write/exec, 64 KiB chunking, short transfers and transport failures; the board
  check remains the final word.
- **Compile-time seam:** `const` assertions on `BootInfo`/image-header size and field offsets, so an
  ABI break fails the build.
- **Hardware-in-the-loop acceptance seam:** build the blink payload with the xtask, boot the loader
  over FEL and over SD, and observe the blink pattern. This is the end-to-end acceptance test.
- **Prior art:** none yet in the repository; establishing a host `cargo test` target for the tooling
  crate is part of this work.

## Out of Scope

- FDT parsing/mutation; the Linux boot protocol; zImage/initrd.
- PSCI, secure→non-secure monitor, and SMP bring-up.
- SPI-NOR/NAND boot and the DMA engine (a later milestone); eMMC.
- T113-S4, V851s and other SoCs.
- Payload image encryption and alternative filesystems.
- The microkernel/firmware that will eventually be the payload.

## Further Notes

- SD currently does **not** work; repairing/validating the SDHCI + card-protocol path is the first
  blocking ticket. JTAG and a logic analyser are the intended tools for it.
- Keep the blink pattern simple but distinctive (e.g. a short-long cadence) so a human can tell
  "payload running" apart from any pre-boot LED activity.
- The research report at `research/awboot-rust-migration.md` and ADRs `0001`–`0006` are the primary
  sources for the decisions above.
