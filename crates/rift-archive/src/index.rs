//! The entry index every source builds once, when the package is opened.
//!
//! It is where the zip-slip guard runs (every raw name goes through
//! [`normalize_entry_path`]; unsafe ones never get in), where lookups become
//! case-blind, and where duplicates are settled: when two entries share a
//! case-folded path, the later one wins — the copy an extractor would leave
//! on disk — and the earlier one leaves the listing.

use std::collections::HashMap;
use std::path::Path;

use rift_core::{lower_key, normalize_entry_path, ArchiveEntry, ExtractItem, RiftError};

/// One file in the package, as the index keeps it. `L` is where the format
/// finds the bytes: a zip index, a 7z file slot, a RAR header ordinal, a path
/// on disk.
#[derive(Debug, Clone)]
pub(crate) struct IndexedEntry<L> {
    /// Normalized path (forward slashes, original case).
    pub path: String,
    /// Uncompressed size in bytes, as the package declares it.
    pub size: u64,
    pub encrypted: bool,
    pub locator: L,
}

/// The finished index: files in archive order plus a case-folded lookup.
#[derive(Debug)]
pub(crate) struct EntryIndex<L> {
    entries: Vec<IndexedEntry<L>>,
    by_key: HashMap<String, usize>,
}

/// Collects raw entries while a package is listed.
pub(crate) struct IndexBuilder<'p, L> {
    package: &'p Path,
    slots: Vec<Option<IndexedEntry<L>>>,
    by_key: HashMap<String, usize>,
}

impl<'p, L> IndexBuilder<'p, L> {
    pub(crate) fn new(package: &'p Path) -> Self {
        IndexBuilder {
            package,
            slots: Vec::new(),
            by_key: HashMap::new(),
        }
    }

    /// Offer one FILE entry by its raw name (directories are the caller's to
    /// skip). Returns `false` when the name was refused.
    pub(crate) fn push(&mut self, raw_name: &str, size: u64, encrypted: bool, locator: L) -> bool {
        let path = match normalize_entry_path(raw_name) {
            Ok(path) => path,
            Err(_) => {
                log::warn!(
                    "{}: skipped the entry {raw_name:?}: its path climbs out of the package \
                     (`..`, a drive letter, a `:` or a control character), so it is never \
                     read or extracted",
                    self.package.display()
                );
                return false;
            }
        };
        if path.is_empty() {
            log::warn!(
                "{}: skipped a file entry without a name ({raw_name:?})",
                self.package.display()
            );
            return false;
        }
        let slot = self.slots.len();
        if let Some(previous) = self.by_key.insert(lower_key(&path), slot) {
            if let Some(earlier) = self.slots[previous].take() {
                log::warn!(
                    "{}: {} is in the package more than once; the later copy ({path}) wins",
                    self.package.display(),
                    earlier.path
                );
            }
        }
        self.slots.push(Some(IndexedEntry {
            path,
            size,
            encrypted,
            locator,
        }));
        true
    }

    /// An entry indexed so far, by raw name (case-blind). Used to resolve
    /// RAR links, whose target always comes earlier in the archive.
    pub(crate) fn find(&self, raw_name: &str) -> Option<&IndexedEntry<L>> {
        let path = normalize_entry_path(raw_name).ok()?;
        let slot = *self.by_key.get(&lower_key(&path))?;
        self.slots[slot].as_ref()
    }

    pub(crate) fn finish(self) -> EntryIndex<L> {
        let entries: Vec<IndexedEntry<L>> = self.slots.into_iter().flatten().collect();
        let by_key = entries
            .iter()
            .enumerate()
            .map(|(position, entry)| (lower_key(&entry.path), position))
            .collect();
        EntryIndex { entries, by_key }
    }
}

impl<L> EntryIndex<L> {
    /// Find an entry by path — case-blind, and in any dialect the caller
    /// writes it (`Mod\a.txt`, `./Mod/a.txt`). Returns its position too.
    pub(crate) fn lookup(&self, entry: &str) -> Result<(usize, &IndexedEntry<L>), RiftError> {
        let not_found = || RiftError::EntryNotFound {
            entry: entry.to_string(),
        };
        let path = normalize_entry_path(entry).map_err(|_| not_found())?;
        let position = *self.by_key.get(&lower_key(&path)).ok_or_else(not_found)?;
        Ok((position, &self.entries[position]))
    }

    pub(crate) fn get(&self, position: usize) -> &IndexedEntry<L> {
        &self.entries[position]
    }

