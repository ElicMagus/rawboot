// SPDX-License-Identifier: Unlicense

//! Payload placement, the `BootInfo` handoff descriptor, and the jump itself.
//!
//! The loader stages the payload image in DRAM (SD path) or SRAM (FEL path), validates it, copies
//! the payload to the `load_addr` from its header, and verifies the checksum at the destination. It
//! then builds [`BootInfo`] in a reserved area below the handoff stack and enters the payload
//! according to ADR-0006: ARM state, Supervisor mode, MMU and caches off, IRQ/FIQ masked, `r0`
//! pointing at `BootInfo` and `r1` zero.

use core::fmt;
use core::ptr;
use core::slice;

use rawboot_image::{
    FEL_ARGS_BASE, FEL_MAILBOX_BASE, FEL_MAILBOX_WORDS, FEL_PAYLOAD_BASE, FelMailbox,
    IMAGE_HEADER_SIZE, ImageError, image_checksum, parse_header,
};

use crate::dram::SDRAM_BASE;
use crate::fat::{self, FatError};
use crate::io::{read32, write32};
use crate::print;
use crate::sdcard::Sdcard_data;

const MIB: u32 = 1 << 20;

/// Bytes reserved for the payload's initial stack.
const HANDOFF_STACK_SIZE: u32 = 64 * 1024;

/// The `BootInfo` magic: `RBBI` in little-endian order.
pub(crate) const BOOT_INFO_MAGIC: u32 = u32::from_le_bytes(*b"RBBI");
/// The current `BootInfo` layout version.
pub(crate) const BOOT_INFO_VERSION: u32 = 1;
/// The payload was read from an SD/MMC card.
pub(crate) const BOOT_MEDIUM_SD: u32 = 1;
/// The payload was uploaded over FEL and read from SRAM.
pub(crate) const BOOT_MEDIUM_FEL: u32 = 2;

/// The versioned handoff descriptor, placed in DRAM and passed to the payload in `r0`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BootInfo {
    /// [`BOOT_INFO_MAGIC`].
    pub magic: u32,
    /// [`BOOT_INFO_VERSION`].
    pub version: u32,
    /// Where the payload came from: [`BOOT_MEDIUM_SD`] or [`BOOT_MEDIUM_FEL`].
    pub boot_medium: u32,
    /// Size of the initialised DRAM in bytes.
    pub ram_size: u32,
    /// Address the payload was copied to.
    pub image_load_addr: u32,
    /// Payload size in bytes, excluding the image header.
    pub image_size: u32,
    /// Address the payload starts executing at.
    pub entry: u32,
    /// NUL-terminated arguments string pointer, or zero when the source provides none.
    pub args: u32,
}

const BOOT_INFO_SIZE: u32 = core::mem::size_of::<BootInfo>() as u32;

mod boot_info_offset {
    pub const MAGIC: usize = 0;
    pub const VERSION: usize = 4;
    pub const BOOT_MEDIUM: usize = 8;
    pub const RAM_SIZE: usize = 12;
    pub const IMAGE_LOAD_ADDR: usize = 16;
    pub const IMAGE_SIZE: usize = 20;
    pub const ENTRY: usize = 24;
    pub const ARGS: usize = 28;
}

const _: () = {
    use core::mem::offset_of;
    assert!(BOOT_INFO_SIZE == 32);
    assert!(offset_of!(BootInfo, magic) == boot_info_offset::MAGIC);
    assert!(offset_of!(BootInfo, version) == boot_info_offset::VERSION);
    assert!(offset_of!(BootInfo, boot_medium) == boot_info_offset::BOOT_MEDIUM);
    assert!(offset_of!(BootInfo, ram_size) == boot_info_offset::RAM_SIZE);
    assert!(offset_of!(BootInfo, image_load_addr) == boot_info_offset::IMAGE_LOAD_ADDR);
    assert!(offset_of!(BootInfo, image_size) == boot_info_offset::IMAGE_SIZE);
    assert!(offset_of!(BootInfo, entry) == boot_info_offset::ENTRY);
    assert!(offset_of!(BootInfo, args) == boot_info_offset::ARGS);
};

