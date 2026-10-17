//! rift-archive — opens a package and hands back a [`PackageSource`].
//!
//! Four formats behind one contract (`rift_core::PackageSource`):
//!
//! | format | engine | reading one entry | `extract` |
//! |--------|--------|-------------------|-----------|
//! | zip    | `zip` (pure Rust) | random access per entry | item order |
//! | 7z     | `sevenz-rust2` (pure Rust) | random access per solid block | one pass, archive order |
//! | rar    | the official UnRAR sources (`unrar_sys`) | sequential from the start | one pass, archive order |
//! | folder | `walkdir`, no symlinks, depth ≤ 64 | the file on disk | item order, always a copy (RS-2) |
//!
//! Rules every source keeps:
//!
//! - **The zip-slip guard.** Every entry path goes through
//!   [`rift_core::normalize_entry_path`] once, when the package is opened. A
//!   path that climbs out (`..`, a drive letter, a `:` stream, a control
//!   character) is dropped with a `log::warn!` and can never be read or
//!   extracted — it is simply not in the package any more.
//! - **Case-blind lookups**, the way the Windows filesystem sees
//!   `natives/STM` and `natives/stm` as one place. When two entries share a
//!   case-folded path the later one wins — the copy an extractor would leave
//!   on disk — and the earlier one is dropped with a warning.
//! - **`limit` stops decompression**, not just copying: zip inflates no
//!   further than the limit, 7z stops its block decoder, and UnRAR is told to
//!   cancel through its data callback. A pak's table of contents costs its
//!   first `16 + 48 × n` bytes, not the 2 GB behind them.
//! - **Solid archives are passed once.** `extract` walks a 7z block or a RAR
//!   archive in its own order and writes the requested entries as they go by.
//!   `read_entry` on a small entry also keeps every same-named small sibling
//!   it passes (every `modinfo.ini` of a 281-option collection), so the
//!   catalog's one-modinfo-at-a-time scan costs one pass, not 281. That is a
//!   favour, never a condition: a sibling that fails costs the entry asked
//!   for nothing, and a read fails only when the entry itself cannot be
//!   reached, naming the place the pass broke (see the `cache` module).
//! - **Links are never files.** Symlinks and junctions are dropped from the
//!   listing; a RAR hard link or file copy is served from the entry it points
//!   at.
//! - **Destinations are created, never overwritten.** `extract` opens every
//!   destination with `create_new` (folder copies, and the second copy of an
//!   entry asked for twice, too). A file, folder or link (even a dangling one)
//!   already at a destination stops the batch with [`RiftError::Refused`]
//!   naming it, and nothing is written over it or through it. RAR is no
//!   exception: UnRAR runs in test mode and hands its bytes to the same
//!   writer, so nothing of UnRAR's own ever opens a destination.
//! - **Damage is not emptiness.** zip and 7z keep their directories at the
//!   end of the file, so a cut-off download fails to open. RAR streams, and
//!   UnRAR reports a cut-off archive as merely ending; so a RAR's block chain
//!   is walked to its end-of-archive block before it is listed. A 7z file
//!   whose stream ends before its promised size is cut off, never a short
//!   file that looks whole.
//! - **Encryption** fails the whole package only when every entry with data
//!   is encrypted. A partly encrypted package lists normally, and reading an
//!   encrypted entry answers [`RiftError::Encrypted`] — as does reading an
//!   entry that lies behind one in a solid RAR, which cannot be unpacked
//!   without unpacking the locked entry first.
//! - **Every error names the package path** (or, for a write or a refused
//!   destination, the destination) so the window can say which download
//!   misbehaved.

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use rift_core::{PackageFormat, PackageSource, RiftError};

mod cache;
mod fail;
mod folder_source;
mod index;
mod rar_frame;
mod rar_source;
mod sevenz_source;
mod unrar_dll;
mod write;
mod zip_source;

pub use folder_source::FolderSource;
pub use rar_source::RarSource;
pub use sevenz_source::SevenZipSource;
pub use zip_source::ZipSource;

