// SPDX-License-Identifier: Unlicense

//! FAT access for the payload image.
//!
//! The card is mounted through `fatfs` over a read-only block adapter. The
//! fixed payload path makes the boot flow predictable; the caller supplies the
//! destination buffer, so this module never allocates.

mod block;
mod partition;

use core::fmt;

use core_io::{Error, Read, Seek, SeekFrom};
use fatfs::{FileSystem, FsOptions};

use crate::print;
use crate::sdcard::{Sdcard_data, sdmmc_blk_read};

use block::BlockDevice;
use partition::{BLOCK_SIZE, MAX_CANDIDATES, Partition};

/// The fixed payload path on the FAT volume.
pub(crate) const PAYLOAD_FILE: &str = "payload.bin";

/// Why the payload could not be read.
pub(crate) enum FatError {
    /// The SD/MMC card did not initialise.
    CardOffline,
    /// The first block of the card could not be read.
    ReadFirstSector,
    /// Neither a bare FAT volume nor a FAT partition was found.
    NoPartition,
    /// `fatfs` rejected every candidate volume.
    Mount(Error),
    /// The payload file is missing or cannot be opened.
    NoPayload(Error),
    /// The file does not fit in the caller's buffer.
    TooLarge { size: u64, capacity: usize },
    /// The file was opened but could not be read in full.
    Read(Error),
}

impl fmt::Display for FatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CardOffline => f.write_str("SD/MMC card is not online"),
            Self::ReadFirstSector => f.write_str("cannot read the first SD block"),
            Self::NoPartition => f.write_str("no FAT partition found on the card"),
            Self::Mount(err) => write!(f, "cannot mount FAT: {err}"),
            Self::NoPayload(err) => write!(f, "cannot open {PAYLOAD_FILE}: {err}"),
            Self::TooLarge { size, capacity } => write!(
                f,
                "{PAYLOAD_FILE} is {size} bytes, but the buffer holds {capacity}"
            ),
            Self::Read(err) => write!(f, "cannot read {PAYLOAD_FILE}: {err}"),
        }
    }
}

/// Mounts the FAT volume and reads [`PAYLOAD_FILE`] into `buf`.
///
/// Returns the number of bytes read. The whole file must fit: a truncated read
/// is reported, never silently accepted.
pub(crate) fn read_payload(sd: &mut Sdcard_data, buf: &mut [u8]) -> Result<usize, FatError> {
    let (mounted, partition) = mount(sd)?;
    print!(
        "FAT: mounted {:?} at LBA {} ({} bytes)\r\n",
        mounted.fat_type(),
        partition.base_lba,
        partition.len_bytes
    );

    let root = mounted.root_dir();
    let mut file = root.open_file(PAYLOAD_FILE).map_err(FatError::NoPayload)?;

    let size = file.seek(SeekFrom::End(0)).map_err(FatError::Read)?;
    if size > buf.len() as u64 {
        return Err(FatError::TooLarge {
            size,
            capacity: buf.len(),
        });
    }

    let size = size as usize;
    file.seek(SeekFrom::Start(0)).map_err(FatError::Read)?;
    file.read_exact(&mut buf[..size]).map_err(FatError::Read)?;
    Ok(size)
}

fn mount(sd: &mut Sdcard_data) -> Result<(FileSystem<BlockDevice<'_>>, Partition), FatError> {
    let candidates = find_candidates(sd)?;
    let mut chosen = None;
    let mut last_error = None;

    // Probe candidates in order; a bare-volume false positive falls through to
    // the MBR entries. The probe's filesystem is dropped before the real mount.
    for partition in candidates.into_iter().flatten() {
        let device = BlockDevice::new(sd, partition);
        match FileSystem::new(device, FsOptions::new()) {
            Ok(fs) => {
                drop(fs);
                chosen = Some(partition);
                break;
            }
            Err(err) => last_error = Some(err),
        }
    }

    let Some(partition) = chosen else {
        return Err(match last_error {
            Some(err) => FatError::Mount(err),
            None => FatError::NoPartition,
        });
    };

    let device = BlockDevice::new(sd, partition);
    let fs = FileSystem::new(device, FsOptions::new()).map_err(FatError::Mount)?;
    Ok((fs, partition))
}

fn find_candidates(sd: &mut Sdcard_data) -> Result<[Option<Partition>; MAX_CANDIDATES], FatError> {
    if !sd.online {
        return Err(FatError::CardOffline);
    }

    let mut sector = [0u8; BLOCK_SIZE as usize];
    if sdmmc_blk_read(sd, sector.as_mut_ptr(), 0, 1) != 1 {
        return Err(FatError::ReadFirstSector);
    }

    Ok(partition::candidates(&sector, sd.card.capacity))
}