/// Why a payload could not be handed off.
pub(crate) enum BootError {
    /// The payload file could not be read from the card.
    Fat(FatError),
    /// The image header is malformed or the payload checksum failed.
    Image(ImageError),
    /// The reported DRAM size leaves no room for the staging area and handoff stack.
    RamTooSmall,
    /// The DRAM top or the payload's address range overflowed a `u32`.
    AddressOverflow,
    /// The payload does not fit in the initialised DRAM.
    OutsideRam {
        load_addr: u32,
        size: u32,
        ram_size: u32,
    },
    /// The payload would overwrite its `BootInfo` or the handoff stack.
    OverlapsReserved {
        load_addr: u32,
        size: u32,
        reserved_start: u32,
    },
    /// The entry point is not inside the loaded payload.
    EntryOutsidePayload {
        entry: u32,
        load_addr: u32,
        size: u32,
    },
    /// The entry point selects Thumb state; the handoff contract is ARM state.
    EntryNotArm { entry: u32 },
    /// The image declares an empty payload.
    EmptyPayload,
    /// The FEL mailbox names an image outside the SRAM staging window.
    FelImageOutsideSram { addr: u32, size: u32 },
    /// The FEL mailbox argument string is outside its window or not NUL-terminated.
    ArgumentsInvalid { args: u32 },
}

impl fmt::Display for BootError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fat(err) => write!(f, "{err}"),
            Self::Image(err) => write!(f, "invalid payload image: {err}"),
            Self::RamTooSmall => f.write_str("DRAM is too small for the payload staging area"),
            Self::AddressOverflow => f.write_str("payload address range overflows"),
            Self::OutsideRam {
                load_addr,
                size,
                ram_size,
            } => write!(
                f,
                "payload 0x{load_addr:08x}+0x{size:x} does not fit in {ram_size} bytes of DRAM"
            ),
            Self::OverlapsReserved {
                load_addr,
                size,
                reserved_start,
            } => write!(
                f,
                "payload 0x{load_addr:08x}+0x{size:x} overlaps the reserved area at 0x{reserved_start:08x}"
            ),
            Self::EntryOutsidePayload {
                entry,
                load_addr,
                size,
            } => write!(
                f,
                "entry 0x{entry:08x} is outside payload 0x{load_addr:08x}+0x{size:x}"
            ),
            Self::EntryNotArm { entry } => write!(
                f,
                "entry 0x{entry:08x} is not word-aligned; the payload must start in ARM state"
            ),
            Self::EmptyPayload => f.write_str("payload image is empty"),
            Self::FelImageOutsideSram { addr, size } => write!(
                f,
                "FEL payload image 0x{addr:08x}+0x{size:x} is outside the SRAM staging window \
                 0x{FEL_PAYLOAD_BASE:08x}..0x{FEL_MAILBOX_BASE:08x}"
            ),
            Self::ArgumentsInvalid { args } => write!(
                f,
                "FEL argument string 0x{args:08x} is outside 0x{FEL_ARGS_BASE:08x}..0x{FEL_PAYLOAD_BASE:08x} \
                 or not NUL-terminated"
            ),
        }
    }
}

/// Reads the FEL payload mailbox, or returns `None` when the host did not write one.
///
/// A present mailbox is consumed immediately, so a warm reset that preserves SRAM cannot silently
/// boot a stale FEL image instead of falling back to the SD path.
pub(crate) fn fel_mailbox() -> Option<FelMailbox> {
    let mut words = [0u32; FEL_MAILBOX_WORDS];
    for (index, word) in words.iter_mut().enumerate() {
        *word = read32(FEL_MAILBOX_BASE + index as u32 * 4);
    }
    let mailbox = FelMailbox::from_words(words);
    if mailbox.is_some() {
        write32(FEL_MAILBOX_BASE, 0);
    }
    mailbox
}

/// Reads the payload from the SD card, validates it, places it and transfers control to it.
///
/// Returns only on failure: a successful handoff never comes back.
pub(crate) fn boot_sd(sd: &mut Sdcard_data, dram_size_mib: u32) -> BootError {
    finish(prepare_sd(sd, dram_size_mib))
}

