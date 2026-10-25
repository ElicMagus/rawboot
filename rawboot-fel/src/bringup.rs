// SPDX-License-Identifier: Unlicense

//! The host side of the in-DRAM flasher bring-up.
//!
//! Three operations, in order: run the DDR-init stub and read its SRAM result,
//! prove DRAM by writing and reading a pattern, then place a [`FlashJob`] and
//! the flasher, execute it and read the [`FlashStatus`] it returns (ADR-0013).
//! All addresses and block layouts come from [`rawboot_image::flash`].

use rawboot_image::{
    DDR_RESULT_ADDR, DDR_RESULT_SIZE, DDR_STUB_LOAD_ADDR, DDR_STUB_MAX_SIZE, DRAM_TEST_ADDR,
    DRAM_TEST_SIZE, DdrResult, FLASH_ARGS_ADDR, FLASH_COMMAND_FLASH_SD, FLASH_COMMAND_NOOP,
    FLASH_JOB_ADDR, FLASH_LOADER_ADDR, FLASH_PAYLOAD_ADDR, FLASH_PLAN_ADDR,
    FLASH_STAGE_REQUIRED_SIZE, FLASH_STATUS_ADDR, FLASH_STATUS_SIZE, FLASHER_LOAD_ADDR,
    FLASHER_MAX_SIZE, FlashJob, FlashPlan, FlashStatus, read_u32_le, verify_sealed,
};

use crate::device::FelDevice;
use crate::device::Transport;
use crate::error::FelError;

const MIB: u64 = 1024 * 1024;

/// The result of a complete no-op bring-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BringupReport {
    /// Detected DRAM size in MiB, as reported by the DDR-init stub.
    pub dram_size_mib: u32,
    /// The flasher's status block.
    pub status: FlashStatus,
}

/// Inputs for [`FelDevice::flash_sd_card`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlashSdRequest<'a> {
    /// Returning DDR-init stub.
    pub stub: &'a [u8],
    /// DRAM-linked flasher.
    pub flasher: &'a [u8],
    /// Wrapped payload image written as `payload.bin`.
    pub payload: &'a [u8],
    /// Optional `boot.cfg` contents; empty leaves the existing file alone.
    pub args: &'a [u8],
    /// Sealed SD/SPI loader written to sector 16.
    pub loader: &'a [u8],
    /// Job/status sequence.
    pub sequence: u32,
}

/// The result of an in-board SD flash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlashSdReport {
    /// Detected DRAM size in MiB, as reported by the DDR-init stub.
    pub dram_size_mib: u32,
    /// The flasher's status block; check [`FlashStatus::is_ok`] before claiming
    /// the card was written.
    pub status: FlashStatus,
}

impl<T: Transport> FelDevice<T> {
    /// Run the DDR-init stub, then read the result it leaves in SRAM.
    ///
    /// A stub that does not return leaves the next FEL read waiting until the
    /// transport timeout, which surfaces as a transport error.
    pub fn run_ddr_stub(&mut self, stub: &[u8]) -> Result<u32, FelError> {
        if stub.is_empty() {
            return Err(FelError::EmptyImage {
                what: "DDR-init stub",
            });
        }
        if stub.starts_with(b"eGON") {
            return Err(FelError::UnexpectedEgon {
                what: "DDR-init stub",
            });
        }
        if stub.len() > DDR_STUB_MAX_SIZE as usize {
            return Err(FelError::StubTooLarge {
                size: stub.len(),
                max: DDR_STUB_MAX_SIZE,
            });
        }

        self.write(DDR_STUB_LOAD_ADDR, stub)?;
        // A retry must not accept the result a previous, successful stub left.
        self.write(DDR_RESULT_ADDR, &[0u8; DDR_RESULT_SIZE])?;
        self.exec(DDR_STUB_LOAD_ADDR)?;

        let bytes = self.read(DDR_RESULT_ADDR, DDR_RESULT_SIZE)?;
        match DdrResult::from_bytes(&bytes) {
            Some(result) if result.size_mib != 0 => Ok(result.size_mib),
            _ => Err(FelError::DdrInitFailed {
                magic: read_u32_le(&bytes, 0),
                size_mib: read_u32_le(&bytes, 4),
            }),
        }
    }

