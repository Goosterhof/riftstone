//! A headers-only walk of a RAR file's block chain, run before UnRAR lists it.
//!
//! UnRAR answers "end of archive" both when it reaches the archive's end
//! block and when the file simply stops: after a valid signature followed by
//! nonsense, or in a download that was cut off. Unchecked, a damaged RAR would
//! list as an empty or partial package — the catalog would call it "nothing
//! to install" instead of "damaged". zip and 7z keep their directories at the
//! end of the file, so their readers notice truncation on their own; RAR
//! streams, so this walk follows the block chain from the signature and
//! insists on reaching the end.
//!
//! - **RAR5** (headers.hpp, arcread.cpp `ReadHeader50`): every block is
//!   `CRC32 · vint size · header [· data]`; the CRC covers the size field and
//!   the header. The chain must end in an end-of-archive block (type 5).
//! - **RAR4** (arcread.cpp `ReadHeader15`): blocks are
//!   `CRC16 · type · flags · size [· data]`. Old archives may stop without an
//!   end block, and old comment and AV blocks carry CRCs computed over only
//!   part of the header, so RAR4 is held to its framing alone: the first block
//!   must be the main header and no block may run past the end of the file.
//! - Encrypted headers (`rar -hp`) cannot be walked without the password;
//!   the walk stops there and UnRAR reports the archive as encrypted.

use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use rift_core::RiftError;

use crate::fail;

const RAR4_SIGNATURE_LEN: u64 = 7;
const RAR5_SIGNATURE_LEN: u64 = 8;

// RAR5 header types and flags (headers.hpp).
const HEAD5_MAIN: u64 = 1;
const HEAD5_CRYPT: u64 = 4;
const HEAD5_ENDARC: u64 = 5;
const HFL5_EXTRA: u64 = 0x0001;
const HFL5_DATA: u64 = 0x0002;

// RAR4 header types and flags (headers.hpp).
const HEAD3_MAIN: u8 = 0x73;
const HEAD3_FILE: u8 = 0x74;
const HEAD3_OLDSERVICE: u8 = 0x77;
const HEAD3_PROTECT: u8 = 0x78;
const HEAD3_SERVICE: u8 = 0x7a;
const HEAD3_ENDARC: u8 = 0x7b;
const MHD_PASSWORD: u16 = 0x0080;
const LHD_LARGE: u16 = 0x0100;
const LONG_BLOCK: u16 = 0x8000;

/// Walk the block chain of the RAR at `path`. `Ok` when it reaches the end
/// (or encrypted headers); `CorruptArchive` naming what is wrong otherwise.
pub(crate) fn check(path: &Path) -> Result<(), RiftError> {
    let file = File::open(path).map_err(|err| RiftError::unreadable(path, err))?;
    let len = file
        .metadata()
        .map_err(|err| RiftError::unreadable(path, err))?
        .len();
    let mut reader = BufReader::with_capacity(16 * 1024, file);
    let mut signature = [0u8; 8];
    let read =
        read_up_to(&mut reader, &mut signature).map_err(|err| fail::structure_read(path, &err))?;
    let outcome = if signature[..read].starts_with(crate::RAR5_SIGNATURE) {
        walk_rar5(&mut reader, len)
    } else if signature[..read].starts_with(crate::RAR4_SIGNATURE) {
        walk_rar4(&mut reader, len)
    } else {
        Err(Broken::Damaged(
            "it does not start with a RAR signature".to_string(),
        ))
    };
    outcome.map_err(|broken| broken.into_error(path))
}

/// Why a chain is not whole.
#[derive(Debug)]
enum Broken {
    /// The file ends before the archive does: an unfinished download.
    CutOff(String),
    /// A header is not what the format allows.
    Damaged(String),
    /// The operating system failed the read.
    Io(io::Error),
}

impl Broken {
    fn into_error(self, path: &Path) -> RiftError {
        match self {
            Broken::CutOff(detail) => RiftError::corrupt(
                path,
                format!("the archive is cut off ({detail}); it looks like an unfinished download"),
            ),
            Broken::Damaged(detail) => RiftError::corrupt(path, detail),
            Broken::Io(err) => fail::structure_read(path, &err),
        }
    }
}

