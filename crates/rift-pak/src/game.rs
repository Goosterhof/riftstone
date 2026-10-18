//! The game's own pak (`re_chunk_000.pak`), read-only: its encrypted table of contents and the
//! entries the Muster names things from (v0.4).
//!
//! A game pak is KPKA v4 with feature flag `0x0008`: the entry table is XOR-encrypted with a key
//! that sits, RSA-encrypted, behind it. Dragon's Dogma 2's own pak carries `0x0068`; the blocks
//! behind its table, in order, are:
//!
//! | flag     | block |
//! |----------|-------|
//! | `0x0040` | a chunk table of 16 bytes per chunk (u32 count, u32 block size, then offset, size, hash) |
//! | `0x0010` | one u32 |
//! | `0x0004` | nine bytes |
//! | `0x0008` | the 128-byte key |
//! | `0x0020` | a chunk table of 8 bytes per chunk (u32 block size, u32 count, then offset, size) |
//!
//! The key is the RSA public operation (`e = 65537` and the 1024-bit modulus every RE Engine
//! pak reader publishes) on the 128 little-endian bytes, and the table is decrypted with
//! `t[i] ^= i + k[i % 32] × k[i % 29]`. An entry is raw (compression 0), raw deflate (1) or
//! zstd (2); one whose attributes are exactly `0x1000000`, `0x1000400`, `0x1000800` or
//! `0x1000C00` is stored in the chunk table's 512 KiB chunks, each zstd unless it is exactly
//! 512 KiB. Formats after ree-pak-rs (MIT, eigeen) and the facts REE.PAK.Tool documents; this
//! file is written fresh from them.
//!
//! Measured on the investor's install (2026-10-18): 318,966 entries, the table read and
//! decrypted in 2.3 s over the WSL bench's mount. One entry in the whole pak is an encrypted
//! resource (attribute bits 16–23); none the Muster reads is, so the stone refuses those
//! rather than guess at them.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Mutex;

use num_bigint::BigUint;

use crate::{path_hash, ENTRY_LEN, FLAG_ENCRYPTED_TABLE, HEADER_LEN, MAGIC, MAX_ENTRIES};

const TABLE_MODULUS_LE: [u8; 128] = [
    0x7D, 0x0B, 0xF8, 0xC1, 0x7C, 0x23, 0xFD, 0x3B, 0xD4, 0x75, 0x16, 0xD2, 0x33, 0x21, 0xD8, 0x10,
    0x71, 0xF9, 0x7C, 0xD1, 0x34, 0x93, 0xBA, 0x77, 0x26, 0xFC, 0xAB, 0x2C, 0xEE, 0xDA, 0xD9, 0x1C,
    0x89, 0xE7, 0x29, 0x7B, 0xDD, 0x8A, 0xAE, 0x50, 0x39, 0xB6, 0x01, 0x6D, 0x21, 0x89, 0x5D, 0xA5,
    0xA1, 0x3E, 0xA2, 0xC0, 0x8C, 0x93, 0x13, 0x36, 0x65, 0xEB, 0xE8, 0xDF, 0x06, 0x17, 0x67, 0x96,
    0x06, 0x2B, 0xAC, 0x23, 0xED, 0x8C, 0xB7, 0x8B, 0x90, 0xAD, 0xEA, 0x71, 0xC4, 0x40, 0x44, 0x9D,
    0x1C, 0x7B, 0xBA, 0xC4, 0xB6, 0x2D, 0xD6, 0xD2, 0x4B, 0x62, 0xD6, 0x26, 0xFC, 0x74, 0x20, 0x07,
    0xEC, 0xE3, 0x59, 0x9A, 0xE6, 0xAF, 0xB9, 0xA8, 0x35, 0x8B, 0xE0, 0xE8, 0xD3, 0xCD, 0x45, 0x65,
    0xB0, 0x91, 0xC4, 0x95, 0x1B, 0xF3, 0x23, 0x1E, 0xC6, 0x71, 0xCF, 0x3E, 0x35, 0x2D, 0x6B, 0xE3,
];
const TABLE_EXPONENT: u32 = 65_537;
const KEY_LEN: usize = 128;
const CHUNK_SIZE: u64 = 512 * 1024;
const CHUNKED: [u64; 4] = [0x100_0000, 0x100_0400, 0x100_0800, 0x100_0C00];
/// The largest entry the stone will read into memory (the Muster's largest is 0.8 MB).
const MAX_ENTRY: u64 = 64 << 20;

