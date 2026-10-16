//! The sibling cache for sequential formats (7z blocks, RAR archives).
//!
//! The catalog scans a package by reading every `modinfo.ini` with
//! `read_entry`, one call each. In a solid archive each call decompresses
//! everything before its entry: 281 options would cost 281 passes over a
//! 268 MB stream. So when `read_entry` asks for a small entry, the pass also
//! keeps every small, same-named entry it can reach on the way (case-blind
//! file name: every `modinfo.ini`), and the next 280 calls are served from
//! memory. Large entries are never kept, and the cache has a fixed budget.
//!
//! **Best effort.** Capturing a sibling is a favour, never a condition: a
//! sibling whose read fails costs the entry asked for nothing (the sources
//! return the target and keep what the pass got whole), and it is *refused*
//! from then on — read again only when someone asks for it by name, so one
//! damaged option is not retried by every later pass.

use std::collections::{HashMap, HashSet};

use rift_core::lower_key;
use rift_core::path::file_name;

use crate::index::{EntryIndex, IndexedEntry};

/// An entry this size or smaller counts as small: worth capturing whole when
/// a pass goes by it, and served from memory afterwards.
pub(crate) const SMALL_ENTRY_MAX: u64 = 256 * 1024;

/// Most bytes one open package keeps cached.
pub(crate) const BUDGET: u64 = 16 * 1024 * 1024;

#[derive(Debug, Default)]
pub(crate) struct SiblingCache {
    entries: HashMap<usize, Vec<u8>>,
    held: u64,
    /// Entries whose capture as a sibling failed: never offered again.
    refused: HashSet<usize>,
}

impl SiblingCache {
    pub(crate) fn get(&self, position: usize) -> Option<&[u8]> {
        self.entries.get(&position).map(Vec::as_slice)
    }

    pub(crate) fn contains(&self, position: usize) -> bool {
        self.entries.contains_key(&position)
    }

    /// Keep `bytes` for the entry at `position`, unless the budget is spent.
    pub(crate) fn keep(&mut self, position: usize, bytes: Vec<u8>) {
        let size = bytes.len() as u64;
        if self.entries.contains_key(&position) || self.held + size > BUDGET {
            return;
        }
        self.held += size;
        self.entries.insert(position, bytes);
    }

    /// Never offer the entries at `positions` as siblings again: their
    /// capture failed. A read that asks for one of them by name still reads it.
    pub(crate) fn refuse(&mut self, positions: &[usize]) {
        self.refused.extend(positions.iter().copied());
    }

    /// Positions worth capturing alongside the small entry at `target`: the
    /// other small, readable, not-yet-cached, not-refused entries with the
    /// same file name that `reachable` admits (same block, known size), in
    /// archive order, within what is left of the budget. The target itself
    /// is not included.
    pub(crate) fn siblings<L>(
        &self,
        index: &EntryIndex<L>,
        target: usize,
        reachable: impl Fn(&IndexedEntry<L>) -> bool,
    ) -> Vec<usize> {
        let name = lower_key(file_name(&index.get(target).path));
        let mut room = BUDGET.saturating_sub(self.held + index.get(target).size);
        let mut chosen = Vec::new();
        for (position, entry) in index.entries().iter().enumerate() {
            if position == target
                || entry.encrypted
                || entry.size > SMALL_ENTRY_MAX
                || entry.size > room
                || self.contains(position)
                || self.refused.contains(&position)
                || lower_key(file_name(&entry.path)) != name
                || !reachable(entry)
            {
                continue;
            }
            room -= entry.size;
            chosen.push(position);
        }
        chosen
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::index::IndexBuilder;

    fn index(entries: &[(&str, u64)]) -> EntryIndex<usize> {
        let mut builder = IndexBuilder::new(Path::new("Vernworth Cloaks Collection.rar"));
        for (i, (name, size)) in entries.iter().enumerate() {
            builder.push(name, *size, false, i);
        }
        builder.finish()
    }

    #[test]
    fn siblings_share_the_file_name_and_stay_small() {
        let index = index(&[
            ("Cloaks/modinfo.ini", 100),
            ("Cloaks/Ermine Cape/MODINFO.INI", 120),
            ("Cloaks/Ermine Cape/natives/stm/cape.mesh", 9_000_000),
            ("Cloaks/Huge/modinfo.ini", SMALL_ENTRY_MAX + 1),
            ("Cloaks/Wool Cape/modinfo.ini", 90),
            ("Cloaks/Wool Cape/readme.txt", 40),
        ]);
        let cache = SiblingCache::default();
        assert_eq!(cache.siblings(&index, 0, |_| true), vec![1, 4]);
        assert_eq!(
            cache.siblings(&index, 0, |entry| entry.locator != 4),
            vec![1]
        );
    }

    #[test]
    fn a_refused_sibling_is_never_offered_again() {
        let index = index(&[
            ("Cloaks/Cape 0/modinfo.ini", 100),
            ("Cloaks/Cape 1/modinfo.ini", 100),
            ("Cloaks/Cape 2/modinfo.ini", 100),
        ]);
        let mut cache = SiblingCache::default();
        cache.refuse(&[1]);
        assert_eq!(cache.siblings(&index, 0, |_| true), vec![2]);
        assert_eq!(
            cache.siblings(&index, 1, |_| true),
            vec![0, 2],
            "refusal only stops it riding along; it can still be the target"
        );
    }

    #[test]
    fn the_budget_is_never_exceeded() {
        let mut cache = SiblingCache::default();
        cache.keep(0, vec![0; usize::try_from(BUDGET).unwrap() - 10]);
        cache.keep(1, vec![0; 11]);
        assert!(cache.get(0).is_some());
        assert!(cache.get(1).is_none(), "over budget");
        cache.keep(2, vec![0; 10]);
        assert!(cache.get(2).is_some(), "exactly fits");
    }
}