    /// Write a known pattern to DRAM and read it back.
    pub fn verify_dram(&mut self) -> Result<(), FelError> {
        let mut expected = Vec::with_capacity(DRAM_TEST_SIZE as usize);
        for index in 0..DRAM_TEST_SIZE / 4 {
            expected.extend_from_slice(&dram_pattern(index).to_le_bytes());
        }

        self.write(DRAM_TEST_ADDR, &expected)?;
        let actual = self.read(DRAM_TEST_ADDR, expected.len())?;

        let (expected, _) = expected.as_chunks::<4>();
        let (actual, _) = actual.as_chunks::<4>();
        for (index, (expected, actual)) in expected.iter().zip(actual).enumerate() {
            let expected = u32::from_le_bytes(*expected);
            let actual = u32::from_le_bytes(*actual);
            if expected != actual {
                return Err(FelError::DramMismatch {
                    address: DRAM_TEST_ADDR + index as u32 * 4,
                    expected,
                    actual,
                });
            }
        }

        Ok(())
    }

    /// Write `job` and `flasher`, execute the flasher, and read its status.
    ///
    /// The status is cleared before the jump, so a flasher that returns without
    /// writing one is reported as a contract error instead of a stale success.
    pub fn run_flash_job(
        &mut self,
        flasher: &[u8],
        job: FlashJob,
    ) -> Result<FlashStatus, FelError> {
        if flasher.is_empty() {
            return Err(FelError::EmptyImage { what: "flasher" });
        }
        if flasher.starts_with(b"eGON") {
            return Err(FelError::UnexpectedEgon { what: "flasher" });
        }
        job.verify()?;
        if flasher.len() > FLASHER_MAX_SIZE as usize {
            return Err(FelError::FlasherTooLarge {
                size: flasher.len(),
                max: FLASHER_MAX_SIZE,
            });
        }

        self.write(FLASH_STATUS_ADDR, &[0u8; FLASH_STATUS_SIZE])?;
        self.write(FLASH_JOB_ADDR, &job.to_bytes())?;
        self.write(FLASHER_LOAD_ADDR, flasher)?;
        self.exec(FLASHER_LOAD_ADDR)?;

        let status = FlashStatus::from_bytes(&self.read(FLASH_STATUS_ADDR, FLASH_STATUS_SIZE)?)?;
        if status.sequence != job.sequence {
            return Err(FelError::StatusSequence {
                expected: job.sequence,
                actual: status.sequence,
            });
        }

        Ok(status)
    }

    /// Run the whole #41 spike: DRAM, pattern, no-op job and status.
    pub fn bringup(
        &mut self,
        stub: &[u8],
        flasher: &[u8],
        sequence: u32,
    ) -> Result<BringupReport, FelError> {
        let dram_size_mib = self.run_ddr_stub(stub)?;
        self.verify_dram()?;
        let status = self.run_flash_job(
            flasher,
            FlashJob::new(sequence, FLASH_COMMAND_NOOP, 0, 0, 0),
        )?;

        Ok(BringupReport {
            dram_size_mib,
            status,
        })
    }