impl From<io::Error> for Broken {
    fn from(err: io::Error) -> Self {
        if err.kind() == io::ErrorKind::UnexpectedEof {
            Broken::CutOff("a header stops half-way".to_string())
        } else {
            Broken::Io(err)
        }
    }
}

fn walk_rar5(reader: &mut BufReader<File>, len: u64) -> Result<(), Broken> {
    let mut position = RAR5_SIGNATURE_LEN;
    let mut first = true;
    loop {
        if position >= len {
            return Err(Broken::CutOff(if position == len {
                "it ends before its end-of-archive block".to_string()
            } else {
                "a block runs past the end of the file".to_string()
            }));
        }
        reader.seek(SeekFrom::Start(position))?;
        let mut crc = [0u8; 4];
        reader.read_exact(&mut crc)?;
        let (size, size_bytes) = read_vint(reader, 3)?;
        if size == 0 {
            return Err(Broken::Damaged(format!(
                "the header at byte {position} is empty"
            )));
        }
        let header_end = position + 4 + size_bytes.len() as u64 + size;
        if header_end > len {
            return Err(Broken::CutOff(
                "a header runs past the end of the file".to_string(),
            ));
        }
        // At most 3 vint bytes: a header is never larger than 2 MiB.
        let mut header = vec![0u8; usize::try_from(size).unwrap_or(0)];
        reader.read_exact(&mut header)?;
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&size_bytes);
        hasher.update(&header);
        if hasher.finalize() != u32::from_le_bytes(crc) {
            return Err(Broken::Damaged(format!(
                "the header at byte {position} fails its checksum"
            )));
        }
        let mut fields = &header[..];
        let kind = read_vint(&mut fields, 10)?.0;
        let flags = read_vint(&mut fields, 10)?.0;
        if flags & HFL5_EXTRA != 0 {
            read_vint(&mut fields, 10)?;
        }
        let data = if flags & HFL5_DATA != 0 {
            read_vint(&mut fields, 10)?.0
        } else {
            0
        };
        if first && kind != HEAD5_MAIN && kind != HEAD5_CRYPT {
            return Err(Broken::Damaged(
                "its first block is not a main archive header".to_string(),
            ));
        }
        first = false;
        match kind {
            HEAD5_ENDARC => return Ok(()),
            // Encrypted headers follow: only UnRAR, with a password, can go on.
            HEAD5_CRYPT => return Ok(()),
            _ => {}
        }
        position = position
            .checked_add(4 + size_bytes.len() as u64 + size)
            .and_then(|end| end.checked_add(data))
            .ok_or_else(|| {
                Broken::Damaged(format!(
                    "the block at byte {position} has an impossible size"
                ))
            })?;
    }
}

