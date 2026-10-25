// SPDX-License-Identifier: Unlicense

//! FEL operations on top of a byte-level transport.
//!
//! [`FelDevice`] owns the protocol state machine: it frames commands, splits
//! large accesses into 64 KiB chunks, and validates the BROM's replies. The USB
//! details live behind [`Transport`], so the whole protocol can be exercised on
//! the host with a scripted mock.

use crate::MAX_CHUNK_SIZE;
use crate::error::{FelError, ProtocolError, TransportError};
use crate::protocol::{
    self, FEL_EXEC, FEL_READ, FEL_REQUEST_SIZE, FEL_STATUS_SIZE, FEL_VERSION, FEL_WRITE,
    FelVersion, Soc, USB_HEADER_SIZE, USB_READ, USB_RESPONSE_SIZE, USB_WRITE, VERSION_SIZE,
};

/// Default number of bytes per FEL read or write.
pub const DEFAULT_CHUNK_SIZE: usize = MAX_CHUNK_SIZE;

/// Raw bidirectional USB bulk transport used by [`FelDevice`].
///
/// Implementations carry whole transfers only; framing, chunking and timeouts
/// (in the USB implementation) stay outside the protocol layer.
pub trait Transport {
    /// Send one complete bulk payload.
    fn bulk_out(&mut self, data: &[u8]) -> Result<(), TransportError>;

    /// Receive up to `buf.len()` bytes and return how many arrived.
    ///
    /// A short result is not an error here: the device layer turns it into
    /// [`ProtocolError::ShortTransfer`].
    fn bulk_in(&mut self, buf: &mut [u8]) -> Result<usize, TransportError>;
}

/// A FEL device speaking the Allwinner BROM protocol over a [`Transport`].
pub struct FelDevice<T: Transport> {
    transport: T,
    chunk_size: usize,
}