const ZIP_LOCAL_HEADER: &[u8] = b"PK\x03\x04";
const ZIP_EMPTY_ARCHIVE: &[u8] = b"PK\x05\x06";
const SEVEN_Z_SIGNATURE: &[u8] = b"7z\xBC\xAF\x27\x1C";
const RAR4_SIGNATURE: &[u8] = b"Rar!\x1A\x07\x00";
const RAR5_SIGNATURE: &[u8] = b"Rar!\x1A\x07\x01\x00";

/// Which package format sits at `path`, decided by magic bytes (never by the
/// extension alone): `PK\x03\x04` zip, `7z\xBC\xAF\x27\x1C` 7z,
/// `Rar!\x1A\x07` rar (v4 and v5), a directory → folder. `Ok(None)` means
/// "not a package" (a loose .txt, an .exe, a partial `.crdownload`).
pub fn sniff(path: &Path) -> Result<Option<PackageFormat>, RiftError> {
    let metadata = fs::metadata(path).map_err(|err| RiftError::unreadable(path, err))?;
    if metadata.is_dir() {
        return Ok(Some(PackageFormat::Folder));
    }
    if !metadata.is_file() {
        // A pipe, a socket or a device: reading it could block forever.
        return Ok(None);
    }
    let mut head = Vec::with_capacity(8);
    File::open(path)
        .and_then(|file| file.take(8).read_to_end(&mut head))
        .map_err(|err| RiftError::unreadable(path, err))?;
    Ok(format_from_magic(&head))
}

/// The format a file's first bytes announce.
fn format_from_magic(head: &[u8]) -> Option<PackageFormat> {
    if head.starts_with(ZIP_LOCAL_HEADER) || head.starts_with(ZIP_EMPTY_ARCHIVE) {
        Some(PackageFormat::Zip)
    } else if head.starts_with(SEVEN_Z_SIGNATURE) {
        Some(PackageFormat::SevenZip)
    } else if head.starts_with(RAR4_SIGNATURE) || head.starts_with(RAR5_SIGNATURE) {
        Some(PackageFormat::Rar)
    } else {
        None
    }
}

/// Open the package at `path`.
pub fn open(path: &Path) -> Result<Box<dyn PackageSource>, RiftError> {
    match sniff(path)? {
        Some(PackageFormat::Zip) => Ok(Box::new(ZipSource::open(path)?)),
        Some(PackageFormat::SevenZip) => Ok(Box::new(SevenZipSource::open(path)?)),
        Some(PackageFormat::Rar) => Ok(Box::new(RarSource::open(path)?)),
        Some(PackageFormat::Folder) => Ok(Box::new(FolderSource::open(path)?)),
        None => Err(RiftError::UnsupportedFormat {
            path: path.display().to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magic_bytes_decide_the_format() {
        assert_eq!(
            format_from_magic(b"PK\x03\x04rest"),
            Some(PackageFormat::Zip)
        );
        assert_eq!(
            format_from_magic(b"PK\x05\x06\0\0"),
            Some(PackageFormat::Zip)
        );
        assert_eq!(
            format_from_magic(b"7z\xBC\xAF\x27\x1C\0\x04"),
            Some(PackageFormat::SevenZip)
        );
        assert_eq!(
            format_from_magic(b"Rar!\x1A\x07\x00\xCF"),
            Some(PackageFormat::Rar)
        );
        assert_eq!(
            format_from_magic(b"Rar!\x1A\x07\x01\x00"),
            Some(PackageFormat::Rar)
        );
    }

    #[test]
    fn near_misses_are_not_packages() {
        for head in [
            &b""[..],
            b"PK",
            b"PK\x01\x02",
            b"MZ\x90\x00",
            b"7z\xBC\xAF\x27",
            b"Rar!\x1A\x07\x02",
            b"Rar!",
        ] {
            assert_eq!(format_from_magic(head), None, "{head:?}");
        }
    }
}
