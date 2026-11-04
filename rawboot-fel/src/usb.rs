// SPDX-License-Identifier: CC-BY-4.0

//! `nusb`-backed transport: find the FEL device and move raw bulk frames.
//!
//! `nusb` drives the kernel's USB device interface directly (usbfs on Linux),
//! so this crate needs no system `libusb`.

use std::time::Duration;

use nusb::descriptors::TransferType;
use nusb::transfer::{Buffer, Bulk, In, Out, TransferError};
use nusb::{DeviceInfo, Endpoint, Interface, MaybeFuture};

use crate::device::Transport;
use crate::error::{FelError, TransportError};

/// Vendor ID of Allwinner's FEL gadget.
pub const FEL_VENDOR_ID: u16 = 0x1f3a;
/// Product ID of Allwinner's FEL gadget.
pub const FEL_PRODUCT_ID: u16 = 0xefe8;
/// Second vendor ID some FEL implementations report.
pub const FEL_ALTERNATE_VENDOR_ID: u16 = 0x1f3c;

/// Default timeout for one USB bulk transfer.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// Whether these USB IDs identify a FEL gadget.
///
/// `1f3a:efe8` is the standard BROM gadget; some implementations ship under
/// the `1f3c` vendor ID with any product ID.
pub fn is_fel_ids(vendor_id: u16, product_id: u16) -> bool {
    (vendor_id == FEL_VENDOR_ID && product_id == FEL_PRODUCT_ID)
        || vendor_id == FEL_ALTERNATE_VENDOR_ID
}

/// Whether this enumerated device is a FEL gadget.
pub fn is_fel_device(info: &DeviceInfo) -> bool {
    is_fel_ids(info.vendor_id(), info.product_id())
}

/// Bulk endpoints and timeout for one open FEL device.
pub struct UsbTransport {
    // Declared first so the interface outlives the endpoints it created.
    _interface: Interface,
    endpoint_in: Endpoint<Bulk, In>,
    endpoint_out: Endpoint<Bulk, Out>,
    timeout: Duration,
    in_packet_size: usize,
}

impl UsbTransport {
    /// Open the first attached FEL device using [`DEFAULT_TIMEOUT`].
    pub fn open() -> Result<Self, FelError> {
        Self::open_with_timeout(DEFAULT_TIMEOUT)
    }

    /// Open the first attached FEL device with a per-transfer timeout.
    pub fn open_with_timeout(timeout: Duration) -> Result<Self, FelError> {
        let interface = open_interface()?;

        let descriptor = interface
            .descriptor()
            .ok_or_else(|| FelError::Usb("FEL interface 0 has no descriptor".into()))?;

        let mut in_address = None;
        let mut out_address = None;
        for endpoint in descriptor.endpoints() {
            if endpoint.transfer_type() != TransferType::Bulk {
                continue;
            }
            if endpoint.address() & 0x80 != 0 {
                in_address.get_or_insert(endpoint.address());
            } else {
                out_address.get_or_insert(endpoint.address());
            }
        }

        let in_address = in_address
            .ok_or_else(|| FelError::Usb("FEL interface has no bulk IN endpoint".into()))?;
        let out_address = out_address
            .ok_or_else(|| FelError::Usb("FEL interface has no bulk OUT endpoint".into()))?;

        let endpoint_in = interface
            .endpoint::<Bulk, In>(in_address)
            .map_err(|error| FelError::Usb(format!("open bulk IN endpoint: {error}")))?;
        let endpoint_out = interface
            .endpoint::<Bulk, Out>(out_address)
            .map_err(|error| FelError::Usb(format!("open bulk OUT endpoint: {error}")))?;
        let in_packet_size = endpoint_in.max_packet_size().max(1);

        Ok(Self {
            _interface: interface,
            endpoint_in,
            endpoint_out,
            timeout,
            in_packet_size,
        })
    }
}

impl crate::FelDevice<UsbTransport> {
    /// Open the first attached FEL device using [`DEFAULT_TIMEOUT`].
    pub fn open() -> Result<Self, FelError> {
        Ok(Self::new(UsbTransport::open()?))
    }

    /// Open the first attached FEL device with a per-transfer timeout.
    pub fn open_with_timeout(timeout: Duration) -> Result<Self, FelError> {
        Ok(Self::new(UsbTransport::open_with_timeout(timeout)?))
    }
}

fn open_interface() -> Result<Interface, FelError> {
    let info = nusb::list_devices()
        .wait()
        .map_err(|error| FelError::Usb(format!("list USB devices: {error}")))?
        .find(is_fel_device)
        .ok_or(FelError::NoDevice)?;

    let device = info
        .open()
        .wait()
        .map_err(|error| FelError::Usb(format!("open FEL device: {error}")))?;
    device
        .detach_and_claim_interface(0)
        .wait()
        .map_err(|error| FelError::Usb(format!("claim FEL interface 0: {error}")))
}

impl Transport for UsbTransport {
    fn bulk_out(&mut self, data: &[u8]) -> Result<(), TransportError> {
        let completion = self
            .endpoint_out
            .transfer_blocking(Buffer::from(data), self.timeout);
        completion.status.map_err(map_transfer_error)?;

        if completion.actual_len != data.len() {
            return Err(TransportError::Usb(format!(
                "short bulk write: sent {} of {} bytes",
                completion.actual_len,
                data.len()
            )));
        }
        Ok(())
    }

    fn bulk_in(&mut self, buf: &mut [u8]) -> Result<usize, TransportError> {
        if buf.is_empty() {
            return Ok(0);
        }

        // The host stack expects whole packets on an IN transfer; ask for a
        // rounded-up length and keep only the bytes the device sent.
        let requested = buf.len().div_ceil(self.in_packet_size) * self.in_packet_size;
        let completion = self
            .endpoint_in
            .transfer_blocking(Buffer::new(requested), self.timeout);
        completion.status.map_err(map_transfer_error)?;

        let actual = completion.actual_len;
        if actual > buf.len() {
            return Err(TransportError::Usb(format!(
                "bulk read returned {actual} bytes into a {}-byte buffer",
                buf.len()
            )));
        }
        buf[..actual].copy_from_slice(&completion.buffer[..actual]);
        Ok(actual)
    }
}

fn map_transfer_error(error: TransferError) -> TransportError {
    match error {
        TransferError::Cancelled => TransportError::Timeout,
        TransferError::Disconnected => TransportError::Disconnected,
        other => TransportError::Usb(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_accepts_both_fel_id_sets() {
        assert!(is_fel_ids(0x1f3a, 0xefe8));
        assert!(is_fel_ids(0x1f3c, 0x0000));
        assert!(is_fel_ids(0x1f3c, 0xffff));
        assert!(!is_fel_ids(0x1f3a, 0xefe9));
        assert!(!is_fel_ids(0x1234, 0xefe8));
    }

    #[test]
    fn transfer_errors_map_to_transport_errors() {
        assert_eq!(
            map_transfer_error(TransferError::Cancelled),
            TransportError::Timeout
        );
        assert_eq!(
            map_transfer_error(TransferError::Disconnected),
            TransportError::Disconnected
        );
        assert!(matches!(
            map_transfer_error(TransferError::Stall),
            TransportError::Usb(_)
        ));
    }
}