impl<T: Transport> FelDevice<T> {
    /// Wrap a transport with the default 64 KiB chunk size.
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            chunk_size: DEFAULT_CHUNK_SIZE,
        }
    }

    /// Use a different chunk size, for tests or for BROMs that need smaller
    /// transfers. Must be between 1 and [`MAX_CHUNK_SIZE`].
    pub fn with_chunk_size(mut self, chunk_size: usize) -> Result<Self, FelError> {
        if chunk_size == 0 || chunk_size > MAX_CHUNK_SIZE {
            return Err(FelError::InvalidChunkSize { size: chunk_size });
        }
        self.chunk_size = chunk_size;
        Ok(self)
    }

    /// Recover the transport, for example to inspect a test mock.
    pub fn into_inner(self) -> T {
        self.transport
    }

    /// Read the BROM's version block.
    pub fn version(&mut self) -> Result<FelVersion, FelError> {
        let mut bytes = [0u8; VERSION_SIZE];
        self.send_fel_request(FEL_VERSION, 0, 0)?;
        self.usb_read(&mut bytes)?;
        self.read_status()?;
        Ok(FelVersion::parse(&bytes)?)
    }

    /// Read the version block and confirm the target is a T113-S3.
    pub fn identify(&mut self) -> Result<(FelVersion, Soc), FelError> {
        let version = self.version()?;
        let chip_id = version.chip_id();
        let soc = Soc::from_chip_id(chip_id).ok_or(FelError::UnsupportedSoc { chip_id })?;

        let vector = self.read_u32(0)?;
        if vector != protocol::T113_VECTOR {
            return Err(FelError::UnexpectedChip { chip_id, vector });
        }

        Ok((version, soc))
    }

    /// Read `length` bytes starting at `address`, in chunks the BROM accepts.
    pub fn read(&mut self, address: u32, length: usize) -> Result<Vec<u8>, FelError> {
        let mut data = Vec::new();
        data.try_reserve_exact(length)
            .map_err(|_| FelError::OutOfMemory { length })?;
        let mut offset = 0u64;

        while offset < length as u64 {
            let chunk = ((length as u64 - offset).min(self.chunk_size as u64)) as usize;
            let address = self.chunk_address(address, offset)?;
            let mut buf = vec![0u8; chunk];
            self.read_raw(address, &mut buf)?;
            data.extend_from_slice(&buf);
            offset += chunk as u64;
        }

        Ok(data)
    }

    /// Read a little-endian 32-bit word.
    pub fn read_u32(&mut self, address: u32) -> Result<u32, FelError> {
        let bytes = self.read(address, 4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Write `data` starting at `address`, in chunks the BROM accepts.
    pub fn write(&mut self, address: u32, data: &[u8]) -> Result<(), FelError> {
        let mut offset = 0u64;

        for chunk in data.chunks(self.chunk_size) {
            let address = self.chunk_address(address, offset)?;
            self.write_raw(address, chunk)?;
            offset += chunk.len() as u64;
        }

        Ok(())
    }

    /// Jump to `address` and let the BROM run the code there.
    pub fn exec(&mut self, address: u32) -> Result<(), FelError> {
        self.send_fel_request(FEL_EXEC, address, 0)?;
        self.read_status()
    }

    fn chunk_address(&self, address: u32, offset: u64) -> Result<u32, FelError> {
        u32::try_from(offset)
            .ok()
            .and_then(|offset| address.checked_add(offset))
            .ok_or(FelError::AddressOverflow { address, offset })
    }

    fn read_raw(&mut self, address: u32, buf: &mut [u8]) -> Result<(), FelError> {
        debug_assert!(buf.len() <= MAX_CHUNK_SIZE);
        self.send_fel_request(FEL_READ, address, buf.len() as u32)?;
        self.usb_read(buf)?;
        self.read_status()
    }

    fn write_raw(&mut self, address: u32, data: &[u8]) -> Result<(), FelError> {
        debug_assert!(data.len() <= MAX_CHUNK_SIZE);
        self.send_fel_request(FEL_WRITE, address, data.len() as u32)?;
        self.usb_write(data)?;
        self.read_status()
    }

    fn send_fel_request(
        &mut self,
        request: u32,
        address: u32,
        length: u32,
    ) -> Result<(), FelError> {
        let frame = protocol::fel_request(request, address, length);
        self.usb_write(&frame)
    }

    fn send_usb_request(&mut self, request: u16, length: usize) -> Result<(), FelError> {
        debug_assert!(length <= MAX_CHUNK_SIZE);
        let frame = protocol::usb_request(request, length as u32);
        self.transport.bulk_out(&frame).map_err(FelError::Transport)
    }

    fn read_usb_response(&mut self) -> Result<(), FelError> {
        let mut response = [0u8; USB_RESPONSE_SIZE];
        let actual = self
            .transport
            .bulk_in(&mut response)
            .map_err(FelError::Transport)?;
        if actual != USB_RESPONSE_SIZE {
            return Err(ProtocolError::ShortTransfer {
                expected: USB_RESPONSE_SIZE,
                actual,
            }
            .into());
        }
        if !protocol::is_awus(&response) {
            let mut got = [0u8; 4];
            got.copy_from_slice(&response[..4]);
            return Err(ProtocolError::BadUsbResponse { got }.into());
        }
        Ok(())
    }

    fn usb_read(&mut self, buf: &mut [u8]) -> Result<(), FelError> {
        self.send_usb_request(USB_READ, buf.len())?;
        let actual = self.transport.bulk_in(buf).map_err(FelError::Transport)?;
        if actual != buf.len() {
            return Err(ProtocolError::ShortTransfer {
                expected: buf.len(),
                actual,
            }
            .into());
        }
        self.read_usb_response()
    }

    fn usb_write(&mut self, data: &[u8]) -> Result<(), FelError> {
        self.send_usb_request(USB_WRITE, data.len())?;
        self.transport.bulk_out(data).map_err(FelError::Transport)?;
        self.read_usb_response()
    }

    fn read_status(&mut self) -> Result<(), FelError> {
        // Every command ends with an 8-byte status word. Neither xfel nor
        // sunxi-tools assign it a failure code: a rejected command shows up as
        // a stalled or short USB transfer, so drain the word instead of
        // inventing semantics the BROM does not document.
        let mut status = [0u8; FEL_STATUS_SIZE];
        self.usb_read(&mut status)
    }
}

// Keep the frame sizes next to the protocol constants they mirror.
const _: () = {
    assert!(FEL_REQUEST_SIZE == 16);
    assert!(USB_HEADER_SIZE == 32);
};

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::*;
    use crate::protocol::{
        CHIP_ID_T113, FEL_REQUEST_SIZE, T113_VECTOR, fel_request, usb_request, version_reply,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Step {
        Out(Vec<u8>),
        In(Vec<u8>),
        Fail(TransportError),
    }

    struct Mock {
        steps: VecDeque<Step>,
    }

    impl Mock {
        fn new(steps: Vec<Step>) -> Self {
            Self {
                steps: steps.into(),
            }
        }

        fn assert_done(&self) {
            assert!(self.steps.is_empty(), "unconsumed steps: {:?}", self.steps);
        }
    }

    impl Transport for Mock {
        fn bulk_out(&mut self, data: &[u8]) -> Result<(), TransportError> {
            match self.steps.pop_front() {
                Some(Step::Out(expected)) => {
                    assert_eq!(expected, data, "bulk_out frame mismatch");
                    Ok(())
                }
                Some(Step::Fail(error)) => Err(error),
                Some(other) => panic!("expected {other:?}, got bulk_out({data:02x?})"),
                None => panic!("unexpected bulk_out({data:02x?})"),
            }
        }

        fn bulk_in(&mut self, buf: &mut [u8]) -> Result<usize, TransportError> {
            match self.steps.pop_front() {
                Some(Step::In(data)) => {
                    let actual = data.len().min(buf.len());
                    buf[..actual].copy_from_slice(&data[..actual]);
                    Ok(actual)
                }
                Some(Step::Fail(error)) => Err(error),
                Some(other) => panic!("expected {other:?}, got bulk_in({} bytes)", buf.len()),
                None => panic!("unexpected bulk_in({} bytes)", buf.len()),
            }
        }
    }

    fn awuc(request: u16, length: u32) -> Step {
        Step::Out(usb_request(request, length).to_vec())
    }

    fn fel(request: u32, address: u32, length: u32) -> Step {
        Step::Out(fel_request(request, address, length).to_vec())
    }

    fn awus() -> Step {
        let mut response = vec![0u8; USB_RESPONSE_SIZE];
        response[..4].copy_from_slice(b"AWUS");
        Step::In(response)
    }

    fn status_steps() -> Vec<Step> {
        vec![awuc(USB_READ, 8), Step::In(vec![0u8; 8]), awus()]
    }

    fn version_steps(bytes: &[u8]) -> Vec<Step> {
        let mut steps = vec![
            awuc(USB_WRITE, FEL_REQUEST_SIZE as u32),
            fel(FEL_VERSION, 0, 0),
            awus(),
            awuc(USB_READ, VERSION_SIZE as u32),
            Step::In(bytes.to_vec()),
            awus(),
        ];
        steps.extend(status_steps());
        steps
    }

    fn read_steps(address: u32, data: &[u8]) -> Vec<Step> {
        let mut steps = vec![
            awuc(USB_WRITE, FEL_REQUEST_SIZE as u32),
            fel(FEL_READ, address, data.len() as u32),
            awus(),
            awuc(USB_READ, data.len() as u32),
            Step::In(data.to_vec()),
            awus(),
        ];
        steps.extend(status_steps());
        steps
    }

    fn write_steps(address: u32, data: &[u8]) -> Vec<Step> {
        let mut steps = vec![
            awuc(USB_WRITE, FEL_REQUEST_SIZE as u32),
            fel(FEL_WRITE, address, data.len() as u32),
            awus(),
            awuc(USB_WRITE, data.len() as u32),
            Step::Out(data.to_vec()),
            awus(),
        ];
        steps.extend(status_steps());
        steps
    }

    fn exec_steps(address: u32) -> Vec<Step> {
        let mut steps = vec![
            awuc(USB_WRITE, FEL_REQUEST_SIZE as u32),
            fel(FEL_EXEC, address, 0),
            awus(),
        ];
        steps.extend(status_steps());
        steps
    }

    fn t113_version() -> [u8; crate::protocol::VERSION_SIZE] {
        version_reply(0x0018_5900)
    }

    fn device(steps: Vec<Step>) -> FelDevice<Mock> {
        FelDevice::new(Mock::new(steps))
    }

    #[test]
    fn version_uses_the_brom_framing() {
        let bytes = t113_version();
        let mut fel = device(version_steps(&bytes));

        let version = fel.version().unwrap();
        assert_eq!(version.chip_id(), CHIP_ID_T113);
        assert_eq!(version.signature[..4], *b"eGON");

        fel.into_inner().assert_done();
    }

    #[test]
    fn identify_recognises_the_t113_reset_vector() {
        let bytes = t113_version();
        let mut steps = version_steps(&bytes);
        steps.extend(read_steps(0, &T113_VECTOR.to_le_bytes()));

        let mut fel = device(steps);
        let (version, soc) = fel.identify().unwrap();

        assert_eq!(soc, Soc::T113);
        assert_eq!(soc.name(), "T113-S3");
        assert_eq!(version.chip_id(), CHIP_ID_T113);
        fel.into_inner().assert_done();
    }

    #[test]
    fn identify_rejects_an_unsupported_chip_id() {
        let bytes = version_reply(0x0016_2300);
        let mut fel = device(version_steps(&bytes));

        assert_eq!(
            fel.identify(),
            Err(FelError::UnsupportedSoc { chip_id: 0x1623 })
        );
        fel.into_inner().assert_done();
    }

    #[test]
    fn identify_rejects_a_family_id_with_a_different_reset_vector() {
        let bytes = t113_version();
        let mut steps = version_steps(&bytes);
        steps.extend(read_steps(0, &0x1234_5678u32.to_le_bytes()));

        let mut fel = device(steps);
        assert_eq!(
            fel.identify(),
            Err(FelError::UnexpectedChip {
                chip_id: CHIP_ID_T113,
                vector: 0x1234_5678,
            })
        );
        fel.into_inner().assert_done();
    }

    #[test]
    fn write_splits_at_the_chunk_size_and_advances_the_address() {
        let mut steps = Vec::new();
        steps.extend(write_steps(0x1000, b"abcd"));
        steps.extend(write_steps(0x1004, b"efgh"));
        steps.extend(write_steps(0x1008, b"ij"));

        let mut fel = device(steps).with_chunk_size(4).unwrap();
        fel.write(0x1000, b"abcdefghij").unwrap();
        fel.into_inner().assert_done();
    }

    #[test]
    fn read_splits_at_the_chunk_size_and_joins_the_result() {
        let mut steps = Vec::new();
        steps.extend(read_steps(0x2000, b"abcd"));
        steps.extend(read_steps(0x2004, b"efgh"));
        steps.extend(read_steps(0x2008, b"ij"));

        let mut fel = device(steps).with_chunk_size(4).unwrap();
        assert_eq!(fel.read(0x2000, 10).unwrap(), b"abcdefghij");
        fel.into_inner().assert_done();
    }

    #[test]
    fn default_chunking_uses_64_kib_transfers() {
        let data = vec![0xa5u8; MAX_CHUNK_SIZE + 1];
        let mut steps = write_steps(0x4000_0000, &data[..MAX_CHUNK_SIZE]);
        steps.extend(write_steps(
            0x4000_0000 + MAX_CHUNK_SIZE as u32,
            &data[MAX_CHUNK_SIZE..],
        ));

        let mut fel = device(steps);
        fel.write(0x4000_0000, &data).unwrap();
        fel.into_inner().assert_done();
    }

    #[test]
    fn exec_sends_the_address_and_no_payload() {
        let mut fel = device(exec_steps(0x0002_8000));

        fel.exec(0x0002_8000).unwrap();
        fel.into_inner().assert_done();
    }

    #[test]
    fn an_empty_write_touches_nothing() {
        let mut fel = device(Vec::new());

        fel.write(0x0003_9000, &[]).unwrap();
        fel.into_inner().assert_done();
    }

    #[test]
    fn transport_errors_are_propagated() {
        let mut fel = device(vec![Step::Fail(TransportError::Timeout)]);

        assert_eq!(
            fel.version(),
            Err(FelError::Transport(TransportError::Timeout))
        );
    }

    #[test]
    fn a_bad_awus_word_is_a_protocol_error() {
        let bytes = t113_version();
        let mut steps = version_steps(&bytes);
        steps[2] = Step::In(vec![b'X'; USB_RESPONSE_SIZE]);

        let mut fel = device(steps);
        assert_eq!(
            fel.version(),
            Err(FelError::Protocol(ProtocolError::BadUsbResponse {
                got: *b"XXXX",
            }))
        );
    }

    #[test]
    fn a_short_read_is_a_protocol_error() {
        let mut steps = read_steps(0x2000, b"abcd");
        steps[4] = Step::In(b"ab".to_vec());

        let mut fel = device(steps);
        assert_eq!(
            fel.read(0x2000, 4),
            Err(FelError::Protocol(ProtocolError::ShortTransfer {
                expected: 4,
                actual: 2,
            }))
        );
    }

    #[test]
    fn chunk_size_must_be_within_the_brom_limit() {
        assert_eq!(
            FelDevice::new(Mock::new(Vec::new()))
                .with_chunk_size(0)
                .err(),
            Some(FelError::InvalidChunkSize { size: 0 })
        );
        assert_eq!(
            FelDevice::new(Mock::new(Vec::new()))
                .with_chunk_size(MAX_CHUNK_SIZE + 1)
                .err(),
            Some(FelError::InvalidChunkSize {
                size: MAX_CHUNK_SIZE + 1
            })
        );
    }

    #[test]
    fn a_chunk_that_would_wrap_the_address_space_is_rejected() {
        let mut fel = device(write_steps(0xffff_ffff, b"abcd"))
            .with_chunk_size(4)
            .unwrap();

        assert_eq!(
            fel.write(0xffff_ffff, b"abcdefgh"),
            Err(FelError::AddressOverflow {
                address: 0xffff_ffff,
                offset: 4,
            })
        );
        fel.into_inner().assert_done();
    }

    #[test]
    fn read_u32_decodes_little_endian() {
        let mut fel = device(read_steps(0x1234, &0xdead_beefu32.to_le_bytes()));
        assert_eq!(fel.read_u32(0x1234).unwrap(), 0xdead_beef);
        fel.into_inner().assert_done();
    }
}
