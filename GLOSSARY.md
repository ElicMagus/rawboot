# rawboot — Allwinner T113-S3 bootloader

A second-stage bootloader written in Rust for the Allwinner T113-S3 (ARMv7-A, dual Cortex-A7),
ported from `szemzoa/awboot` but idiomatic Rust. It brings up the SoC, loads a custom firmware
image, and hands off control.

## Language

**BROM**:
Allwinner's on-chip first-stage boot ROM; it loads and validates an eGON image from SD (sector 16,
fallback 256), SPI flash, or USB FEL.

**FEL**:
Allwinner's USB recovery/boot mode; a host tool (`xfel`) uploads an image into SRAM/DRAM and
starts it.

**eGON**:
Allwinner's boot-image header (magic `eGON.BT0`, seed-checksummed with `0x5F0A6C39`) prepended to a
stage binary so BROM accepts it.

**loader** (crate `rawboot`):
Our second-stage program: initialises the SoC, reads the payload from storage, and hands off.
_Avoid_: app, SPL

**runtime** (crate `rawboot-rt`):
The startup layer that lays down the eGON header, sets the stack, clears BSS/copies data, and calls
the loader's `main`.
_Avoid_: framework, OS

**payload**:
The image the loader boots — the user's own firmware (possibly a microkernel), including a trivial
test image such as a blink loop. Not Linux.
_Avoid_: kernel, zImage

**image header**:
The fixed `#[repr(C)]` header at the front of a payload image (magic, version, flags, entry, load
address, size, checksum) that the loader parses.

**BootInfo**:
The versioned `#[repr(C)]` descriptor the loader places in DRAM and passes to the payload in `r0`
at handoff.

**handoff**:
Transferring control from the loader to the payload, in a defined CPU and register state.

**DRAM init**:
Bringing up the T113 DDR3 controller, without which DRAM is unusable.

**regs**:
Named MMIO register constants and structs, transcribed from the T113 datasheet and awboot.
