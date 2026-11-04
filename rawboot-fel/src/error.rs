// SPDX-License-Identifier: CC-BY-4.0

//! Errors from USB transport, FEL framing, device identification and the
//! in-DRAM flasher bring-up.

use std::fmt;

use rawboot_image::{FlashContractError, VerifyError};

/// A failure below the FEL protocol: a USB bulk transfer did not complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// The transfer did not finish before its timeout.
    Timeout,
    /// The device disappeared (unplugged, reset into another mode, ...).
    Disconnected,
    /// Any other USB error reported by the host stack.
    Usb(String),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timeout => write!(f, "USB transfer timed out"),
            Self::Disconnected => write!(f, "USB device disconnected"),
            Self::Usb(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for TransportError {}

/// A malformed exchange with the BROM's FEL implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    /// A bulk read returned fewer bytes than the frame requires.
    ShortTransfer {
        /// Bytes the frame requires.
        expected: usize,
        /// Bytes the device actually sent.
        actual: usize,
    },
    /// A transfer that should have ended with the `AWUS` status word did not.
    BadUsbResponse {
        /// First four bytes the device sent instead of `AWUS`.
        got: [u8; 4],
    },
    /// The version reply was shorter than its 32-byte layout.
    ShortVersion {
        /// Bytes received.
        length: usize,
    },
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ShortTransfer { expected, actual } => {
                write!(f, "short transfer: expected {expected} bytes, got {actual}")
            }
            Self::BadUsbResponse { got } => {
                write!(f, "unexpected FEL response {got:02x?}; expected `AWUS`")
            }
            Self::ShortVersion { length } => {
                write!(f, "short version reply: {length} bytes")
            }
        }
    }
}

impl std::error::Error for ProtocolError {}

/// A FEL-callable image the host validates before loading it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    /// The returning DDR-init stub.
    DdrStub,
    /// The DRAM-linked flash job executor.
    Flasher,
}

impl fmt::Display for ImageKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DdrStub => f.write_str("DDR-init stub"),
            Self::Flasher => f.write_str("flasher"),
        }
    }
}

/// Anything that can stop a FEL operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FelError {
    /// No device matching the Allwinner FEL USB IDs is attached.
    NoDevice,
    /// A USB operation outside a bulk transfer failed (enumeration, open, claim).
    Usb(String),
    /// A bulk transfer failed.
    Transport(TransportError),
    /// The device's replies did not follow the FEL protocol.
    Protocol(ProtocolError),
    /// The reported chip ID has no support in this crate.
    UnsupportedSoc {
        /// Chip ID from the FEL version reply.
        chip_id: u16,
    },
    /// The chip ID matches the T113 family, but the FEL vector is not the T113 one.
    UnexpectedChip {
        /// Chip ID from the FEL version reply.
        chip_id: u16,
        /// Word at address zero, as read over FEL.
        vector: u32,
    },
    /// A multi-chunk access would run past the 32-bit FEL address space.
    AddressOverflow {
        /// Base address of the access.
        address: u32,
        /// Offset of the chunk that would overflow.
        offset: u64,
    },
    /// The configured chunk size is zero or exceeds [`crate::MAX_CHUNK_SIZE`].
    InvalidChunkSize {
        /// Rejected chunk size.
        size: usize,
    },
    /// A read could not reserve a buffer of the requested length.
    OutOfMemory {
        /// Bytes the read asked for.
        length: usize,
    },
    /// A FEL-callable image is empty.
    EmptyImage {
        /// Which image was rejected.
        kind: ImageKind,
    },
    /// A FEL-callable image carries an eGON header.
    UnexpectedEgon {
        /// Which image was rejected.
        kind: ImageKind,
    },
    /// The DDR-init stub is larger than its FEL SRAM window.
    StubTooLarge {
        /// Bytes supplied.
        size: usize,
        /// Largest accepted size.
        max: u32,
    },
    /// The flasher is larger than its DRAM window.
    FlasherTooLarge {
        /// Bytes supplied.
        size: usize,
        /// Largest accepted size.
        max: u32,
    },
    /// The DDR-init stub did not leave a valid result in SRAM.
    DdrInitFailed {
        /// Magic read after `exec`, or zero when the stub never wrote it.
        magic: u32,
        /// Detected DRAM size in MiB, or zero.
        size_mib: u32,
    },
    /// The SD loader is not a sealed eGON image.
    LoaderNotSealed(VerifyError),
    /// A word written to DRAM did not read back.
    DramMismatch {
        /// Address of the first mismatching word.
        address: u32,
        /// Pattern the host wrote.
        expected: u32,
        /// Value the device returned.
        actual: u32,
    },
    /// A job or status block violates the shared contract.
    FlashContract(FlashContractError),
    /// The flasher answered a different job than the host submitted.
    StatusSequence {
        /// Sequence in the job the host wrote.
        expected: u32,
        /// Sequence in the status the flasher wrote.
        actual: u32,
    },
    /// The detected DRAM is smaller than the fixed staging window.
    DramTooSmall {
        /// Detected DRAM size in MiB.
        size_mib: u32,
        /// Bytes the staging layout requires.
        required: u64,
    },
}