    /// Stage the payload files and loader, then run an SD flash job.
    ///
    /// The plan is built and validated before the device is touched, so an
    /// oversized input fails without uploading anything. The staged regions sit
    /// at the fixed addresses from [`rawboot_image::flash`]; the flasher checks
    /// their checksums before it writes the card (ADR-0014).
    pub fn flash_sd_card(
        &mut self,
        request: FlashSdRequest<'_>,
    ) -> Result<FlashSdReport, FelError> {
        let FlashSdRequest {
            stub,
            flasher,
            payload,
            args,
            loader,
            sequence,
        } = request;

        verify_sealed(loader).map_err(FelError::LoaderNotSealed)?;
        let plan = FlashPlan::new(payload, args, loader)?;

        let dram_size_mib = self.run_ddr_stub(stub)?;
        let required = FLASH_STAGE_REQUIRED_SIZE as u64;
        if (dram_size_mib as u64) * MIB < required {
            return Err(FelError::DramTooSmall {
                size_mib: dram_size_mib,
                required,
            });
        }
        self.verify_dram()?;

        self.write(FLASH_PLAN_ADDR, &plan.to_bytes())?;
        self.write(FLASH_PAYLOAD_ADDR, payload)?;
        if !args.is_empty() {
            self.write(FLASH_ARGS_ADDR, args)?;
        }
        self.write(FLASH_LOADER_ADDR, loader)?;

        let status = self.run_flash_job(
            flasher,
            FlashJob::new(sequence, FLASH_COMMAND_FLASH_SD, 0, 0, 0),
        )?;

        Ok(FlashSdReport {
            dram_size_mib,
            status,
        })
    }
}

const DRAM_PATTERN_SEED: u32 = 0x5242_0000;
const DRAM_PATTERN_MULTIPLIER: u32 = 0x9e37_79b9;