/// Validates the payload image uploaded over FEL, places it and transfers control to it.
///
/// Returns only on failure: a successful handoff never comes back.
pub(crate) fn boot_fel(mailbox: FelMailbox, dram_size_mib: u32) -> BootError {
    finish(prepare_fel(mailbox, dram_size_mib))
}

fn finish(result: Result<Handoff, BootError>) -> BootError {
    match result {
        Ok(handoff) => handoff.enter(),
        Err(err) => err,
    }
}

fn prepare_sd(sd: &mut Sdcard_data, dram_size_mib: u32) -> Result<Handoff, BootError> {
    let layout = DramLayout::new(dram_size_mib).ok_or(BootError::RamTooSmall)?;

    // The whole image is staged at the bottom of DRAM. It is copied to `load_addr` afterwards;
    // `ptr::copy` (not `copy_nonoverlapping`) makes the copy safe when the ranges overlap.
    let (image_ptr, image_len) = {
        // SAFETY: DRAM is initialised and the slice ends below the reserved BootInfo/stack area.
        let staging =
            unsafe { slice::from_raw_parts_mut(SDRAM_BASE as *mut u8, layout.staging_len) };
        let len = fat::read_payload(sd, staging).map_err(BootError::Fat)?;
        (staging.as_ptr(), len)
    };

    // SAFETY: `fat::read_payload` filled the first `image_len` bytes of the staging area.
    let image = unsafe { slice::from_raw_parts(image_ptr, image_len) };
    prepare(image, BOOT_MEDIUM_SD, 0, &layout)
}

fn prepare_fel(mailbox: FelMailbox, dram_size_mib: u32) -> Result<Handoff, BootError> {
    let layout = DramLayout::new(dram_size_mib).ok_or(BootError::RamTooSmall)?;

    let image_end = mailbox
        .image_addr
        .checked_add(mailbox.image_size)
        .ok_or(BootError::AddressOverflow)?;
    if mailbox.image_addr < FEL_PAYLOAD_BASE || image_end > FEL_MAILBOX_BASE {
        return Err(BootError::FelImageOutsideSram {
            addr: mailbox.image_addr,
            size: mailbox.image_size,
        });
    }
    if mailbox.args_addr != 0 && !args_addr_valid(mailbox.args_addr) {
        return Err(BootError::ArgumentsInvalid {
            args: mailbox.args_addr,
        });
    }

    // SAFETY: the mailbox range was checked against the SRAM staging window above.
    let image = unsafe {
        slice::from_raw_parts(mailbox.image_addr as *const u8, mailbox.image_size as usize)
    };
    prepare(image, BOOT_MEDIUM_FEL, mailbox.args_addr, &layout)
}

fn args_addr_valid(args: u32) -> bool {
    if !(FEL_ARGS_BASE..FEL_PAYLOAD_BASE).contains(&args) {
        return false;
    }

    // SAFETY: `args` is inside the 4 KiB FEL argument window in SRAM.
    let bytes =
        unsafe { slice::from_raw_parts(args as *const u8, (FEL_PAYLOAD_BASE - args) as usize) };
    bytes.contains(&0)
}