/// Why the game's pak, or one entry in it, could not be read. No variant names a path on disk.
#[derive(Debug, thiserror::Error)]
pub enum GamePakError {
    #[error("could not read the game's pak: {0}")]
    Io(#[from] std::io::Error),
    #[error("the game's pak is not one the stone reads ({0})")]
    Format(&'static str),
    #[error("the game's pak entry is an encrypted resource, which the stone does not open")]
    EncryptedResource,
    #[error("the game's pak entry would not decompress")]
    Decompress,
    #[error("the game's pak entry is larger than the stone reads")]
    TooLarge,
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    offset: u64,
    compressed: u64,
    size: u64,
    attributes: u64,
}

/// The game's pak, opened read-only, its table of contents decrypted.
pub struct GamePak {
    file: Mutex<File>,
    entries: HashMap<u64, Entry>,
    /// The 8-byte chunk table, as (offset, size).
    chunks: Vec<(u64, u64)>,
}

impl std::fmt::Debug for GamePak {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GamePak")
            .field("entries", &self.entries.len())
            .field("chunks", &self.chunks.len())
            .finish()
    }
}

fn read_u32(r: &mut impl Read) -> std::io::Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}

/// The table's XOR key: the RSA public operation on the stored 128 bytes, little-endian.
fn table_key(stored: &[u8; KEY_LEN]) -> Vec<u8> {
    let modulus = BigUint::from_bytes_le(&TABLE_MODULUS_LE);
    let mut key = BigUint::from_bytes_le(stored)
        .modpow(&BigUint::from(TABLE_EXPONENT), &modulus)
        .to_bytes_le();
    key.resize(KEY_LEN.max(key.len()), 0);
    key
}

/// Decrypt (or, being XOR, encrypt) a table in place.
fn xor_table(table: &mut [u8], key: &[u8]) {
    for (i, b) in table.iter_mut().enumerate() {
        let k = u32::from(key[i % 32]) * u32::from(key[i % 29]);
        *b ^= (i as u32).wrapping_add(k) as u8;
    }
}

fn chunk_table(r: &mut impl Read, wide: bool) -> Result<Vec<(u64, u64)>, GamePakError> {
    let (first, second) = (read_u32(r)?, read_u32(r)?);
    let count = if wide { first } else { second } as usize;
    if count > (1 << 24) {
        return Err(GamePakError::Format("a chunk table claims too many chunks"));
    }
    let width = if wide { 16 } else { 8 };
    let mut raw = vec![0u8; count * width];
    r.read_exact(&mut raw)?;
    Ok(raw
        .chunks_exact(width)
        .map(|c| {
            let offset = u32::from_le_bytes(c[0..4].try_into().expect("4"));
            let size = u32::from_le_bytes(c[4..8].try_into().expect("4"));
            (
                u64::from(offset) + (u64::from(size & 0x3FF) << 32),
                u64::from(size >> 10),
            )
        })
        .collect())
}

