//! Fixture builders. Every package a test opens is built here, at run time,
//! in a temporary directory: zips through the `zip` writer, 7z through
//! `sevenz-rust2`'s writer, and RAR5 forged byte by byte (no crate writes
//! RAR, and no `rar` binary exists on the bench). Names are invented.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use rift_core::{ExtractItem, ExtractProgress, PackageSource};
use sevenz_rust2::{
    ArchiveEntry, ArchiveWriter, EncoderConfiguration, EncoderMethod, SourceReader,
};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

// ---------------------------------------------------------------------------
// Payloads
// ---------------------------------------------------------------------------

/// Deterministic bytes that compress to roughly half: a small alphabet
/// chosen by a linear congruential generator. Large enough fixtures to make
/// "the limit stopped the decoder" observable.
pub fn compressible(len: usize, seed: u32) -> Vec<u8> {
    let mut state = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
    (0..len)
        .map(|_| {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            b"natives/stm/meshcape"[(state >> 16) as usize % 20]
        })
        .collect()
}

/// A modinfo.ini body for an invented option.
pub fn modinfo(name: &str) -> Vec<u8> {
    format!("name={name}\r\nauthor=Pawn Guild\r\nversion=1.0\r\n").into_bytes()
}

// ---------------------------------------------------------------------------
// Zip
// ---------------------------------------------------------------------------

pub enum Z<'a> {
    Stored(&'a str, &'a [u8]),
    Deflated(&'a str, &'a [u8]),
    Dir(&'a str),
}

