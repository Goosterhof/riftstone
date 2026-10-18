//! The DSSS container: a 16-byte header, the encrypted slices, and a footer with the length
//! and a signature.
//!
//! ```text
//! 0      "DSSS"  u32 version (2)  u32 flags  u32 0
//! 16     the Mandarin slices (mandarin.rs)
//! end-140  128 bytes of salt
//! end-12   u64 decrypted length
//! end-4    u32 MurmurHash3 x86_32 (seed 0xFFFFFFFF) of every byte before it
//! ```
//!
//! Flags: `0x10` Mandarin, `0x08` deflate (the decrypted block is then u64 `zsize + 16`,
//! u32 1, u32 `zsize`, u64 size and `zsize` bytes of raw deflate). The 2026 slots carry both.

use std::io::Read;

use flate2::read::DeflateDecoder;
use thiserror::Error;

use crate::mandarin::{self, MandarinError};

const MAGIC: &[u8; 4] = b"DSSS";
const VERSION: u32 = 2;
const FLAG_DEFLATE: u32 = 0x08;
const FLAG_MANDARIN: u32 = 0x10;
const HEADER: usize = 16;
const FOOTER: usize = 0x8C;
const SIGNATURE_SEED: u32 = 0xFFFF_FFFF;
/// A payload this stone will inflate at most (the investor's slot inflates to 3.8 MB).
const MAX_PAYLOAD: u64 = 256 << 20;
/// The largest file that can still open: the largest payload, a 528-byte header for every
/// slice (a slice covers at least 16 KiB), one slice of rounding, and the container's own
/// header and footer. A reader that checks a file's size against this first never reads a
/// larger file whole.
pub const MAX_FILE: u64 = MAX_PAYLOAD
    + (MAX_PAYLOAD / (1 << 14) + 2) * mandarin::SLICE_HEADER as u64
    + (128 << 10)
    + (HEADER + FOOTER) as u64;
// The ceiling stays near the payload's: the size check is a guard, not a second limit.
const _: () = assert!(MAX_FILE < MAX_PAYLOAD + (16 << 20));

/// Why a save would not open. No variant carries a byte of the file.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SaveError {
    #[error("not a Dragon's Dogma 2 save (no DSSS header)")]
    NotDsss,
    #[error("a DSSS version this stone does not read ({0})")]
    Version(u32),
    #[error("a DSSS cipher this stone does not read (flags {0:#x})")]
    Cipher(u32),
    #[error("the save is shorter than its own header and footer")]
    Truncated,
    #[error("the save's signature does not match its bytes")]
    Signature,
    #[error("the save's slices do not have the length the payload asks for")]
    SliceLayout,
    #[error("the save does not open with this account's key")]
    WrongAccount,
    #[error("the save's payload would not inflate")]
    Inflate,
    #[error("the save's payload is larger than this stone reads ({0} bytes)")]
    TooLarge(u64),
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().expect("4 bytes"))
}

fn u64_at(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..at + 8].try_into().expect("8 bytes"))
}

/// Open a save read by the caller (`bytes`) with the Steam account id it was found under, and
/// return its payload: decrypted and, for `0x08` saves, inflated. The payload's stream starts
/// at file offset 16, which matters for its alignment ([`crate::stream::parse`] knows).
pub fn open(bytes: &[u8], account_id: u32) -> Result<Vec<u8>, SaveError> {
    // Too short to hold even the magic: the game empties a slot before it writes it again.
    if bytes.len() < MAGIC.len() {
        return Err(SaveError::Truncated);
    }
    if &bytes[..4] != MAGIC {
        return Err(SaveError::NotDsss);
    }
    if bytes.len() < HEADER + FOOTER {
        return Err(SaveError::Truncated);
    }
    let version = u32_at(bytes, 4);
    if version != VERSION {
        return Err(SaveError::Version(version));
    }
    let flags = u32_at(bytes, 8);
    if flags & FLAG_MANDARIN == 0 || flags & !(FLAG_MANDARIN | FLAG_DEFLATE) != 0 {
        return Err(SaveError::Cipher(flags));
    }
    let end = bytes.len();
    if bytes.len() % 4 != 0
        || rift_pak::murmur3_32(&bytes[..end - 4], SIGNATURE_SEED) != u32_at(bytes, end - 4)
    {
        return Err(SaveError::Signature);
    }
    let len = u64_at(bytes, end - 12);
    if len > MAX_PAYLOAD {
        return Err(SaveError::TooLarge(len));
    }
    let block =
        mandarin::decrypt(&bytes[HEADER..end - FOOTER], len, account_id).map_err(|e| match e {
            MandarinError::SliceLayout => SaveError::SliceLayout,
            MandarinError::WrongAccount => SaveError::WrongAccount,
        })?;
    if flags & FLAG_DEFLATE == 0 {
        return Ok(block);
    }
    if block.len() < 24 {
        return Err(SaveError::Inflate);
    }
    let zsize = u64::from(u32_at(&block, 12));
    let size = u64_at(&block, 16);
    if size > MAX_PAYLOAD {
        return Err(SaveError::TooLarge(size));
    }
    let zend = 24usize
        .checked_add(zsize as usize)
        .filter(|&z| z <= block.len())
        .ok_or(SaveError::Inflate)?;
    let mut payload = Vec::with_capacity(size as usize);
    DeflateDecoder::new(&block[24..zend])
        .take(size + 1)
        .read_to_end(&mut payload)
        .map_err(|_| SaveError::Inflate)?;
    if payload.len() as u64 != size {
        return Err(SaveError::Inflate);
    }
    Ok(payload)
}