impl fmt::Display for FelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoDevice => write!(f, "no Allwinner FEL device found (USB 1f3a:efe8 or 1f3c:*)"),
            Self::Usb(message) => write!(f, "USB: {message}"),
            Self::Transport(error) => write!(f, "{error}"),
            Self::Protocol(error) => write!(f, "FEL protocol: {error}"),
            Self::UnsupportedSoc { chip_id } => {
                write!(f, "unsupported SoC chip ID {chip_id:#06x}")
            }
            Self::UnexpectedChip { chip_id, vector } => write!(
                f,
                "chip ID {chip_id:#06x} is not a T113-S3 (word at 0x0 is {vector:#010x})"
            ),
            Self::AddressOverflow { address, offset } => write!(
                f,
                "address {address:#010x} plus offset {offset:#x} overflows 32 bits"
            ),
            Self::InvalidChunkSize { size } => write!(
                f,
                "chunk size {size} must be between 1 and {}",
                crate::MAX_CHUNK_SIZE
            ),
            Self::OutOfMemory { length } => {
                write!(f, "cannot allocate a {length}-byte read buffer")
            }
            Self::EmptyImage { kind } => write!(f, "{kind} is empty"),
            Self::UnexpectedEgon { kind } => write!(
                f,
                "{kind} starts with an eGON header; a FEL-callable image must not"
            ),
            Self::StubTooLarge { size, max } => write!(
                f,
                "DDR-init stub is {size} bytes, larger than the {max}-byte SRAM window"
            ),
            Self::FlasherTooLarge { size, max } => write!(
                f,
                "flasher is {size} bytes, larger than the {max}-byte DRAM window"
            ),
            Self::DdrInitFailed { magic, size_mib } => write!(
                f,
                "DDR-init stub did not report success (result magic {magic:#010x}, {size_mib} MiB)"
            ),
            Self::LoaderNotSealed(error) => write!(f, "SD loader is not sealed: {error}"),
            Self::DramMismatch {
                address,
                expected,
                actual,
            } => write!(
                f,
                "DRAM read-back at {address:#010x} returned {actual:#010x}, expected {expected:#010x}"
            ),
            Self::FlashContract(error) => write!(f, "flash contract: {error}"),
            Self::StatusSequence { expected, actual } => write!(
                f,
                "flasher answered job sequence {actual}, expected {expected}"
            ),
            Self::DramTooSmall { size_mib, required } => write!(
                f,
                "DRAM is {size_mib} MiB; the flasher staging window needs {required} bytes"
            ),
        }
    }
}

impl std::error::Error for FelError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Transport(error) => Some(error),
            Self::Protocol(error) => Some(error),
            _ => None,
        }
    }
}

impl From<TransportError> for FelError {
    fn from(error: TransportError) -> Self {
        Self::Transport(error)
    }
}

impl From<ProtocolError> for FelError {
    fn from(error: ProtocolError) -> Self {
        Self::Protocol(error)
    }
}

impl From<FlashContractError> for FelError {
    fn from(error: FlashContractError) -> Self {
        Self::FlashContract(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_kind_names_appear_in_their_errors() {
        assert_eq!(ImageKind::DdrStub.to_string(), "DDR-init stub");
        assert_eq!(ImageKind::Flasher.to_string(), "flasher");
        assert_eq!(
            FelError::EmptyImage {
                kind: ImageKind::DdrStub
            }
            .to_string(),
            "DDR-init stub is empty"
        );
        assert_eq!(
            FelError::UnexpectedEgon {
                kind: ImageKind::Flasher
            }
            .to_string(),
            "flasher starts with an eGON header; a FEL-callable image must not"
        );
    }
}