impl GamePak {
    /// Open the pak at `path` read-only and decrypt its table of contents. Nothing is written.
    pub fn open(path: &Path) -> Result<GamePak, GamePakError> {
        let mut file = File::open(path)?;
        let mut header = [0u8; HEADER_LEN];
        file.read_exact(&mut header)?;
        if header[0..4] != MAGIC {
            return Err(GamePakError::Format("no KPKA magic"));
        }
        if header[4] != 4 {
            return Err(GamePakError::Format("not a version 4 pak"));
        }
        let flags = u16::from_le_bytes([header[6], header[7]]);
        let count = u32::from_le_bytes(header[8..12].try_into().expect("4"));
        if count > MAX_ENTRIES {
            return Err(GamePakError::Format("the header claims too many files"));
        }
        let mut table = vec![0u8; count as usize * ENTRY_LEN];
        file.read_exact(&mut table)?;
        let mut chunks = Vec::new();
        if flags & 0x0040 != 0 {
            chunk_table(&mut file, true)?;
        }
        if flags & 0x0010 != 0 {
            file.seek(SeekFrom::Current(4))?;
        }
        if flags & 0x0004 != 0 {
            file.seek(SeekFrom::Current(9))?;
        }
        if flags & FLAG_ENCRYPTED_TABLE != 0 {
            let mut stored = [0u8; KEY_LEN];
            file.read_exact(&mut stored)?;
            xor_table(&mut table, &table_key(&stored));
        }
        if flags & 0x0020 != 0 {
            chunks = chunk_table(&mut file, false)?;
        }
        let entries = table
            .chunks_exact(ENTRY_LEN)
            .map(|e| {
                let u = |at: usize| u64::from_le_bytes(e[at..at + 8].try_into().expect("8"));
                let lower = u64::from(u32::from_le_bytes(e[0..4].try_into().expect("4")));
                let upper = u64::from(u32::from_le_bytes(e[4..8].try_into().expect("4")));
                (
                    (upper << 32) | lower,
                    Entry {
                        offset: u(8),
                        compressed: u(16),
                        size: u(24),
                        attributes: u(32),
                    },
                )
            })
            .collect();
        Ok(GamePak {
            file: Mutex::new(file),
            entries,
            chunks,
        })
    }

    /// How many files the table lists.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The bytes of the file at `game_path` (from `natives/`), or `None` when the pak has none.
    pub fn read(&self, game_path: &str) -> Result<Option<Vec<u8>>, GamePakError> {
        let Some(entry) = self.entries.get(&path_hash(game_path).combined()).copied() else {
            return Ok(None);
        };
        if entry.size > MAX_ENTRY || entry.compressed > MAX_ENTRY {
            return Err(GamePakError::TooLarge);
        }
        let compression = entry.attributes & 0xF;
        let encryption = (entry.attributes >> 16) & 0xFF;
        let mut file = self
            .file
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if compression == 0 && !self.chunks.is_empty() && CHUNKED.contains(&entry.attributes) {
            let mut out = Vec::with_capacity(entry.size as usize);
            let mut index =
                usize::try_from(entry.offset).map_err(|_| GamePakError::Format("a chunk index"))?;
            let mut remaining = entry.compressed;
            while remaining > 0 {
                let &(offset, size) = self
                    .chunks
                    .get(index)
                    .ok_or(GamePakError::Format("a chunk past the table"))?;
                if size == 0 || size > CHUNK_SIZE {
                    return Err(GamePakError::Format("a chunk of an impossible size"));
                }
                let mut raw = vec![0u8; size as usize];
                file.seek(SeekFrom::Start(offset))?;
                file.read_exact(&mut raw)?;
                if size == CHUNK_SIZE {
                    out.extend_from_slice(&raw);
                } else {
                    out.extend(
                        zstd::stream::decode_all(&raw[..]).map_err(|_| GamePakError::Decompress)?,
                    );
                }
                if out.len() as u64 > MAX_ENTRY {
                    return Err(GamePakError::TooLarge);
                }
                remaining = remaining.saturating_sub(size);
                index += 1;
            }
            out.truncate(entry.size as usize);
            return Ok(Some(out));
        }
        if (1..5).contains(&encryption) {
            return Err(GamePakError::EncryptedResource);
        }
        let mut raw = vec![0u8; entry.compressed as usize];
        file.seek(SeekFrom::Start(entry.offset))?;
        file.read_exact(&mut raw)?;
        drop(file);
        let bytes = match compression {
            0 => raw,
            1 => {
                let mut out = Vec::with_capacity(entry.size as usize);
                flate2::read::DeflateDecoder::new(&raw[..])
                    .take(MAX_ENTRY)
                    .read_to_end(&mut out)
                    .map_err(|_| GamePakError::Decompress)?;
                out
            }
            2 => zstd::stream::decode_all(&raw[..]).map_err(|_| GamePakError::Decompress)?,
            _ => return Err(GamePakError::Format("an unknown compression")),
        };
        if bytes.len() as u64 != entry.size {
            return Err(GamePakError::Decompress);
        }
        Ok(Some(bytes))
    }
}