fn dram_pattern(index: u32) -> u32 {
    DRAM_PATTERN_SEED ^ index.wrapping_mul(DRAM_PATTERN_MULTIPLIER)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::TransportError;
    use crate::protocol::{
        FEL_EXEC, FEL_READ, FEL_REQUEST_SIZE, FEL_WRITE, USB_READ, USB_RESPONSE_SIZE, USB_WRITE,
        fel_request, usb_request,
    };
    use rawboot_image::{FLASH_RESULT_OK, FLASH_STEP_DONE};
    use std::collections::VecDeque;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Step {
        Out(Vec<u8>),
        In(Vec<u8>),
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

    fn device(steps: Vec<Step>) -> FelDevice<Mock> {
        FelDevice::new(Mock::new(steps))
    }

    #[test]
    fn runs_the_stub_and_reads_the_size() {
        let stub = b"stub";
        let mut steps = write_steps(DDR_STUB_LOAD_ADDR, stub);
        steps.extend(write_steps(DDR_RESULT_ADDR, &[0u8; DDR_RESULT_SIZE]));
        steps.extend(exec_steps(DDR_STUB_LOAD_ADDR));
        steps.extend(read_steps(DDR_RESULT_ADDR, &DdrResult::new(128).to_bytes()));

        let mut fel = device(steps);
        assert_eq!(fel.run_ddr_stub(stub), Ok(128));
        fel.into_inner().assert_done();
    }

    #[test]
    fn reports_a_stub_that_did_not_publish_a_result() {
        let mut steps = write_steps(DDR_STUB_LOAD_ADDR, b"stub");
        steps.extend(write_steps(DDR_RESULT_ADDR, &[0u8; DDR_RESULT_SIZE]));
        steps.extend(exec_steps(DDR_STUB_LOAD_ADDR));
        steps.extend(read_steps(DDR_RESULT_ADDR, &[0u8; DDR_RESULT_SIZE]));

        let mut fel = device(steps);
        assert_eq!(
            fel.run_ddr_stub(b"stub"),
            Err(FelError::DdrInitFailed {
                magic: 0,
                size_mib: 0
            })
        );
        fel.into_inner().assert_done();
    }

    #[test]
    fn rejects_an_empty_stub_without_touching_the_device() {
        let mut fel = device(Vec::new());
        assert_eq!(
            fel.run_ddr_stub(b""),
            Err(FelError::EmptyImage {
                what: "DDR-init stub"
            })
        );
        fel.into_inner().assert_done();
    }

    #[test]
    fn rejects_an_empty_flasher_without_touching_the_device() {
        let mut fel = device(Vec::new());
        assert_eq!(
            fel.run_flash_job(b"", FlashJob::new(1, FLASH_COMMAND_NOOP, 0, 0, 0)),
            Err(FelError::EmptyImage { what: "flasher" })
        );
        fel.into_inner().assert_done();
    }

    #[test]
    fn rejects_an_egon_stub_without_touching_the_device() {
        let mut fel = device(Vec::new());
        assert_eq!(
            fel.run_ddr_stub(b"eGON.BT0"),
            Err(FelError::UnexpectedEgon {
                what: "DDR-init stub"
            })
        );
        fel.into_inner().assert_done();
    }

    #[test]
    fn rejects_an_egon_flasher_without_touching_the_device() {
        let mut fel = device(Vec::new());
        assert_eq!(
            fel.run_flash_job(b"eGON.BT0", FlashJob::new(1, FLASH_COMMAND_NOOP, 0, 0, 0)),
            Err(FelError::UnexpectedEgon { what: "flasher" })
        );
        fel.into_inner().assert_done();
    }

    #[test]
    fn rejects_an_oversized_stub_without_touching_the_device() {
        let mut fel = device(Vec::new());
        assert_eq!(
            fel.run_ddr_stub(&vec![0u8; DDR_STUB_MAX_SIZE as usize + 1]),
            Err(FelError::StubTooLarge {
                size: DDR_STUB_MAX_SIZE as usize + 1,
                max: DDR_STUB_MAX_SIZE,
            })
        );
        fel.into_inner().assert_done();
    }

    #[test]
    fn verifies_a_dram_pattern() {
        let mut expected = Vec::new();
        for index in 0..DRAM_TEST_SIZE / 4 {
            expected.extend_from_slice(&dram_pattern(index).to_le_bytes());
        }
        let mut steps = write_steps(DRAM_TEST_ADDR, &expected);
        steps.extend(read_steps(DRAM_TEST_ADDR, &expected));

        let mut fel = device(steps);
        assert_eq!(fel.verify_dram(), Ok(()));
        fel.into_inner().assert_done();
    }

    #[test]
    fn reports_the_first_mismatching_dram_word() {
        let mut expected = Vec::new();
        for index in 0..DRAM_TEST_SIZE / 4 {
            expected.extend_from_slice(&dram_pattern(index).to_le_bytes());
        }
        let mut actual = expected.clone();
        actual[8] ^= 0xff;
        let mut steps = write_steps(DRAM_TEST_ADDR, &expected);
        steps.extend(read_steps(DRAM_TEST_ADDR, &actual));

        let bad = u32::from_le_bytes(actual[8..12].try_into().unwrap());
        let mut fel = device(steps);
        assert_eq!(
            fel.verify_dram(),
            Err(FelError::DramMismatch {
                address: DRAM_TEST_ADDR + 8,
                expected: dram_pattern(2),
                actual: bad,
            })
        );
        fel.into_inner().assert_done();
    }

    #[test]
    fn runs_a_job_and_reads_the_status() {
        let job = FlashJob::new(9, FLASH_COMMAND_NOOP, 1, 2, 3);
        let status = FlashStatus::new(9, FLASH_STEP_DONE, FLASH_RESULT_OK);
        let mut steps = write_steps(FLASH_STATUS_ADDR, &[0u8; FLASH_STATUS_SIZE]);
        steps.extend(write_steps(FLASH_JOB_ADDR, &job.to_bytes()));
        steps.extend(write_steps(FLASHER_LOAD_ADDR, b"flasher"));
        steps.extend(exec_steps(FLASHER_LOAD_ADDR));
        steps.extend(read_steps(FLASH_STATUS_ADDR, &status.to_bytes()));

        let mut fel = device(steps);
        assert_eq!(fel.run_flash_job(b"flasher", job), Ok(status));
        fel.into_inner().assert_done();
    }

    #[test]
    fn reports_a_status_for_a_different_sequence() {
        let job = FlashJob::new(9, FLASH_COMMAND_NOOP, 0, 0, 0);
        let status = FlashStatus::new(10, FLASH_STEP_DONE, FLASH_RESULT_OK);
        let mut steps = write_steps(FLASH_STATUS_ADDR, &[0u8; FLASH_STATUS_SIZE]);
        steps.extend(write_steps(FLASH_JOB_ADDR, &job.to_bytes()));
        steps.extend(write_steps(FLASHER_LOAD_ADDR, b"flasher"));
        steps.extend(exec_steps(FLASHER_LOAD_ADDR));
        steps.extend(read_steps(FLASH_STATUS_ADDR, &status.to_bytes()));

        let mut fel = device(steps);
        assert_eq!(
            fel.run_flash_job(b"flasher", job),
            Err(FelError::StatusSequence {
                expected: 9,
                actual: 10,
            })
        );
        fel.into_inner().assert_done();
    }

    #[test]
    fn rejects_an_oversized_flasher_without_touching_the_device() {
        let mut fel = device(Vec::new());
        assert_eq!(
            fel.run_flash_job(
                &vec![0u8; FLASHER_MAX_SIZE as usize + 1],
                FlashJob::new(1, FLASH_COMMAND_NOOP, 0, 0, 0)
            ),
            Err(FelError::FlasherTooLarge {
                size: FLASHER_MAX_SIZE as usize + 1,
                max: FLASHER_MAX_SIZE,
            })
        );
        fel.into_inner().assert_done();
    }

    fn dram_verify_steps() -> Vec<Step> {
        let mut expected = Vec::new();
        for index in 0..DRAM_TEST_SIZE / 4 {
            expected.extend_from_slice(&dram_pattern(index).to_le_bytes());
        }
        let mut steps = write_steps(DRAM_TEST_ADDR, &expected);
        steps.extend(read_steps(DRAM_TEST_ADDR, &expected));
        steps
    }

    fn sealed_loader() -> Vec<u8> {
        use rawboot_image::{EgonHeader, SealOptions, seal};
        seal(&EgonHeader::PLACEHOLDER.to_bytes(), SealOptions::BLOCK).unwrap()
    }

    fn request<'a>(
        stub: &'a [u8],
        flasher: &'a [u8],
        payload: &'a [u8],
        args: &'a [u8],
        loader: &'a [u8],
        sequence: u32,
    ) -> FlashSdRequest<'a> {
        FlashSdRequest {
            stub,
            flasher,
            payload,
            args,
            loader,
            sequence,
        }
    }

    fn ddr_steps(stub: &[u8], dram_size_mib: u32) -> Vec<Step> {
        let mut steps = write_steps(DDR_STUB_LOAD_ADDR, stub);
        steps.extend(write_steps(DDR_RESULT_ADDR, &[0u8; DDR_RESULT_SIZE]));
        steps.extend(exec_steps(DDR_STUB_LOAD_ADDR));
        steps.extend(read_steps(
            DDR_RESULT_ADDR,
            &DdrResult::new(dram_size_mib).to_bytes(),
        ));
        steps
    }

    fn sd_flash_steps(
        stub: &[u8],
        flasher: &[u8],
        payload: &[u8],
        args: Option<&[u8]>,
        loader: &[u8],
        plan: &FlashPlan,
        sequence: u32,
    ) -> (Vec<Step>, FlashStatus) {
        let job = FlashJob::new(sequence, FLASH_COMMAND_FLASH_SD, 0, 0, 0);
        let status = FlashStatus::new(sequence, FLASH_STEP_DONE, FLASH_RESULT_OK);

        let mut steps = ddr_steps(stub, 128);
        steps.extend(dram_verify_steps());
        steps.extend(write_steps(FLASH_PLAN_ADDR, &plan.to_bytes()));
        steps.extend(write_steps(FLASH_PAYLOAD_ADDR, payload));
        if let Some(args) = args {
            steps.extend(write_steps(FLASH_ARGS_ADDR, args));
        }
        steps.extend(write_steps(FLASH_LOADER_ADDR, loader));
        steps.extend(write_steps(FLASH_STATUS_ADDR, &[0u8; FLASH_STATUS_SIZE]));
        steps.extend(write_steps(FLASH_JOB_ADDR, &job.to_bytes()));
        steps.extend(write_steps(FLASHER_LOAD_ADDR, flasher));
        steps.extend(exec_steps(FLASHER_LOAD_ADDR));
        steps.extend(read_steps(FLASH_STATUS_ADDR, &status.to_bytes()));

        (steps, status)
    }

    #[test]
    fn stages_and_runs_an_sd_flash_job() {
        let stub = b"stub";
        let flasher = b"flasher";
        let payload = b"wrapped payload image";
        let args = b"console=ttyS0";
        let loader = sealed_loader();
        let sequence = 7;
        let plan = FlashPlan::new(payload, args, &loader).unwrap();
        let (steps, status) =
            sd_flash_steps(stub, flasher, payload, Some(args), &loader, &plan, sequence);

        let mut fel = device(steps);
        let report = fel
            .flash_sd_card(request(stub, flasher, payload, args, &loader, sequence))
            .unwrap();

        assert_eq!(report.dram_size_mib, 128);
        assert_eq!(report.status, status);
        fel.into_inner().assert_done();
    }

    #[test]
    fn an_sd_flash_without_boot_cfg_skips_the_args_region() {
        let stub = b"stub";
        let flasher = b"flasher";
        let payload = b"payload";
        let loader = sealed_loader();
        let sequence = 3;
        let plan = FlashPlan::new(payload, &[], &loader).unwrap();
        let (steps, status) =
            sd_flash_steps(stub, flasher, payload, None, &loader, &plan, sequence);

        let mut fel = device(steps);
        let report = fel
            .flash_sd_card(request(stub, flasher, payload, &[], &loader, sequence))
            .unwrap();

        assert_eq!(report.status, status);
        fel.into_inner().assert_done();
    }

    #[test]
    fn rejects_an_oversized_boot_cfg_without_touching_the_device() {
        use rawboot_image::FlashContractError;

        let mut fel = device(Vec::new());
        let args = vec![0u8; rawboot_image::FLASH_ARGS_MAX_SIZE as usize + 1];
        let loader = sealed_loader();
        assert!(matches!(
            fel.flash_sd_card(request(b"stub", b"flasher", b"payload", &args, &loader, 1)),
            Err(FelError::FlashContract(FlashContractError::TooLarge {
                field: "boot.cfg contents",
                ..
            }))
        ));
        fel.into_inner().assert_done();
    }

    #[test]
    fn rejects_a_staging_window_that_does_not_fit_the_detected_dram() {
        let mut fel = device(ddr_steps(b"stub", 8));
        let loader = sealed_loader();
        let err = fel
            .flash_sd_card(request(b"stub", b"flasher", b"payload", &[], &loader, 1))
            .unwrap_err();

        assert_eq!(
            err,
            FelError::DramTooSmall {
                size_mib: 8,
                required: FLASH_STAGE_REQUIRED_SIZE as u64,
            }
        );
        fel.into_inner().assert_done();
    }

    #[test]
    fn rejects_an_unsealed_loader_without_touching_the_device() {
        let mut fel = device(Vec::new());
        assert!(matches!(
            fel.flash_sd_card(request(
                b"stub",
                b"flasher",
                b"payload",
                &[],
                &[0u8; 512],
                1
            )),
            Err(FelError::LoaderNotSealed(_))
        ));
        fel.into_inner().assert_done();
    }
}
