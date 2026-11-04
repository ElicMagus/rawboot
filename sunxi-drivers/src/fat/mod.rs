// SPDX-License-Identifier: Unlicense

//! FAT access over the SD/MMC driver.
//!
//! [`mount`] locates and mounts a FAT volume through [`block::BlockDevice`].
//! [`read_boot_files`] is the loader's convenience on top of it: a fixed
//! `payload.bin` path plus an optional `boot.cfg`, into caller-owned buffers.
//! [`create_or_replace_file`] writes or replaces a file in the volume's root
//! from a caller-owned buffer. Neither path allocates.
use crate::console::info;

pub mod block;
pub mod partition;

use core::fmt;

#[cfg(feature = "write")]
use core::cmp::min;
#[cfg(feature = "write")]
use core_io::Write;
use core_io::{Error, ErrorKind, Read, Seek, SeekFrom};
use fatfs::{FileSystem, FsOptions};

use crate::sdcard::{Sdcard_data, sdmmc_blk_read};

use block::{BlockDevice, BlockIo, SdCardIo};
use partition::{BLOCK_SIZE, MAX_CANDIDATES, Partition};

/// The fixed payload path on the FAT volume.
pub const PAYLOAD_FILE: &str = "payload.bin";

/// The optional argument file on the FAT volume.
pub const ARGS_FILE: &str = "boot.cfg";

/// The files a boot reads from the FAT volume.
pub struct BootFiles {
    /// Bytes read from [`PAYLOAD_FILE`].
    pub payload_len: usize,
    /// Bytes read from [`ARGS_FILE`] before its NUL terminator, or `None` when
    /// the file is absent.
    pub args_len: Option<usize>,
}

/// Why one of the boot files could not be read.
#[derive(Debug)]
pub enum FileError {
    /// The file exists but cannot be opened.
    Open(Error),
    /// The file does not fit in the caller's buffer, terminator included.
    TooLarge { size: u64, capacity: usize },
    /// The file was opened but could not be read in full.
    Read(Error),
}

impl fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open(err) => write!(f, "cannot open: {err}"),
            Self::TooLarge { size, capacity } => {
                write!(f, "is {size} bytes, but the buffer holds {capacity}")
            }
            Self::Read(err) => write!(f, "cannot read: {err}"),
        }
    }
}

/// Why a file could not be created, replaced, or flushed.
#[cfg(feature = "write")]
#[derive(Debug)]
pub enum WriteError {
    /// The root directory could not create or open the entry.
    Create(Error),
    /// The existing contents could not be discarded.
    Truncate(Error),
    /// The caller's bytes could not be written in full.
    Write(Error),
    /// The final dirty block could not be written to the card.
    Flush(Error),
}

#[cfg(feature = "write")]
impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Create(err) => write!(f, "cannot create: {err}"),
            Self::Truncate(err) => write!(f, "cannot truncate: {err}"),
            Self::Write(err) => write!(f, "cannot write: {err}"),
            Self::Flush(err) => write!(f, "cannot flush: {err}"),
        }
    }
}

/// Why the boot files could not be read.
#[derive(Debug)]
pub enum FatError {
    /// The SD/MMC card did not initialise.
    CardOffline,
    /// The first block of the card could not be read.
    ReadFirstSector,
    /// Neither a bare FAT volume nor a FAT partition was found.
    NoPartition,
    /// `fatfs` rejected every candidate volume.
    Mount(Error),
    /// [`PAYLOAD_FILE`] is missing or unreadable.
    Payload(FileError),
    /// [`ARGS_FILE`] exists but cannot be read.
    Args(FileError),
}

impl fmt::Display for FatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CardOffline => f.write_str("SD/MMC card is not online"),
            Self::ReadFirstSector => f.write_str("cannot read the first SD block"),
            Self::NoPartition => f.write_str("no FAT partition found on the card"),
            Self::Mount(err) => write!(f, "cannot mount FAT: {err}"),
            Self::Payload(err) => write!(f, "{PAYLOAD_FILE}: {err}"),
            Self::Args(err) => write!(f, "{ARGS_FILE}: {err}"),
        }
    }
}

/// Mounts the FAT volume, reads [`PAYLOAD_FILE`] into `payload_buf` and the
/// optional [`ARGS_FILE`] into `args_buf`.
///
/// The whole payload must fit; a truncated read is reported, never silently
/// accepted. A present argument file is copied into `args_buf` and terminated
/// with NUL, whose byte is included in the buffer's capacity. A missing file is
/// not an error: `args_len` is `None` and the caller passes a null pointer.
pub fn read_boot_files(
    sd: &mut Sdcard_data,
    payload_buf: &mut [u8],
    args_buf: &mut [u8],
) -> Result<BootFiles, FatError> {
    let (mounted, partition) = mount(sd)?;
    info!(
        "FAT: mounted {:?} at LBA {} ({} bytes)\r\n",
        mounted.fat_type(),
        partition.base_lba,
        partition.len_bytes
    );

    let root = mounted.root_dir();
    let mut payload = root
        .open_file(PAYLOAD_FILE)
        .map_err(|err| FatError::Payload(FileError::Open(err)))?;
    let payload_len = read_file(&mut payload, payload_buf).map_err(FatError::Payload)?;

    let args_len = match root.open_file(ARGS_FILE) {
        Ok(mut args) => Some(read_args_file(&mut args, args_buf).map_err(FatError::Args)?),
        Err(err) if err.kind() == ErrorKind::NotFound => None,
        Err(err) => return Err(FatError::Args(FileError::Open(err))),
    };

    Ok(BootFiles {
        payload_len,
        args_len,
    })
}