/// Seal an invented payload into a save for `account_id` — tests only. `deflate` gives the
/// 2026 slots' `0x18` form.
#[cfg(any(test, feature = "fixtures"))]
pub fn seal(payload: &[u8], account_id: u32, deflate: bool) -> Vec<u8> {
    use std::io::Write;

    let (flags, block) = if deflate {
        let mut z = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        z.write_all(payload).expect("deflate in memory");
        let z = z.finish().expect("deflate in memory");
        let mut block = Vec::with_capacity(z.len() + 24);
        block.extend_from_slice(&(z.len() as u64 + 16).to_le_bytes());
        block.extend_from_slice(&1u32.to_le_bytes());
        block.extend_from_slice(&(z.len() as u32).to_le_bytes());
        block.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        block.extend_from_slice(&z);
        (FLAG_MANDARIN | FLAG_DEFLATE, block)
    } else {
        (FLAG_MANDARIN, payload.to_vec())
    };
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&mandarin::encrypt(&block, account_id));
    out.extend_from_slice(&[0x5Au8; 128]);
    out.extend_from_slice(&(block.len() as u64).to_le_bytes());
    let signature = rift_pak::murmur3_32(&out, SIGNATURE_SEED);
    out.extend_from_slice(&signature.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invented_payload(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i % 97) as u8).collect()
    }

    #[test]
    fn a_sealed_save_opens_both_ways() {
        for deflate in [false, true] {
            for len in [0usize, 24, 16_384, 70_000] {
                let payload = invented_payload(len);
                let save = seal(&payload, 5_550_123, deflate);
                assert_eq!(
                    open(&save, 5_550_123).unwrap(),
                    payload,
                    "deflate {deflate}, len {len}"
                );
            }
        }
    }

    #[test]
    fn another_accounts_save_says_so() {
        let save = seal(&invented_payload(500), 5_550_123, true);
        assert_eq!(open(&save, 5_550_124), Err(SaveError::WrongAccount));
    }

    #[test]
    fn a_flipped_byte_breaks_the_signature() {
        let mut save = seal(&invented_payload(500), 1, true);
        save[100] ^= 1;
        assert_eq!(open(&save, 1), Err(SaveError::Signature));
    }

    #[test]
    fn a_cut_or_foreign_file_is_refused_without_a_panic() {
        let save = seal(&invented_payload(500), 1, true);
        // Too short to hold the magic is cut, not foreign: the game empties a slot before it
        // writes it, and the reader looks at a cut file again.
        assert_eq!(open(&[], 1), Err(SaveError::Truncated));
        assert_eq!(open(&save[..3], 1), Err(SaveError::Truncated));
        assert_eq!(open(&save[..100], 1), Err(SaveError::Truncated));
        assert_eq!(
            open(b"PK\x03\x04 not a save at all, a zip pretending", 1),
            Err(SaveError::NotDsss)
        );
        let mut v3 = save.clone();
        v3[4] = 3;
        assert_eq!(open(&v3, 1), Err(SaveError::Version(3)));
    }

    #[test]
    fn no_save_that_opens_is_larger_than_the_ceiling() {
        let largest = HEADER as u64 + mandarin::encrypted_len(MAX_PAYLOAD) + FOOTER as u64;
        assert!(largest <= MAX_FILE, "{largest} > {MAX_FILE}");
    }
}
