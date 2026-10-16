//! The write side of extraction, shared by every source: destination files,
//! capped reads, and the progress tally.
//!
//! **Created new, never overwritten.** Every destination is opened with
//! `create_new` (zip, 7z, RAR and folder copies alike, and the second copy of
//! an entry asked for twice): when anything already sits there, a file, a
//! folder or a link (even a dangling one), the batch stops with
//! [`RiftError::Refused`] naming it, and the thing that sits there is left
//! exactly as it was: never truncated, never written through, never removed.
//! The deploy engine checks that its temporary names are free before it
//! extracts; this closes the moment between that check and the write.
//!
//! A destination is written fully, flushed, and only then reported done. A
//! file that fails half-way is removed again (best effort): a caller never
//! finds a half-written file at a destination the extractor did not report.
//! Only a file this module created is ever removed. Files reported earlier
//! stay where they are — the caller's journal knows them. `extract` does not
//! fsync; durability is the deploy engine's call.

use std::fs::{self, File, OpenOptions};
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
    /// Create `dest` as a new file, creating its parent folders first. When
    /// anything already sits at `dest` this refuses ([`fail::taken`]) and
    /// touches nothing; no `DestWriter` exists then, so nothing can remove it.
    pub(crate) fn create(dest: &'a Path, entry: &'a str) -> Result<Self, RiftError> {
        if let Some(parent) = dest.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|err| fail::write(dest, entry, &err))?;
            }
        }
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dest)
            .map_err(|err| match err.kind() {
                io::ErrorKind::AlreadyExists => fail::taken(dest, entry),
                _ => fail::write(dest, entry, &err),
            })?;
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
/// requested twice in one batch). The copy is created new like every other
/// destination; `fs::copy` would truncate whatever sat there, and its error
/// path removed `dest` even when the file there was never the extractor's.
pub(crate) fn duplicate(written: &Path, dest: &Path, entry: &str) -> Result<u64, RiftError> {
    let copy_failed = |err: &io::Error| {
        RiftError::io(
            format!(
                "could not copy {} to {} (unpacking {entry})",
                written.display(),
                dest.display()
            ),
            err,
        )
    };
    let mut original = File::open(written).map_err(|err| copy_failed(&err))?;
    write_from_reader(dest, entry, &mut original, &copy_failed)
}

/// Counts finished files and reports after each one, naming it: the caller
/// learns from the reports exactly which destinations were created.
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
                item: 0,
            },
        }
    }

    /// The batch's `item` is written whole, `bytes` long.
    pub(crate) fn file_done(&mut self, item: usize, bytes: u64) {
        self.state.files_done += 1;
        self.state.bytes_done += bytes;
        self.state.item = item;
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

    fn assert_refused_over(err: &RiftError, dest: &Path) {
        assert!(
            matches!(err, RiftError::Refused { reason }
                if reason.contains(&dest.display().to_string())),
            "{err:?}"
        );
    }

    #[test]
    fn a_destination_that_is_taken_is_refused_and_left_as_it_was() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("natives/stm/cape.mesh.riftstone-part");
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::write(&dest, b"not the Riftstone's").unwrap();
        let err = write_from_reader(&dest, "cape.mesh", &mut &b"cape"[..], &|_| unreachable!())
            .unwrap_err();
        assert_refused_over(&err, &dest);
        assert_eq!(std::fs::read(&dest).unwrap(), b"not the Riftstone's");

        // A folder there fails the write too, naming it, and stays.
        let folder = dir.path().join("natives/stm/hood.mesh.riftstone-part");
        std::fs::create_dir_all(folder.join("inside")).unwrap();
        let err = DestWriter::create(&folder, "hood.mesh").err().unwrap();
        assert!(
            err.to_string().contains(&folder.display().to_string()),
            "{err:?}"
        );
        #[cfg(unix)] // EEXIST; Windows may answer access denied for a folder
        assert_refused_over(&err, &folder);
        assert!(folder.join("inside").is_dir());
    }

    #[test]
    fn a_duplicate_is_created_new_too() {
        let dir = tempfile::tempdir().unwrap();
        let written = dir.path().join("0.out");
        std::fs::write(&written, b"cape").unwrap();
        let free = dir.path().join("deeper/1.out");
        assert_eq!(duplicate(&written, &free, "cape.mesh").unwrap(), 4);
        assert_eq!(std::fs::read(&free).unwrap(), b"cape");

        let taken = dir.path().join("2.out");
        std::fs::write(&taken, b"not the Riftstone's").unwrap();
        let err = duplicate(&written, &taken, "cape.mesh").unwrap_err();
        assert_refused_over(&err, &taken);
        assert_eq!(std::fs::read(&taken).unwrap(), b"not the Riftstone's");

        // A copy whose original is gone fails without removing what sits at
        // the destination (fs::copy's error path used to delete it).
        std::fs::remove_file(&written).unwrap();
        let err = duplicate(&written, &taken, "cape.mesh").unwrap_err();
        assert!(
            matches!(&err, RiftError::Io { detail } if detail.contains("could not copy")),
            "{err:?}"
        );
        assert_eq!(std::fs::read(&taken).unwrap(), b"not the Riftstone's");
    }

    #[cfg(unix)]
    #[test]
    fn a_link_at_the_destination_is_never_written_through() {
        let dir = tempfile::tempdir().unwrap();
        let vanilla = dir.path().join("re_chunk_000.pak");
        std::fs::write(&vanilla, b"KPKA vanilla").unwrap();
        let dest = dir.path().join("planted.riftstone-part");
        std::os::unix::fs::symlink(&vanilla, &dest).unwrap();
        let err = write_from_reader(&dest, "a.pak", &mut &b"mod bytes"[..], &|_| unreachable!())
            .unwrap_err();
        assert_refused_over(&err, &dest);
        assert_eq!(std::fs::read(&vanilla).unwrap(), b"KPKA vanilla");
        assert!(dest.is_symlink(), "the link itself stays too");

        // A dangling link is refused as well: nothing is created at its target.
        let nowhere = dir.path().join("DD2.exe");
        let dangling = dir.path().join("dangling.riftstone-part");
        std::os::unix::fs::symlink(&nowhere, &dangling).unwrap();
        let err = write_from_reader(&dangling, "a.dll", &mut &b"MZ"[..], &|_| unreachable!())
            .unwrap_err();
        assert_refused_over(&err, &dangling);
        assert!(!nowhere.exists());
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