/// Builds an invented game pak — tests only, here and in the crates that read through it.
#[cfg(any(test, feature = "fixtures"))]
pub mod fixture {
    use super::{xor_table, CHUNK_SIZE, ENTRY_LEN, KEY_LEN};
    use crate::{path_hash, MAGIC};
    use std::io::Write;

    /// How a fixture entry is stored.
    #[derive(Debug, Clone, Copy)]
    pub enum Stored {
        Raw,
        Deflate,
        Zstd,
        /// In the chunk table (zstd chunks), as the game stores big files.
        Chunked,
    }

    /// An invented `0x68` pak holding `files`, its table encrypted under a key chosen from
    /// `seed` (any 128 bytes do: the table key is whatever the public operation makes of them).
    pub fn pak(files: &[(&str, &[u8], Stored)], seed: u8) -> Vec<u8> {
        let stored_key: [u8; KEY_LEN] =
            std::array::from_fn(|i| (i as u8).wrapping_mul(37).wrapping_add(seed) | 1);
        let key = super::table_key(&stored_key);
        let flags: u16 = 0x0068;
        let mut table = vec![0u8; files.len() * ENTRY_LEN];
        let mut data: Vec<u8> = Vec::new();
        let mut chunks: Vec<(u64, u64)> = Vec::new();
        let mut entries = Vec::new();
        for (path, bytes, how) in files {
            let (offset, compressed, attributes) = match how {
                Stored::Raw => {
                    let at = data.len();
                    data.extend_from_slice(bytes);
                    (at as u64, bytes.len() as u64, 0u64)
                }
                Stored::Deflate => {
                    let mut z = flate2::write::DeflateEncoder::new(
                        Vec::new(),
                        flate2::Compression::default(),
                    );
                    z.write_all(bytes).expect("deflate");
                    let z = z.finish().expect("deflate");
                    let at = data.len();
                    data.extend_from_slice(&z);
                    (at as u64, z.len() as u64, 1)
                }
                Stored::Zstd => {
                    let z = zstd::stream::encode_all(*bytes, 3).expect("zstd");
                    let at = data.len();
                    data.extend_from_slice(&z);
                    (at as u64, z.len() as u64, 0x1002)
                }
                Stored::Chunked => {
                    let first = chunks.len() as u64;
                    let mut total = 0u64;
                    for piece in bytes.chunks(CHUNK_SIZE as usize) {
                        let z = zstd::stream::encode_all(piece, 3).expect("zstd");
                        chunks.push((data.len() as u64, z.len() as u64));
                        total += z.len() as u64;
                        data.extend_from_slice(&z);
                    }
                    (first, total, 0x100_0000)
                }
            };
            entries.push((
                path_hash(path).combined(),
                offset,
                compressed,
                bytes.len() as u64,
                attributes,
                *how,
            ));
        }
        // Lay out: header, table, 16-byte chunk table (empty), key, 8-byte chunk table, data.
        let wide_len = 8;
        let narrow_len = 8 + chunks.len() * 8;
        let data_start = (16 + table.len() + wide_len + KEY_LEN + narrow_len) as u64;
        for (i, (hash, offset, compressed, size, attributes, how)) in entries.iter().enumerate() {
            let at = i * ENTRY_LEN;
            let absolute = if matches!(how, Stored::Chunked) {
                *offset
            } else {
                offset + data_start
            };
            table[at..at + 8].copy_from_slice(&hash.to_le_bytes());
            table[at + 8..at + 16].copy_from_slice(&absolute.to_le_bytes());
            table[at + 16..at + 24].copy_from_slice(&compressed.to_le_bytes());
            table[at + 24..at + 32].copy_from_slice(&size.to_le_bytes());
            table[at + 32..at + 40].copy_from_slice(&attributes.to_le_bytes());
        }
        xor_table(&mut table, &key);
        let mut out = Vec::new();
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&[4, 2]);
        out.extend_from_slice(&flags.to_le_bytes());
        out.extend_from_slice(&(files.len() as u32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&table);
        out.extend_from_slice(&0u32.to_le_bytes()); // 16-byte table: count 0
        out.extend_from_slice(&(CHUNK_SIZE as u32).to_le_bytes());
        out.extend_from_slice(&stored_key);
        out.extend_from_slice(&(CHUNK_SIZE as u32).to_le_bytes()); // 8-byte table: block size
        out.extend_from_slice(&(chunks.len() as u32).to_le_bytes());
        for (offset, size) in &chunks {
            let absolute = offset + data_start;
            out.extend_from_slice(&(absolute as u32).to_le_bytes());
            let packed = ((size << 10) | ((absolute >> 32) & 0x3FF)) as u32;
            out.extend_from_slice(&packed.to_le_bytes());
        }
        assert_eq!(out.len() as u64, data_start);
        out.extend_from_slice(&data);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::fixture::{pak, Stored};
    use super::*;

    fn write_temp(bytes: &[u8]) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().expect("temp file");
        std::io::Write::write_all(&mut f, bytes).expect("write");
        f
    }

