//! The write side of extraction, shared by every source: destination files,
//! capped reads, and the progress tally.
//!
//! A destination is written fully, flushed, and only then reported done. A
//! file that fails half-way is removed again (best effort): a caller never
//! finds a half-written file at a destination the extractor did not report.
//! Files reported earlier stay where they are — the caller's journal knows
//! them. `extract` does not fsync; durability is the deploy engine's call.

use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Write};
use std::path::Path;

use rift_core::{ExtractProgress, RiftError};

use crate::fail;

/// Initial capacity cap for a read buffer. Sizes come from the archive and
/// may lie; the buffer still grows to the real size as bytes arrive.
const PREALLOCATE_MAX: u64 = 8 * 1024 * 1024;

/// Copy buffer for pull-model sources (zip, 7z, folders).
const COPY_CHUNK: usize = 256 * 1024;

/// Read at most `limit` bytes (everything when `None`) from `reader`.
/// Reading stops at the limit, so a decoder underneath stops too.
pub(crate) fn read_capped(
    reader: &mut dyn Read,
    limit: Option<u64>,
    size_hint: u64,
) -> io::Result<Vec<u8>> {
    let expected = limit.map_or(size_hint, |limit| limit.min(size_hint));
    let mut bytes = Vec::with_capacity(usize::try_from(expected.min(PREALLOCATE_MAX)).unwrap_or(0));
    match limit {
        Some(limit) => reader.take(limit).read_to_end(&mut bytes)?,
        None => reader.read_to_end(&mut bytes)?,
    };
    Ok(bytes)
}

/// The first `limit` bytes of a cached entry.
pub(crate) fn prefix(bytes: &[u8], limit: Option<u64>) -> Vec<u8> {
    let end = limit.map_or(bytes.len(), |limit| {
        usize::try_from(limit).map_or(bytes.len(), |limit| limit.min(bytes.len()))
    });
    bytes[..end].to_vec()
}

/// One destination file being written.
pub(crate) struct DestWriter<'a> {
    dest: &'a Path,
    entry: &'a str,
    out: Option<BufWriter<File>>,
    written: u64,
    failure: Option<io::Error>,
}

impl<'a> DestWriter<'a> {
    /// Create (or truncate) `dest`, creating its parent folders first.
    pub(crate) fn create(dest: &'a Path, entry: &'a str) -> Result<Self, RiftError> {
        if let Some(parent) = dest.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|err| fail::write(dest, entry, &err))?;
            }
        }
        let file = File::create(dest).map_err(|err| fail::write(dest, entry, &err))?;
        Ok(DestWriter {
            dest,
            entry,
            out: Some(BufWriter::with_capacity(COPY_CHUNK, file)),
            written: 0,
            failure: None,
        })
    }

    /// Append a chunk. Returns `false` once writing has failed; the failure
    /// is kept for [`DestWriter::finish`]. (The RAR data callback needs a
    /// yes/no answer, not a `Result`.)
    pub(crate) fn push(&mut self, chunk: &[u8]) -> bool {
        if self.failure.is_some() {
            return false;
        }
        let Some(out) = self.out.as_mut() else {
            return false;
        };
        match out.write_all(chunk) {
            Ok(()) => {
                self.written += chunk.len() as u64;
                true
            }
            Err(err) => {
                self.failure = Some(err);
                false
            }
        }
    }

    /// Flush and close. On any failure the partial file is removed and the
    /// write error returned; otherwise the byte count.
    pub(crate) fn finish(mut self) -> Result<u64, RiftError> {
        if let Some(err) = self.failure.take() {
            let error = fail::write(self.dest, self.entry, &err);
            self.abandon();
            return Err(error);
        }
        let flushed = self.out.take().map_or(Ok(()), |mut out| out.flush());
        match flushed {
            Ok(()) => Ok(self.written),
            Err(err) => {
                let error = fail::write(self.dest, self.entry, &err);
                self.abandon();
                Err(error)
            }
        }
    }

    /// Give up on this file: close it and remove what was written.
    pub(crate) fn abandon(mut self) {
        drop(self.out.take());
        // Best effort: the file may never have been created, or may already
        // be gone; neither changes the error the caller reports.
        let _ = fs::remove_file(self.dest);
    }
}