    pub(crate) fn entries(&self) -> &[IndexedEntry<L>] {
        &self.entries
    }

    /// The listing `PackageSource::entries` hands out.
    pub(crate) fn listing(&self) -> Vec<ArchiveEntry> {
        self.entries
            .iter()
            .map(|entry| ArchiveEntry {
                path: entry.path.clone(),
                size: entry.size,
            })
            .collect()
    }

    /// True when the package holds encrypted entries and nothing readable
    /// besides empty files — the only case where the whole package is
    /// `Encrypted`. A partly encrypted package lists normally.
    pub(crate) fn fully_encrypted(&self) -> bool {
        self.entries.iter().any(|entry| entry.encrypted)
            && self
                .entries
                .iter()
                .all(|entry| entry.encrypted || entry.size == 0)
    }

    /// Resolve an extraction batch before a byte is written: every entry must
    /// exist and be readable, or nothing happens. Returns index positions in
    /// item order.
    pub(crate) fn resolve(
        &self,
        package: &Path,
        items: &[ExtractItem],
    ) -> Result<Vec<usize>, RiftError> {
        items
            .iter()
            .map(|item| {
                let (position, entry) = self.lookup(&item.entry)?;
                if entry.encrypted {
                    Err(crate::fail::encrypted(package))
                } else {
                    Ok(position)
                }
            })
            .collect()
    }

    /// Declared bytes of the entries at `positions` (for progress totals).
    pub(crate) fn total_size(&self, positions: &[usize]) -> u64 {
        positions
            .iter()
            .map(|&position| self.entries[position].size)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index(names: &[(&str, u64, bool)]) -> EntryIndex<usize> {
        let mut builder = IndexBuilder::new(Path::new("Lantern Glow Recolor.zip"));
        for (i, (name, size, encrypted)) in names.iter().enumerate() {
            builder.push(name, *size, *encrypted, i);
        }
        builder.finish()
    }

    #[test]
    fn unsafe_and_nameless_entries_never_get_in() {
        let index = index(&[
            ("Lantern Glow/natives/stm/glow.tex", 10, false),
            ("../evil.dll", 1, false),
            ("Lantern Glow/../../evil.dll", 1, false),
            ("C:/Windows/evil.dll", 1, false),
            ("Lantern Glow/readme.txt:hidden", 1, false),
            ("./", 0, false),
        ]);
        let paths: Vec<_> = index.listing().into_iter().map(|e| e.path).collect();
        assert_eq!(paths, ["Lantern Glow/natives/stm/glow.tex"]);
    }

    #[test]
    fn lookups_are_case_blind_and_dialect_blind() {
        let index = index(&[(r"Lantern Glow\natives\STM\glow.tex", 10, false)]);
        let (position, entry) = index.lookup("lantern glow/NATIVES/stm/GLOW.tex").unwrap();
        assert_eq!(position, 0);
        assert_eq!(entry.path, "Lantern Glow/natives/STM/glow.tex");
        assert!(index.lookup("./Lantern Glow//natives/stm/glow.tex").is_ok());
        assert!(matches!(
            index.lookup("Lantern Glow/natives/stm/missing.tex"),
            Err(RiftError::EntryNotFound { entry }) if entry == "Lantern Glow/natives/stm/missing.tex"
        ));
        assert!(matches!(
            index.lookup("../escape"),
            Err(RiftError::EntryNotFound { .. })
        ));
    }

    #[test]
    fn the_later_duplicate_wins() {
        let index = index(&[
            ("Mod/modinfo.ini", 5, false),
            ("Mod/other.txt", 1, false),
            ("mod/MODINFO.INI", 7, false),
        ]);
        let listing = index.listing();
        assert_eq!(listing.len(), 2);
        assert_eq!(listing[0].path, "Mod/other.txt");
        assert_eq!(listing[1].path, "mod/MODINFO.INI");
        let (_, winner) = index.lookup("Mod/modinfo.ini").unwrap();
        assert_eq!((winner.size, winner.locator), (7, 2));
    }

    #[test]
    fn encryption_fails_the_package_only_when_nothing_readable_is_left() {
        assert!(!index(&[]).fully_encrypted());
        assert!(!index(&[("a", 1, false)]).fully_encrypted());
        assert!(!index(&[("a", 1, true), ("b", 1, false)]).fully_encrypted());
        assert!(index(&[("a", 1, true), ("b", 9, true)]).fully_encrypted());
        assert!(index(&[("a", 1, true), ("empty", 0, false)]).fully_encrypted());
    }
}
