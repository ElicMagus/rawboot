// SPDX-License-Identifier: Unlicense

//! Errors from USB transport, FEL framing and device identification.

use std::fmt;

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