/// Pull `reader` dry into `dest`. Read errors go through `on_read_error`
/// (they belong to the package); write errors name the destination.
pub(crate) fn write_from_reader(
    dest: &Path,
    entry: &str,
    reader: &mut dyn Read,
    on_read_error: &dyn Fn(&io::Error) -> RiftError,
) -> Result<u64, RiftError> {
    let mut out = DestWriter::create(dest, entry)?;
    let mut chunk = vec![0u8; COPY_CHUNK];
    loop {
        let read = match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => read,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => {
                out.abandon();
                return Err(on_read_error(&err));
            }
        };
        if !out.push(&chunk[..read]) {
            break;
        }
    }
    out.finish()
}

/// Copy an already extracted file to a further destination (the same entry
/// requested twice in one batch).
pub(crate) fn duplicate(written: &Path, dest: &Path, entry: &str) -> Result<u64, RiftError> {
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|err| fail::write(dest, entry, &err))?;
        }
    }
    fs::copy(written, dest).map_err(|err| {
        let _ = fs::remove_file(dest);
        fail::write(dest, entry, &err)
    })
}

/// Counts finished files and reports after each one.
pub(crate) struct Tally<'p> {
    report: &'p mut dyn FnMut(ExtractProgress),
    state: ExtractProgress,
}

impl<'p> Tally<'p> {
    pub(crate) fn new(
        report: &'p mut dyn FnMut(ExtractProgress),
        files_total: usize,
        bytes_total: u64,
    ) -> Self {
        Tally {
            report,
            state: ExtractProgress {
                files_done: 0,
                files_total,
                bytes_done: 0,
                bytes_total,
            },
        }
    }

    pub(crate) fn file_done(&mut self, bytes: u64) {
        self.state.files_done += 1;
        self.state.bytes_done += bytes;
        (self.report)(self.state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capped_reads_stop_at_the_limit() {
        let data = vec![7u8; 10_000];
        assert_eq!(
            read_capped(&mut &data[..], Some(100), 10_000)
                .unwrap()
                .len(),
            100
        );
        assert_eq!(
            read_capped(&mut &data[..], None, 10_000).unwrap().len(),
            10_000
        );
        assert_eq!(
            read_capped(&mut &data[..], Some(50_000), 10_000)
                .unwrap()
                .len(),
            10_000
        );
        assert!(read_capped(&mut &data[..], Some(0), 10_000)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn a_cached_prefix_honours_the_limit() {
        assert_eq!(prefix(b"modinfo", Some(3)), b"mod");
        assert_eq!(prefix(b"modinfo", None), b"modinfo");
        assert_eq!(prefix(b"modinfo", Some(99)), b"modinfo");
    }

    #[test]
    fn a_failed_read_leaves_no_half_written_file() {
        struct Breaks;
        impl Read for Breaks {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::InvalidData, "bad block"))
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("deep/er/out.bin");
        let err = write_from_reader(&dest, "a.bin", &mut Breaks, &|err| {
            fail::entry_read(Path::new("Lantern Glow Recolor.zip"), "a.bin", err)
        })
        .unwrap_err();
        assert!(matches!(err, RiftError::CorruptArchive { .. }));
        assert!(!dest.exists());
    }

    #[test]
    fn a_failed_write_names_the_destination() {
        let dir = tempfile::tempdir().unwrap();
        // A file where a parent folder should be: creating the dest fails.
        std::fs::write(dir.path().join("blocker"), b"x").unwrap();
        let dest = dir.path().join("blocker/out.bin");
        let err =
            write_from_reader(&dest, "a.bin", &mut &b"data"[..], &|_| unreachable!()).unwrap_err();
        assert!(matches!(&err, RiftError::Io { detail } if detail.contains("out.bin")));
    }
}
