// SPDX-License-Identifier: Unlicense

//! Host-side Allwinner FEL transport.
//!
//! This crate implements the BROM's USB FEL protocol itself, without the
//! external `xfel` binary: it finds a FEL device, reads its version and chip
//! ID, and reads, writes and executes memory. The USB layer sits behind
//! [`Transport`], so framing, chunking and error handling are tested on the
//! host with a mock.
//!
//! ```
//! use rawboot_fel::{FelDevice, Transport, TransportError};
//!
//! struct Loopback;
//!
//! impl Transport for Loopback {
//!     fn bulk_out(&mut self, _data: &[u8]) -> Result<(), TransportError> {
//!         Ok(())
//!     }
//!     fn bulk_in(&mut self, _buf: &mut [u8]) -> Result<usize, TransportError> {
//!         Ok(0)
//!     }
//! }
//!
//! let _device = FelDevice::new(Loopback);
//! ```
//!
//! On a real host, [`FelDevice::open`] finds `1f3a:efe8` or any `1f3c:*`
//! device and [`FelDevice::identify`] confirms it is a T113-S3. No system
//! `libusb` is involved.

pub mod device;
pub mod error;
pub mod protocol;
pub mod usb;

pub use device::{DEFAULT_CHUNK_SIZE, FelDevice, Transport};
pub use error::{FelError, ProtocolError, TransportError};
pub use protocol::{CHIP_ID_T113, FelVersion, MAX_CHUNK_SIZE, Soc, T113_VECTOR};
pub use usb::{
    DEFAULT_TIMEOUT, FEL_ALTERNATE_VENDOR_ID, FEL_PRODUCT_ID, FEL_VENDOR_ID, UsbTransport,
    is_fel_device, is_fel_ids,
};