fn walk_rar4(reader: &mut BufReader<File>, len: u64) -> Result<(), Broken> {
    let mut position = RAR4_SIGNATURE_LEN;
    let mut first = true;
    loop {
        if position == len {
            // RAR 1.5–2.x archives may simply stop after their last block.
            return if first {
                Err(Broken::CutOff(
                    "there is no main archive header".to_string(),
                ))
            } else {
                Ok(())
            };
        }
        if position + 7 > len {
            return Err(Broken::CutOff(
                "a block runs past the end of the file".to_string(),
            ));
        }
        reader.seek(SeekFrom::Start(position))?;
        let mut short = [0u8; 7];
        reader.read_exact(&mut short)?;
        let kind = short[2];
        let flags = u16::from_le_bytes([short[3], short[4]]);
        let head_size_field = u16::from_le_bytes([short[5], short[6]]);
        let head_size = u64::from(head_size_field);
        if head_size < 7 {
            return Err(Broken::Damaged(format!(
                "the header at byte {position} is too short"
            )));
        }
        if first && kind != HEAD3_MAIN {
            return Err(Broken::Damaged(
                "its first block is not a main archive header".to_string(),
            ));
        }
        first = false;
        if position + head_size > len {
            return Err(Broken::CutOff(
                "a header runs past the end of the file".to_string(),
            ));
        }
        let mut rest = vec![0u8; usize::from(head_size_field) - 7];
        reader.read_exact(&mut rest)?;
        let header = [&short[..], &rest[..]].concat();
        let u32_at = |offset: usize| -> u64 {
            header.get(offset..offset + 4).map_or(0, |bytes| {
                u64::from(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
            })
        };
        let data = match kind {
            HEAD3_FILE | HEAD3_SERVICE => {
                let high = if flags & LHD_LARGE != 0 {
                    u32_at(32)
                } else {
                    0
                };
                (high << 32) | u32_at(7)
            }
            HEAD3_OLDSERVICE | HEAD3_PROTECT => u32_at(7),
            _ if flags & LONG_BLOCK != 0 => u32_at(7),
            _ => 0,
        };
        if kind == HEAD3_MAIN && flags & MHD_PASSWORD != 0 {
            // Encrypted headers follow: only UnRAR, with a password, can go on.
            return Ok(());
        }
        if kind == HEAD3_ENDARC {
            return Ok(());
        }
        position = position
            .checked_add(head_size)
            .and_then(|end| end.checked_add(data))
            .filter(|&end| end <= len)
            .ok_or_else(|| {
                Broken::CutOff("a file's data runs past the end of the file".to_string())
            })?;
    }
}

/// A RAR5 variable-length integer: 7 bits per byte, low bits first, the top
/// bit set on every byte but the last. Returns the value and its bytes.
fn read_vint(reader: &mut dyn Read, max_bytes: usize) -> Result<(u64, Vec<u8>), Broken> {
    let mut value = 0u64;
    let mut bytes = Vec::with_capacity(max_bytes);
    for shift in (0..max_bytes).map(|i| 7 * i) {
        let mut byte = [0u8; 1];
        reader.read_exact(&mut byte)?;
        bytes.push(byte[0]);
        if shift < 64 {
            value |= u64::from(byte[0] & 0x7f) << shift;
        }
        if byte[0] & 0x80 == 0 {
            return Ok((value, bytes));
        }
    }
    Err(Broken::Damaged(
        "a size field is longer than the format allows".to_string(),
    ))
}

/// Read as many bytes as the file has, up to `buf.len()`.
fn read_up_to(reader: &mut dyn Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match reader.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(read) => filled += read,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => {}
            Err(err) => return Err(err),
        }
    }
    Ok(filled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vints_read_seven_bits_at_a_time() {
        assert_eq!(read_vint(&mut &[0x05][..], 3).unwrap().0, 5);
        assert_eq!(read_vint(&mut &[0x80, 0x01][..], 3).unwrap().0, 128);
        assert_eq!(
            read_vint(&mut &[0xff, 0x7f][..], 3).unwrap(),
            (0x3fff, vec![0xff, 0x7f])
        );
        assert!(matches!(
            read_vint(&mut &[0x80, 0x80, 0x80, 0x01][..], 3),
            Err(Broken::Damaged(_))
        ));
        assert!(matches!(
            read_vint(&mut &[0x80][..], 3),
            Err(Broken::CutOff(_))
        ));
    }

    #[test]
    fn a_signature_alone_is_cut_off() {
        let dir = tempfile::tempdir().unwrap();
        for (name, bytes) in [
            ("five.rar", &b"Rar!\x1A\x07\x01\x00"[..]),
            ("four.rar", &b"Rar!\x1A\x07\x00"[..]),
        ] {
            let path = dir.path().join(name);
            std::fs::write(&path, bytes).unwrap();
            assert!(
                matches!(check(&path), Err(RiftError::CorruptArchive { detail, .. }) if detail.contains("cut off")),
                "{name}"
            );
        }
    }

    #[test]
    fn nonsense_after_the_signature_is_damaged_or_cut_off() {
        let dir = tempfile::tempdir().unwrap();
        for (name, bytes) in [
            (
                "five.rar",
                &b"Rar!\x1A\x07\x01\x00 and then nonsense, not headers"[..],
            ),
            (
                "four.rar",
                &b"Rar!\x1A\x07\x00 and then nonsense, not headers"[..],
            ),
        ] {
            let path = dir.path().join(name);
            std::fs::write(&path, bytes).unwrap();
            assert!(
                matches!(check(&path), Err(RiftError::CorruptArchive { .. })),
                "{name}"
            );
        }
    }
}