pub fn zip_at(path: &Path, entries: &[Z<'_>]) {
    let mut writer = ZipWriter::new(File::create(path).unwrap());
    for entry in entries {
        match entry {
            Z::Stored(name, bytes) | Z::Deflated(name, bytes) => {
                let method = if matches!(entry, Z::Stored(..)) {
                    CompressionMethod::Stored
                } else {
                    CompressionMethod::Deflated
                };
                writer
                    .start_file(
                        *name,
                        SimpleFileOptions::default().compression_method(method),
                    )
                    .unwrap();
                writer.write_all(bytes).unwrap();
            }
            Z::Dir(name) => writer
                .add_directory(*name, SimpleFileOptions::default())
                .unwrap(),
        }
    }
    writer.finish().unwrap();
}

/// Replace every occurrence of `from` with `to` (same length) in the file.
/// Zip checksums cover data, not names, so a renamed entry stays valid.
pub fn patch_bytes(path: &Path, from: &[u8], to: &[u8]) {
    assert_eq!(from.len(), to.len());
    let mut bytes = fs::read(path).unwrap();
    let mut hits = 0;
    let mut i = 0;
    while i + from.len() <= bytes.len() {
        if &bytes[i..i + from.len()] == from {
            bytes[i..i + from.len()].copy_from_slice(to);
            hits += 1;
            i += from.len();
        } else {
            i += 1;
        }
    }
    assert!(
        hits >= 2,
        "{from:?} should appear in the local and central headers"
    );
    fs::write(path, bytes).unwrap();
}

/// Set the "encrypted" bit (general-purpose flag bit 0) of the entry named
/// `name` in both its local and its central header.
pub fn mark_zip_entry_encrypted(path: &Path, name: &str) {
    let mut bytes = fs::read(path).unwrap();
    let mut marked = 0;
    let mut i = 0;
    while i + 4 <= bytes.len() {
        let signature = &bytes[i..i + 4];
        let (flags_at, name_len_at, name_at) = if signature == b"PK\x03\x04" {
            (i + 6, i + 26, i + 30)
        } else if signature == b"PK\x01\x02" {
            (i + 8, i + 28, i + 46)
        } else {
            i += 1;
            continue;
        };
        let name_len = u16::from_le_bytes([bytes[name_len_at], bytes[name_len_at + 1]]) as usize;
        if bytes.get(name_at..name_at + name_len) == Some(name.as_bytes()) {
            bytes[flags_at] |= 1;
            marked += 1;
        }
        i += 4;
    }
    assert_eq!(marked, 2, "{name} should have a local and a central header");
    fs::write(path, bytes).unwrap();
}

/// Overwrite `len` bytes ending `back` bytes before the central directory:
/// the tail of the last entry's compressed data.
pub fn corrupt_before_central_directory(path: &Path, back: usize, len: usize) {
    let mut bytes = fs::read(path).unwrap();
    let directory = bytes
        .windows(4)
        .position(|window| window == b"PK\x01\x02")
        .unwrap();
    for byte in &mut bytes[directory - back..directory - back + len] {
        *byte ^= 0x5a;
    }
    fs::write(path, bytes).unwrap();
}

// ---------------------------------------------------------------------------
// 7z
// ---------------------------------------------------------------------------

pub enum S<'a> {
    /// One solid block of files.
    Solid(Vec<(&'a str, &'a [u8])>),
    /// One file in its own block.
    Single(&'a str, &'a [u8]),
    Dir(&'a str),
    /// An empty file (no stream).
    Empty(&'a str),
}

pub fn sevenz_at(path: &Path, parts: Vec<S<'_>>) {
    let mut writer = ArchiveWriter::create(path).unwrap();
    writer.set_content_methods(vec![EncoderConfiguration::new(EncoderMethod::LZMA2)]);
    for part in parts {
        match part {
            S::Solid(files) => {
                let entries = files
                    .iter()
                    .map(|(name, _)| ArchiveEntry::new_file(name))
                    .collect();
                let readers = files
                    .iter()
                    .map(|(_, bytes)| SourceReader::new(*bytes))
                    .collect();
                writer.push_archive_entries(entries, readers).unwrap();
            }
            S::Single(name, bytes) => {
                writer
                    .push_archive_entry(ArchiveEntry::new_file(name), Some(bytes))
                    .unwrap();
            }
            S::Dir(name) => {
                writer
                    .push_archive_entry(ArchiveEntry::new_directory(name), None::<&[u8]>)
                    .unwrap();
            }
            S::Empty(name) => {
                writer
                    .push_archive_entry(ArchiveEntry::new_file(name), None::<&[u8]>)
                    .unwrap();
            }
        }
    }
    writer.finish().unwrap();
}

/// Overwrite the first byte of the first packed stream. Under LZMA2 a `0x00`
/// there is the end-of-stream marker: the decoder stops before producing a
/// single byte, although the archive promises the block's full size.
pub fn end_first_pack_stream_at_once(path: &Path) {
    let mut file = File::open(path).unwrap();
    let archive = sevenz_rust2::Archive::read(&mut file, &sevenz_rust2::Password::empty()).unwrap();
    let start = usize::try_from(sevenz_rust2::SIGNATURE_HEADER_SIZE + archive.pack_pos()).unwrap();
    let mut bytes = fs::read(path).unwrap();
    bytes[start] = 0x00;
    fs::write(path, bytes).unwrap();
}

/// Flip `len` bytes ending `back` bytes before the end of the first packed
/// stream (the first block's compressed data).
pub fn corrupt_first_pack_tail(path: &Path, back: u64, len: usize) {
    let mut file = File::open(path).unwrap();
    let archive = sevenz_rust2::Archive::read(&mut file, &sevenz_rust2::Password::empty()).unwrap();
    let end = sevenz_rust2::SIGNATURE_HEADER_SIZE + archive.pack_pos() + archive.pack_sizes()[0];
    let start = usize::try_from(end - back).unwrap();
    let mut bytes = fs::read(path).unwrap();
    for byte in &mut bytes[start..start + len] {
        *byte ^= 0x5a;
    }
    fs::write(path, bytes).unwrap();
}

// ---------------------------------------------------------------------------
// RAR5, forged
// ---------------------------------------------------------------------------
//
// Layout per the vendored UnRAR reader (arcread.cpp `ReadHeader50`): every
// block is `CRC32 · vint size · type · flags [· extra size] [· data size] ·
// fields · extra` followed by its data; the CRC covers everything from the
// size field to the end of the extra area. Only stored files (method 0):
// compressing RAR is proprietary, and the plumbing under test is the same.

pub enum R<'a> {
    File(&'a str, &'a [u8]),
    /// A file whose header carries a wrong data CRC.
    BadCrc(&'a str, &'a [u8]),
    /// A file with an AES-256 encryption record (its "data" is noise).
    Encrypted(&'a str, u64),
    Dir(&'a str),
    /// A redirect (1 = Unix symlink, 4 = hard link, 5 = file copy) to `target`.
    Link(&'a str, u64, &'a str, u64),
}

pub fn vint(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

fn block(kind: u64, fields: &[u8], extra: &[u8], data: Option<&[u8]>) -> Vec<u8> {
    let mut flags = 0;
    if !extra.is_empty() {
        flags |= 0x0001;
    }
    if data.is_some() {
        flags |= 0x0002;
    }
    let mut body = Vec::new();
    vint(&mut body, kind);
    vint(&mut body, flags);
    if !extra.is_empty() {
        vint(&mut body, extra.len() as u64);
    }
    if let Some(data) = data {
        vint(&mut body, data.len() as u64);
    }
    body.extend_from_slice(fields);
    body.extend_from_slice(extra);
    let mut size = Vec::new();
    vint(&mut size, body.len() as u64);
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(&size);
    hasher.update(&body);
    let mut out = hasher.finalize().to_le_bytes().to_vec();
    out.extend_from_slice(&size);
    out.extend_from_slice(&body);
    if let Some(data) = data {
        out.extend_from_slice(data);
    }
    out
}

/// File-header fields: flags, sizes, attributes, CRC, method, host, name.
fn file_fields(name: &str, size: u64, directory: bool, crc: Option<u32>) -> Vec<u8> {
    let mut fields = Vec::new();
    let mut file_flags = 0;
    if directory {
        file_flags |= 0x0001;
    }
    if crc.is_some() {
        file_flags |= 0x0004;
    }
    vint(&mut fields, file_flags);
    vint(&mut fields, size);
    vint(&mut fields, if directory { 0x10 } else { 0x20 }); // Windows attributes
    if let Some(crc) = crc {
        fields.extend_from_slice(&crc.to_le_bytes());
    }
    vint(&mut fields, 0); // compression info: version 0, method 0 (stored)
    vint(&mut fields, 0); // host OS: Windows
    vint(&mut fields, name.len() as u64);
    fields.extend_from_slice(name.as_bytes());
    fields
}

pub fn rar5_bytes(solid: bool, items: &[R<'_>], with_end: bool) -> Vec<u8> {
    let mut out = b"Rar!\x1A\x07\x01\x00".to_vec();
    let mut main = Vec::new();
    vint(&mut main, if solid { 0x0004 } else { 0 });
    out.extend(block(1, &main, &[], None));
    for item in items {
        match item {
            R::File(name, data) => {
                let fields =
                    file_fields(name, data.len() as u64, false, Some(crc32fast::hash(data)));
                out.extend(block(2, &fields, &[], Some(data)));
            }
            R::BadCrc(name, data) => {
                let wrong = crc32fast::hash(data) ^ 0xdead_beef;
                let fields = file_fields(name, data.len() as u64, false, Some(wrong));
                out.extend(block(2, &fields, &[], Some(data)));
            }
            R::Encrypted(name, size) => {
                let fields = file_fields(name, *size, false, None);
                let mut record = Vec::new();
                vint(&mut record, 0x01); // FHEXTRA_CRYPT
                vint(&mut record, 0); // AES-256
                vint(&mut record, 0); // no password check value
                record.push(15); // KDF iterations, log2
                record.extend_from_slice(&[0x11; 16]); // salt
                record.extend_from_slice(&[0x22; 16]); // IV
                let mut extra = Vec::new();
                vint(&mut extra, record.len() as u64);
                extra.extend(record);
                let noise = vec![0x33u8; usize::try_from(size.div_ceil(16) * 16).unwrap()];
                out.extend(block(2, &fields, &extra, Some(&noise)));
            }
            R::Dir(name) => {
                let fields = file_fields(name, 0, true, None);
                out.extend(block(2, &fields, &[], None));
            }
            R::Link(name, kind, target, size) => {
                let fields = file_fields(name, *size, false, None);
                let mut record = Vec::new();
                vint(&mut record, 0x05); // FHEXTRA_REDIR
                vint(&mut record, *kind);
                vint(&mut record, 0);
                vint(&mut record, target.len() as u64);
                record.extend_from_slice(target.as_bytes());
                let mut extra = Vec::new();
                vint(&mut extra, record.len() as u64);
                extra.extend(record);
                out.extend(block(2, &fields, &extra, None));
            }
        }
    }
    if with_end {
        let mut end = Vec::new();
        vint(&mut end, 0);
        out.extend(block(5, &end, &[], None));
    }
    out
}

pub fn rar5_at(path: &Path, solid: bool, items: &[R<'_>]) {
    fs::write(path, rar5_bytes(solid, items, true)).unwrap();
}

// ---------------------------------------------------------------------------
// Driving a source
// ---------------------------------------------------------------------------

pub fn paths(source: &mut dyn PackageSource) -> Vec<String> {
    source
        .entries()
        .unwrap()
        .into_iter()
        .map(|entry| entry.path)
        .collect()
}

pub struct Extracted {
    pub progress: Vec<ExtractProgress>,
}

/// `<dest>/staging/<n>.out` for each entry (the deploy engine's temp-name shape).
fn staged(dest: &Path, entries: &[&str]) -> Vec<ExtractItem> {
    entries
        .iter()
        .enumerate()
        .map(|(n, entry)| ExtractItem {
            entry: (*entry).to_string(),
            dest: dest.join(format!("staging/{n}.out")),
        })
        .collect()
}

/// Extract `entries` to `<dest>/<n>.out` (the deploy engine's temp-name
/// shape), returning the progress reports and the destinations.
pub fn extract_to(
    source: &mut dyn PackageSource,
    dest: &Path,
    entries: &[&str],
) -> (Result<(), rift_core::RiftError>, Extracted, Vec<PathBuf>) {
    let items = staged(dest, entries);
    let mut progress = Vec::new();
    let result = source.extract(&items, &mut |report| progress.push(report));
    let dests = items.into_iter().map(|item| item.dest).collect();
    (result, Extracted { progress }, dests)
}

/// What a rival writer leaves at a destination in [`extract_racing`].
pub const PLANTED: &[u8] = b"not the Riftstone's";

/// [`extract_to`] with a rival: once `after` files are done (`0`: before the
/// first byte), something else creates the destination of item `taken`. That
/// is the moment the deploy engine's own check cannot cover: the name was
/// free when it looked, and taken when the extractor got there.
pub fn extract_racing(
    source: &mut dyn PackageSource,
    dest: &Path,
    entries: &[&str],
    after: usize,
    taken: usize,
) -> (Result<(), rift_core::RiftError>, Extracted, Vec<PathBuf>) {
    let items = staged(dest, entries);
    let target = items[taken].dest.clone();
    let plant = || {
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, PLANTED).unwrap();
    };
    if after == 0 {
        plant();
    }
    let mut progress = Vec::new();
    let result = source.extract(&items, &mut |report| {
        progress.push(report);
        if report.files_done == after {
            plant();
        }
    });
    let dests = items.into_iter().map(|item| item.dest).collect();
    (result, Extracted { progress }, dests)
}

/// The batch stopped at the taken destination: a refusal that names it, and
/// the rival's file exactly as the rival left it.
pub fn assert_refused_at(result: &Result<(), rift_core::RiftError>, taken: &Path) {
    match result {
        Err(rift_core::RiftError::Refused { reason }) => assert!(
            reason.contains(&taken.display().to_string()),
            "the refusal should name {}: {reason}",
            taken.display()
        ),
        other => panic!(
            "expected a refusal naming {}, got {other:?}",
            taken.display()
        ),
    }
    assert_eq!(
        fs::read(taken).unwrap(),
        PLANTED,
        "the rival's file is untouched"
    );
}

/// A RAR5 archive whose headers are encrypted (`rar -hp`): an archive
/// encryption header straight after the signature, then ciphertext.
pub fn rar5_encrypted_headers_bytes() -> Vec<u8> {
    let mut out = b"Rar!\x1A\x07\x01\x00".to_vec();
    let mut fields = Vec::new();
    vint(&mut fields, 0); // AES-256
    vint(&mut fields, 0); // no password check value
    fields.push(15); // KDF iterations, log2
    fields.extend_from_slice(&[0x44; 16]); // salt
    out.extend(block(4, &fields, &[], None));
    out.extend_from_slice(&[0x55; 64]); // an IV and encrypted headers, as far as anyone can tell
    out
}

// ---------------------------------------------------------------------------
// RAR4, forged
// ---------------------------------------------------------------------------
//
// Layout per arcread.cpp `ReadHeader15`: `CRC16 · type · flags · size`,
// where the CRC is the low half of the CRC32 of the header after its own
// field. A file header is 32 bytes plus the name (stored, Windows host,
// backslash separators, as WinRAR 3.x writes them).

fn rar4_header(kind: u8, flags: u16, rest: &[u8]) -> Vec<u8> {
    let size = u16::try_from(7 + rest.len()).unwrap();
    let mut body = vec![kind];
    body.extend_from_slice(&flags.to_le_bytes());
    body.extend_from_slice(&size.to_le_bytes());
    body.extend_from_slice(rest);
    let crc = (crc32fast::hash(&body) & 0xffff) as u16;
    let mut out = crc.to_le_bytes().to_vec();
    out.extend(body);
    out
}

pub fn rar4_bytes(files: &[(&str, &[u8])], with_end: bool) -> Vec<u8> {
    let mut out = b"Rar!\x1A\x07\x00".to_vec();
    out.extend(rar4_header(0x73, 0, &[0; 6])); // main header: two reserved fields
    for (name, data) in files {
        let mut rest = Vec::new();
        rest.extend_from_slice(&u32::try_from(data.len()).unwrap().to_le_bytes()); // packed
        rest.extend_from_slice(&u32::try_from(data.len()).unwrap().to_le_bytes()); // unpacked
        rest.push(2); // host OS: Win32
        rest.extend_from_slice(&crc32fast::hash(data).to_le_bytes());
        rest.extend_from_slice(&0x0021_0000u32.to_le_bytes()); // DOS time: 1980-01-01
        rest.push(29); // unpack version 2.9
        rest.push(0x30); // method: store
        rest.extend_from_slice(&u16::try_from(name.len()).unwrap().to_le_bytes());
        rest.extend_from_slice(&0x20u32.to_le_bytes()); // archive attribute
        rest.extend_from_slice(name.as_bytes());
        out.extend(rar4_header(0x74, 0x8000, &rest)); // LONG_BLOCK: data follows
        out.extend_from_slice(data);
    }
    if with_end {
        out.extend(rar4_header(0x7b, 0x4000, &[]));
    }
    out
}
