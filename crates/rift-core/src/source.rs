//! The reading contract every package format implements (rift-archive).
//!
//! A package is opened once, listed once, and then asked for single entries
//! (modinfo.ini, a screenshot, the head of a pak) or for a batch extraction
//! (an install). Batch extraction exists because solid 7z and rar archives
//! decompress sequentially: pulling 40 files one by one would decompress the
//! solid block 40 times.

use std::path::PathBuf;

use crate::error::RiftError;
use crate::model::{ArchiveEntry, PackageFormat};

/// One file to pull out of a package and where to put it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractItem {
    /// Normalized entry path inside the package.
    pub entry: String,
    /// Absolute destination file path. The extractor creates the parent
    /// folders, and always creates the file itself new: when anything
    /// already sits at `dest` — a file, a folder, a link, even a dangling
    /// one — the batch stops with [`RiftError::Refused`] naming it, and the
    /// thing there is left exactly as it was (never truncated, written
    /// through or removed). Callers (the deploy engine) extract to a
    /// temporary name they checked free first.
    pub dest: PathBuf,
}

/// Progress of a batch extraction: one report per finished file, for the
/// window's progress line — and for the caller's record of which
/// destinations the extractor created (`item`), should the batch stop early.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtractProgress {
    pub files_done: usize,
    pub files_total: usize,
    pub bytes_done: u64,
    pub bytes_total: u64,
    /// The file just finished: its index in the batch's `items`. Solid
    /// archives finish files in archive order, not in the order asked.
    pub item: usize,
}

pub trait PackageSource: Send {
    fn format(&self) -> PackageFormat;

    /// Every FILE in the package (no directory entries), normalized paths,
    /// in archive order. Entries with unsafe paths are dropped here (and
    /// reported by the implementation through `log::warn!`) so they can never
    /// reach an install plan.
    fn entries(&mut self) -> Result<Vec<ArchiveEntry>, RiftError>;

    /// The bytes of one entry. `limit` caps how much is read (a pak's table
    /// of contents needs only its head; a screenshot needs all of it) —
    /// `None` reads the whole entry. Returns [`RiftError::EntryNotFound`] for
    /// an unknown path. Matching is case-insensitive.
    fn read_entry(&mut self, entry: &str, limit: Option<u64>) -> Result<Vec<u8>, RiftError>;

    /// Extract a batch. Implementations must make at most one sequential pass
    /// over solid archives, create every destination new (see
    /// [`ExtractItem::dest`]), write each file fully before reporting it done
    /// (once, naming its `item`), and stop at the first error. Files already
    /// reported stay written; a file that failed half-way is removed again
    /// (best effort). The reports are how the caller knows which destinations
    /// the extractor created: it takes nothing unreported for its own.
    fn extract(
        &mut self,
        items: &[ExtractItem],
        progress: &mut dyn FnMut(ExtractProgress),
    ) -> Result<(), RiftError>;
}
