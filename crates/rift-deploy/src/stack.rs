//! The overwrite stack: which journal layers sit at one game path, and in
//! what order.
//!
//! The stack is self-describing. A layer whose `below` is
//! `Backup { owner: Riftstone { option_id: Y } }` holds Y's bytes in its
//! backup slot, so Y sits directly beneath it. The top is the layer nothing
//! sits above. Paths are keyed by [`rift_core::lower_key`]: `natives/STM/a`
//! and `natives/stm/a` are one stack.

use std::collections::HashMap;

use rift_core::{lower_key, OptionId};

use crate::journal::{Below, BelowOwner, DeployedFile, InstalledOption, Journal};

/// One install's file at a path: `journal.installs[install].files[file]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LayerRef {
    pub install: usize,
    pub file: usize,
}

impl LayerRef {
    pub(crate) fn install<'j>(&self, journal: &'j Journal) -> &'j InstalledOption {
        &journal.installs[self.install]
    }

    pub(crate) fn file<'j>(&self, journal: &'j Journal) -> &'j DeployedFile {
        &journal.installs[self.install].files[self.file]
    }
}

/// Every journal layer, by path key. Build one per operation.
pub(crate) struct PathIndex {
    by_key: HashMap<String, Vec<LayerRef>>,
}

impl PathIndex {
    pub(crate) fn build(journal: &Journal) -> Self {
        let mut by_key: HashMap<String, Vec<LayerRef>> = HashMap::new();
        for (install, option) in journal.installs.iter().enumerate() {
            for (file, deployed) in option.files.iter().enumerate() {
                by_key
                    .entry(lower_key(&deployed.game_path))
                    .or_default()
                    .push(LayerRef { install, file });
            }
        }
        PathIndex { by_key }
    }

    pub(crate) fn layers(&self, key: &str) -> &[LayerRef] {
        self.by_key.get(key).map(Vec::as_slice).unwrap_or_default()
    }

    /// Is any journal layer at this path?
    pub(crate) fn claimed(&self, key: &str) -> bool {
        !self.layers(key).is_empty()
    }

    /// The layer directly above `option_id` at this path — the one whose
    /// backup holds `option_id`'s bytes.
    pub(crate) fn above(
        &self,
        journal: &Journal,
        key: &str,
        option_id: &OptionId,
    ) -> Option<LayerRef> {
        self.layers(key).iter().copied().find(|layer| {
            &layer.install(journal).option_id != option_id
                && matches!(
                    &layer.file(journal).below,
                    Below::Backup { owner: BelowOwner::Riftstone { option_id: under }, .. } if under == option_id
                )
        })
    }

    /// The top of the stack: the layer nothing sits above. Should the journal
    /// ever hold several unlinked chains at one path, the newest install's
    /// top is the top.
    pub(crate) fn top(&self, journal: &Journal, key: &str) -> Option<LayerRef> {
        self.layers(key)
            .iter()
            .copied()
            .filter(|layer| {
                self.above(journal, key, &layer.install(journal).option_id)
                    .is_none()
            })
            .max_by_key(|layer| layer.install(journal).seq)
    }

    /// The whole stack, bottom first.
    #[cfg(test)]
    pub(crate) fn stack(&self, journal: &Journal, key: &str) -> Vec<LayerRef> {
        let mut out = Vec::new();
        let mut current = self.top(journal, key);
        while let Some(layer) = current {
            if out.contains(&layer) {
                break;
            }
            out.push(layer);
            current = match &layer.file(journal).below {
                Below::Backup {
                    owner: BelowOwner::Riftstone { option_id },
                    ..
                } => self
                    .layers(key)
                    .iter()
                    .copied()
                    .find(|l| &l.install(journal).option_id == option_id),
                _ => None,
            };
        }
        out.reverse();
        out
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::journal::{DeployKind, Origin};

    fn layer(seq: u64, id: &str, path: &str, below: Below) -> InstalledOption {
        InstalledOption {
            seq,
            option_id: OptionId(id.into()),
            name: id.into(),
            package_file: String::new(),
            package_path: PathBuf::new(),
            package_size: 0,
            option_root: String::new(),
            version: None,
            installed_at: String::new(),
            game_build: None,
            actor: crate::journal::Actor::Player,
            origin: Origin::Riftstone,
            files: vec![DeployedFile {
                game_path: path.into(),
                kind: DeployKind::Loose,
                size: 1,
                entry: None,
                below,
                fingerprint: None,
            }],
        }
    }

    fn backup_of(id: &str) -> Below {
        Below::Backup {
            store: format!("backups/x/{id}"),
            owner: BelowOwner::Riftstone {
                option_id: OptionId(id.into()),
            },
        }
    }

    #[test]
    fn the_stack_follows_the_backup_links_whatever_the_case() {
        let mut journal = Journal::new(Path::new("/g"));
        journal
            .installs
            .push(layer(1, "a", "natives/STM/x.mesh", Below::Nothing));
        // C was installed last, but a layer's place is its link, not its seq.
        journal
            .installs
            .push(layer(3, "c", "natives/stm/x.mesh", backup_of("b")));
        journal
            .installs
            .push(layer(2, "b", "NATIVES/stm/X.MESH", backup_of("a")));
        let index = PathIndex::build(&journal);
        let key = "natives/stm/x.mesh";
        let names: Vec<&str> = index
            .stack(&journal, key)
            .iter()
            .map(|l| l.install(&journal).name.as_str())
            .collect();
        assert_eq!(names, ["a", "b", "c"]);
        assert_eq!(
            index.top(&journal, key).unwrap().install(&journal).name,
            "c"
        );
        let above_a = index.above(&journal, key, &OptionId("a".into())).unwrap();
        assert_eq!(above_a.install(&journal).name, "b");
        assert!(index.above(&journal, key, &OptionId("c".into())).is_none());
    }
}
