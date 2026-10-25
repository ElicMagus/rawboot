# rawboot — Allwinner T113-S3 bootloader

A second-stage bootloader written in Rust for the Allwinner T113-S3 (ARMv7-A, dual Cortex-A7),
ported from `szemzoa/awboot` but idiomatic Rust. It brings up the SoC, loads a custom firmware
image, and hands off control.

## Language

**BROM**:
Allwinner's on-chip first-stage boot ROM; it loads and validates an eGON image from SD (sector 16,
fallback 256), SPI flash, or USB FEL.

**FEL**:
Allwinner's USB recovery/boot mode; a host tool uploads an image into SRAM/DRAM and starts it.

**FEL transport** (crate `rawboot-fel`):
Our host-side implementation of the BROM's FEL USB protocol: device discovery, version and chip
ID, and memory read/write/exec, with a CLI for manual bring-up. `xfel` remains the external tool
for the HIL script.
_Avoid_: FEL library, xfel clone

**FEL mailbox**:
A four-word block in FEL-mode SRAM (magic `RBFM`, payload image address, image size, argument
pointer) that the host writes before starting the loader over FEL. When present, the loader boots
the uploaded payload image instead of reading `payload.bin` from the SD card.

**eGON**:
Allwinner's boot-image header (magic `eGON.BT0`, seed-checksummed with `0x5F0A6C39`) prepended to a
stage binary so BROM accepts it.

**loader** (crate `rawboot`):
Our second-stage program: initialises the SoC, reads the payload from storage, and hands off.
_Avoid_: app, SPL

**runtime** (crate `rawboot-rt`):
The startup layer that lays down the eGON header, sets the stack, clears BSS/copies data, and calls
the loader's `main`.
_Avoid_: framework, OS, HAL

**`sunxi-pl`** (platform layer):
The shared hardware layer for the loader, `rawboot-rt` and payload: MMIO, GPIO, timer. CMSIS-like,
carries no startup code.
_Avoid_: HAL, driver crate, platform

**`sunxi-drivers`** (driver crate):
The Allwinner T113-S3 drivers above `sunxi-pl`: clock/CCU, DRAM, UART, SDHCI, SD/MMC and the
read-only FAT adapter. Shared by the loader and future images; carries no startup code, eGON
header, linker script or global allocator, and reports progress through the `Console` trait so it
does not depend on the loader's `print!`.
_Avoid_: HAL, BSP, platform layer

**payload**:
The image the loader boots — the user's own firmware (possibly a microkernel), including a trivial
test image such as a blink loop. Not Linux.
_Avoid_: kernel, zImage

**image header**:
The fixed `#[repr(C)]` header at the front of a payload image (magic, version, flags, entry, load
address, size, checksum) that the loader parses.

**SD boot prefix**:
The host-built `target/rawboot-sd-mbr.img`: an MBR pointing at an existing FAT partition plus the
sealed SD/SPI loader at sector 16, exactly as long as the partition start (1 MiB by default). It
carries no FAT volume and no payload, and is written over a card with `dd conv=notrunc`; formatting
and `payload.bin` stay on the partition.
_Avoid_: SD image, full image

**BootInfo**:
The versioned `#[repr(C)]` descriptor the loader places in DRAM and passes to the payload in `r0`
at handoff.

**boot.cfg**:
Optional file on the FAT volume whose contents the loader copies into DRAM as a NUL-terminated
string and exposes through `BootInfo.args`; absent means a null `args` pointer.

**handoff**:
Transferring control from the loader to the payload, in a defined CPU and register state.

**DRAM init**:
Bringing up the T113 DDR3 controller, without which DRAM is unusable.

**upstream**:
The C project `szemzoa/awboot` that `rawboot` is ported from; the behaviour we align to.
_Avoid_: original, reference, vendor

**parity**:
Behavioural equivalence with upstream — same step order, constants, branches and tables — reached
with our own Rust expression.
_Avoid_: port, match, fidelity

**regs**:
Named MMIO register constants and structs, transcribed from the T113 datasheet and awboot.
