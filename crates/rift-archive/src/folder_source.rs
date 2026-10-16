//! Plain folders: a mod unpacked beside the library, or one that was never
//! packed. The walk follows no symlinks (on Windows, junctions count as
//! links too) and stops 64 levels down; it lists files in name order so a
//! folder reads the same on every bench.
//!
//! `read_entry` and `extract` read the files where they lie, and `extract`
//! always copies — the library is read-only (RS-2).

use std::fs::File;
use std::path::{Path, PathBuf};

use rift_core::{
    ArchiveEntry, ExtractItem, ExtractProgress, PackageFormat, PackageSource, RiftError,
};
use walkdir::WalkDir;

use crate::fail;
use crate::index::{EntryIndex, IndexBuilder};
use crate::write::{read_capped, write_from_reader, Tally};

/// How deep a folder package may go. The deepest real collection measured
/// nests 17 levels; anything near 64 is a loop or an accident.
const MAX_DEPTH: usize = 64;

/// A folder package.
#[derive(Debug)]
pub struct FolderSource {
    root: PathBuf,
    index: EntryIndex<PathBuf>,
}

impl FolderSource {
    /// Walk the folder at `root` and index its files.
    pub fn open(root: &Path) -> Result<Self, RiftError> {
        let metadata = std::fs::metadata(root).map_err(|err| RiftError::unreadable(root, err))?;
        if !metadata.is_dir() {
            return Err(fail::unsupported(root));
        }
        let mut builder = IndexBuilder::new(root);
        let walk = WalkDir::new(root)
            .follow_links(false)
            .max_depth(MAX_DEPTH)
            .sort_by_file_name();
        for item in walk {
            let item = item.map_err(|err| {
                let at = err.path().unwrap_or(root).to_path_buf();
                RiftError::unreadable(&at, err)
            })?;
            let kind = item.file_type();
            if kind.is_dir() {
                if item.depth() == MAX_DEPTH {
                    log::warn!(
                        "{}: did not look inside {} — it is {MAX_DEPTH} folders deep",
                        root.display(),
                        item.path().display()
                    );
                }
                continue;
            }
            if !kind.is_file() {
                if kind.is_symlink() {
                    log::warn!(
                        "{}: skipped {}: it is a link, and links are never followed or installed",
                        root.display(),
                        item.path().display()
                    );
                }
                continue;
            }
            let Some(name) = relative_name(root, item.path()) else {
                log::warn!(
                    "{}: skipped {}: its name is not valid Unicode",
                    root.display(),
                    item.path().display()
                );
                continue;
            };
            let size = item
                .metadata()
                .map_err(|err| RiftError::unreadable(item.path(), err))?
                .len();
            builder.push(&name, size, false, item.path().to_path_buf());
        }
        Ok(FolderSource {
            root: root.to_path_buf(),
            index: builder.finish(),
        })
    }
}

/// `path` relative to `root`, segments joined with `/`. `None` when a
/// segment is not valid Unicode.
fn relative_name(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let segments: Option<Vec<&str>> = relative
        .components()
        .map(|component| component.as_os_str().to_str())
        .collect();
    Some(segments?.join("/"))
}

impl PackageSource for FolderSource {
    fn format(&self) -> PackageFormat {
        PackageFormat::Folder
    }

    fn entries(&mut self) -> Result<Vec<ArchiveEntry>, RiftError> {
        Ok(self.index.listing())
    }

    fn read_entry(&mut self, entry: &str, limit: Option<u64>) -> Result<Vec<u8>, RiftError> {
        let (_, found) = self.index.lookup(entry)?;
        if limit == Some(0) {
            return Ok(Vec::new());
        }
        let on_disk = &found.locator;
        let mut file = File::open(on_disk).map_err(|err| RiftError::unreadable(on_disk, err))?;
        read_capped(&mut file, limit, found.size).map_err(|err| RiftError::unreadable(on_disk, err))
    }

    fn extract(
        &mut self,
        items: &[ExtractItem],
        progress: &mut dyn FnMut(ExtractProgress),
    ) -> Result<(), RiftError> {
        let positions = self.index.resolve(&self.root, items)?;
        let mut tally = Tally::new(progress, items.len(), self.index.total_size(&positions));
        for (at, (item, position)) in items.iter().zip(positions).enumerate() {
            let found = self.index.get(position);
            let on_disk = found.locator.as_path();
            let mut file =
                File::open(on_disk).map_err(|err| RiftError::unreadable(on_disk, err))?;
            let written = write_from_reader(&item.dest, &found.path, &mut file, &|err| {
                RiftError::unreadable(on_disk, err)
            })?;
            tally.file_done(at, written);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_names_use_forward_slashes() {
        let root = Path::new("/library/Lantern Glow Recolor");
        assert_eq!(
            relative_name(root, &root.join("natives").join("stm").join("glow.tex")).as_deref(),
            Some("natives/stm/glow.tex")
        );
        assert_eq!(relative_name(root, Path::new("/elsewhere/x")), None);
    }
}