fn prepare(
    image: &[u8],
    boot_medium: u32,
    args: u32,
    layout: &DramLayout,
) -> Result<Handoff, BootError> {
    let header = parse_header(image).map_err(BootError::Image)?;

    if header.size == 0 {
        return Err(BootError::EmptyPayload);
    }

    let load_end = header
        .load_addr
        .checked_add(header.size)
        .ok_or(BootError::AddressOverflow)?;
    if header.load_addr < SDRAM_BASE || load_end > layout.ram_end {
        return Err(BootError::OutsideRam {
            load_addr: header.load_addr,
            size: header.size,
            ram_size: layout.ram_size,
        });
    }
    if load_end > layout.boot_info {
        return Err(BootError::OverlapsReserved {
            load_addr: header.load_addr,
            size: header.size,
            reserved_start: layout.boot_info,
        });
    }
    if header.entry & 1 != 0 {
        return Err(BootError::EntryNotArm {
            entry: header.entry,
        });
    }
    if header.entry < header.load_addr || header.entry >= load_end {
        return Err(BootError::EntryOutsidePayload {
            entry: header.entry,
            load_addr: header.load_addr,
            size: header.size,
        });
    }

    // SAFETY: the header was validated against the staged image, the destination is inside DRAM
    // and outside the reserved area, and `ptr::copy` handles overlap.
    unsafe {
        ptr::copy(
            image.as_ptr().add(IMAGE_HEADER_SIZE),
            header.load_addr as *mut u8,
            header.size as usize,
        );
    }

    let payload =
        unsafe { slice::from_raw_parts(header.load_addr as *const u8, header.size as usize) };
    if image_checksum(payload) != header.checksum {
        return Err(BootError::Image(ImageError::BadChecksum));
    }

    let boot_info = BootInfo {
        magic: BOOT_INFO_MAGIC,
        version: BOOT_INFO_VERSION,
        boot_medium,
        ram_size: layout.ram_size,
        image_load_addr: header.load_addr,
        image_size: header.size,
        entry: header.entry,
        args,
    };

    // SAFETY: the destination is the 16-byte-aligned start of the reserved BootInfo slot.
    unsafe {
        ptr::write(layout.boot_info as *mut BootInfo, boot_info);
    }

    print!(
        "BOOT: handoff to 0x{:08x}: {} bytes at 0x{:08x}, checksum 0x{:08x}, BootInfo 0x{:08x}, SP 0x{:08x}\r\n",
        header.entry,
        header.size,
        header.load_addr,
        header.checksum,
        layout.boot_info,
        layout.stack_top
    );

    Ok(Handoff {
        entry: header.entry,
        boot_info: layout.boot_info,
        stack_top: layout.stack_top,
    })
}

/// The DRAM layout around the handoff.
struct DramLayout {
    ram_size: u32,
    ram_end: u32,
    /// First byte after the staging area; the `BootInfo` slot starts here.
    boot_info: u32,
    /// Top of the reserved stack, which grows down.
    stack_top: u32,
    staging_len: usize,
}

impl DramLayout {
    fn new(dram_size_mib: u32) -> Option<Self> {
        let ram_size = dram_size_mib.checked_mul(MIB)?;
        let ram_end = SDRAM_BASE.checked_add(ram_size)?;
        let stack_top = ram_end & !0xf;
        let stack_base = stack_top.checked_sub(HANDOFF_STACK_SIZE)?;
        let boot_info = stack_base.checked_sub(BOOT_INFO_SIZE)? & !0xf;
        if boot_info < SDRAM_BASE {
            return None;
        }

        Some(Self {
            ram_size,
            ram_end,
            boot_info,
            stack_top,
            staging_len: (boot_info - SDRAM_BASE) as usize,
        })
    }
}

struct Handoff {
    entry: u32,
    boot_info: u32,
    stack_top: u32,
}

impl Handoff {
    /// Enters the payload in the state fixed by ADR-0006.
    fn enter(self) -> ! {
        // SAFETY: DRAM, the payload and `BootInfo` are ready, and every register the payload
        // relies on is set explicitly. This block never returns.
        unsafe {
            core::arch::asm!(
                "dsb",
                "mrc p15, 0, {sctlr}, c1, c0, 0",
                "bic {sctlr}, {sctlr}, {cache_mask}",
                "mcr p15, 0, {sctlr}, c1, c0, 0",
                "isb",
                sctlr = out(reg) _,
                cache_mask = in(reg) 0x1005u32,
            );
            core::arch::asm!(
                "msr cpsr_c, {svc_masked}",
                "mov sp, {stack}",
                "bx {entry}",
                svc_masked = in(reg) 0xd3u32,
                stack = in(reg) self.stack_top,
                entry = in(reg) self.entry,
                in("r0") self.boot_info,
                in("r1") 0u32,
                options(noreturn),
            );
        }
    }
}
