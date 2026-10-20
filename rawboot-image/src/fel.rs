// SPDX-License-Identifier: Unlicense

//! The FEL payload mailbox: the host-written request that makes `rawboot` boot an image uploaded
//! over USB FEL instead of reading `payload.bin` from the SD card.
//!
//! The mailbox lives in the free part of the on-chip SRAM, not in DRAM: the host writes it before
//! the loader runs, and the loader is the one that initialises DRAM. A DRAM mailbox would therefore
//! only work after an `xfel ddr` pass, which currently breaks the loader's own DRAM setup (issue
//! #23). The payload image and the optional argument string are staged in the same SRAM window;
//! after DRAM init the loader copies the image to the load address from its header, exactly as it
//! does for the SD path.
//!
//! Layout (little-endian 32-bit words):
//!
//! | offset | field      |
//! | ------ | ---------- |
//! | 0x00   | magic      |
//! | 0x04   | image_addr |
//! | 0x08   | image_size |
//! | 0x0c   | args_addr  |

/// Base of the four-word mailbox in FEL-mode SRAM.
pub const FEL_MAILBOX_BASE: u32 = 0x0004_4000;
/// First byte of the SRAM window reserved for the optional argument string.
pub const FEL_ARGS_BASE: u32 = 0x0003_8000;
/// Largest argument string, including its NUL terminator.
pub const FEL_ARGS_MAX_SIZE: u32 = FEL_PAYLOAD_BASE - FEL_ARGS_BASE;
/// First byte of the SRAM window the host may stage the payload image in.
pub const FEL_PAYLOAD_BASE: u32 = 0x0003_9000;
/// Largest payload image that fits between [`FEL_PAYLOAD_BASE`] and the mailbox.
pub const FEL_PAYLOAD_MAX_SIZE: u32 = FEL_MAILBOX_BASE - FEL_PAYLOAD_BASE;
/// `RBFM` in little-endian order: rawboot FEL mailbox.
pub const FEL_MAILBOX_MAGIC: u32 = u32::from_le_bytes(*b"RBFM");
/// Number of little-endian words in the mailbox.
pub const FEL_MAILBOX_WORDS: usize = 4;

mod offset {
    pub const MAGIC: usize = 0;
    pub const IMAGE_ADDR: usize = 4;
    pub const IMAGE_SIZE: usize = 8;
    pub const ARGS_ADDR: usize = 12;
}

/// A FEL payload request read from the mailbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FelMailbox {
    /// Address of the payload image (header plus body) staged by the host.
    pub image_addr: u32,
    /// Length of the staged image in bytes.
    pub image_size: u32,
    /// Address of the NUL-terminated argument string, or zero for none.
    pub args_addr: u32,
}

impl FelMailbox {
    /// Decode a mailbox, or return `None` when the magic is absent.
    ///
    /// A wrong magic means "not a FEL request" rather than an error: an SD boot must not be
    /// blocked by whatever the SRAM happened to contain after reset.
    pub const fn from_words(words: [u32; FEL_MAILBOX_WORDS]) -> Option<Self> {
        if words[offset::MAGIC / 4] != FEL_MAILBOX_MAGIC {
            return None;
        }
        Some(Self {
            image_addr: words[offset::IMAGE_ADDR / 4],
            image_size: words[offset::IMAGE_SIZE / 4],
            args_addr: words[offset::ARGS_ADDR / 4],
        })
    }

    /// The mailbox's little-endian word representation, as the host writes it.
    pub const fn to_words(self) -> [u32; FEL_MAILBOX_WORDS] {
        let mut words = [0u32; FEL_MAILBOX_WORDS];
        words[offset::MAGIC / 4] = FEL_MAILBOX_MAGIC;
        words[offset::IMAGE_ADDR / 4] = self.image_addr;
        words[offset::IMAGE_SIZE / 4] = self.image_size;
        words[offset::ARGS_ADDR / 4] = self.args_addr;
        words
    }
}

const _: () = {
    assert!(FEL_ARGS_MAX_SIZE == 0x1000);
    assert!(FEL_PAYLOAD_MAX_SIZE == 0xB000);
    assert!(FEL_MAILBOX_WORDS * 4 == 16);
};

#[cfg(test)]
mod tests {
    use super::*;

    const REQUEST: FelMailbox = FelMailbox {
        image_addr: FEL_PAYLOAD_BASE,
        image_size: 236,
        args_addr: 0,
    };

    #[test]
    fn decodes_a_request_written_by_the_host() {
        assert_eq!(FelMailbox::from_words(REQUEST.to_words()), Some(REQUEST));
    }

    #[test]
    fn zeroed_words_are_not_a_request() {
        assert_eq!(FelMailbox::from_words([0; FEL_MAILBOX_WORDS]), None);
    }

    #[test]
    fn a_wrong_magic_is_not_a_request() {
        let mut words = REQUEST.to_words();
        words[0] ^= 0xFF;
        assert_eq!(FelMailbox::from_words(words), None);
    }

    #[test]
    fn every_field_round_trips() {
        let request = FelMailbox {
            image_addr: 0x0003_ABCD,
            image_size: 0x1234,
            args_addr: 0x0004_0123,
        };
        assert_eq!(FelMailbox::from_words(request.to_words()), Some(request));
    }
}