/// Creates `name` in the volume's root or replaces its contents with `data`.
///
/// `data` belongs to the caller, so a flashing image can hand over its staged
/// bytes without an allocator or a copy. The file is truncated first, then
/// written and flushed; a failure at any step is returned with the step name.
/// The caller still owns `fs` and should unmount it after the write.
#[cfg(feature = "write")]
pub fn create_or_replace_file<B: BlockIo>(
    fs: &FileSystem<BlockDevice<B>>,
    name: &str,
    data: &[u8],
) -> Result<(), WriteError> {
    let root = fs.root_dir();
    let mut file = root.create_file(name).map_err(WriteError::Create)?;
    file.truncate().map_err(WriteError::Truncate)?;
    file.write_all(data).map_err(WriteError::Write)?;
    file.flush().map_err(WriteError::Flush)
}

/// Compares a file in the volume's root with `expected` byte by byte.
///
/// Returns `Ok(false)` for a missing file or a different length; a read error is
/// returned separately so the caller can name the failed step.
#[cfg(feature = "write")]
pub fn file_matches<B: BlockIo>(
    fs: &FileSystem<BlockDevice<B>>,
    name: &str,
    expected: &[u8],
) -> Result<bool, FileError> {
    let root = fs.root_dir();
    let mut file = match root.open_file(name) {
        Ok(file) => file,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(FileError::Open(err)),
    };

    let size = file.seek(SeekFrom::End(0)).map_err(FileError::Read)?;
    if size != expected.len() as u64 {
        return Ok(false);
    }
    file.seek(SeekFrom::Start(0)).map_err(FileError::Read)?;

    let mut offset = 0;
    let mut buf = [0u8; 512];
    while offset < expected.len() {
        let count = min(buf.len(), expected.len() - offset);
        file.read_exact(&mut buf[..count])
            .map_err(FileError::Read)?;
        if buf[..count] != expected[offset..offset + count] {
            return Ok(false);
        }
        offset += count;
    }
    Ok(true)
}

fn read_file(file: &mut (impl Read + Seek), buf: &mut [u8]) -> Result<usize, FileError> {
    let size = file.seek(SeekFrom::End(0)).map_err(FileError::Read)?;
    if size > buf.len() as u64 {
        return Err(FileError::TooLarge {
            size,
            capacity: buf.len(),
        });
    }

    let size = size as usize;
    file.seek(SeekFrom::Start(0)).map_err(FileError::Read)?;
    file.read_exact(&mut buf[..size]).map_err(FileError::Read)?;
    Ok(size)
}

/// Reads the argument file, leaving room for the NUL terminator.
fn read_args_file(file: &mut (impl Read + Seek), buf: &mut [u8]) -> Result<usize, FileError> {
    let (terminator, content) = buf
        .split_last_mut()
        .expect("the argument window always reserves a NUL slot");

    let len = read_file(file, content)?;
    *terminator = 0;
    Ok(len)
}

/// Mounts the first FAT volume on `sd`, probing a bare volume and then the MBR
/// partitions.
pub fn mount(
    sd: &mut Sdcard_data,
) -> Result<(FileSystem<BlockDevice<SdCardIo<'_>>>, Partition), FatError> {
    let candidates = find_candidates(sd)?;
    let mut chosen = None;
    let mut last_error = None;

    // Probe candidates in order; a bare-volume false positive falls through to
    // the MBR entries. The probe's filesystem is dropped before the real mount.
    for partition in candidates.into_iter().flatten() {
        let device = BlockDevice::new(SdCardIo::new(sd), partition);
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

    let fs = mount_partition(SdCardIo::new(sd), partition)?;
    Ok((fs, partition))
}

/// Mounts a FAT volume at `partition` on an arbitrary block device.
///
/// The loader reaches this through [`mount`]; host tests supply an in-memory
/// [`BlockIo`].
pub fn mount_partition<B: BlockIo>(
    io: B,
    partition: Partition,
) -> Result<FileSystem<BlockDevice<B>>, FatError> {
    FileSystem::new(BlockDevice::new_flushing(io, partition), FsOptions::new())
        .map_err(FatError::Mount)
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

#[cfg(all(test, feature = "write"))]
mod tests;