    #[test]
    fn an_invented_game_pak_reads_back_in_every_storage() {
        let big: Vec<u8> = (0..(CHUNK_SIZE as usize * 2 + 4321))
            .map(|i| (i % 251) as u8)
            .collect();
        let files: [(&str, &[u8], Stored); 4] = [
            (
                "natives/stm/message/ui/itemname.msg.24",
                b"zstd text",
                Stored::Zstd,
            ),
            (
                "natives/stm/message/ui/job.msg.24",
                b"deflate text",
                Stored::Deflate,
            ),
            (
                "natives/stm/message/ui/personality.msg.24",
                b"raw text",
                Stored::Raw,
            ),
            (
                "natives/stm/streaming/big.tex.251211553",
                &big,
                Stored::Chunked,
            ),
        ];
        let file = write_temp(&pak(&files, 7));
        let opened = GamePak::open(file.path()).unwrap();
        assert_eq!(opened.len(), 4);
        for (path, bytes, _) in files {
            assert_eq!(opened.read(path).unwrap().as_deref(), Some(bytes), "{path}");
        }
        assert_eq!(
            opened
                .read("natives/stm/message/ui/not_there.msg.24")
                .unwrap(),
            None
        );
        // Case and slashes do not matter: the engine's path hash folds them.
        assert!(opened
            .read(r"NATIVES\STM\Message\UI\Job.msg.24")
            .unwrap()
            .is_some());
    }

    #[test]
    fn the_table_key_depends_on_the_stored_bytes() {
        let a = pak(&[("natives/a.msg.24", b"a", Stored::Raw)], 1);
        let b = pak(&[("natives/a.msg.24", b"a", Stored::Raw)], 2);
        assert_ne!(a, b);
        for bytes in [a, b] {
            let file = write_temp(&bytes);
            assert_eq!(
                GamePak::open(file.path())
                    .unwrap()
                    .read("natives/a.msg.24")
                    .unwrap()
                    .as_deref(),
                Some(&b"a"[..])
            );
        }
    }

    #[test]
    fn a_mod_pak_or_garbage_is_refused_cleanly() {
        let file = write_temp(b"PK\x03\x04 not a pak");
        assert!(matches!(
            GamePak::open(file.path()),
            Err(GamePakError::Format(_) | GamePakError::Io(_))
        ));
        let mut bytes = pak(&[("natives/a.msg.24", b"abc", Stored::Raw)], 3);
        bytes[4] = 2;
        let file = write_temp(&bytes);
        assert!(matches!(
            GamePak::open(file.path()),
            Err(GamePakError::Format(_))
        ));
    }
}
