//! Conflicts: who overrides whom in the game folder right now, and whom an
//! option would override if it were installed.
//!
//! The layers handed to `rift_catalog::conflicts::detect` are the journal's
//! installs (the Riftstone's own and adopted ones alike) in install order, so
//! a later install wins a path. Among paks the order REFramework really loads
//! `pak_mods/` in decides (its file-name order, as `read_game_state` reports
//! it). Files no install claims are strays: they take no part.
//!
//! A pak's hashes come from its head and table of contents only — never the
//! payload — and are remembered by file name + size + modification time. A pak
//! whose table cannot be read (a damaged header claiming millions of files, a
//! truncated download, an encrypted table) contributes nothing and is logged
//! as unreadable. A pak whose feature flags no reference reader describes
//! keeps its hashes, but the conflicts it takes part in are marked unconfirmed.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use rift_catalog::{Conflict, Layer};
use rift_core::{lower_key, DeployTarget, ModOption, OptionId, PackageId, PackageSource};
use rift_deploy::{DeployKind, GameState, Journal, PakOwner};
use rift_pak::{PakError, PakReadError, PathHash};

use crate::dto::{ConflictKey, ConflictView, LayerRef, PathHashView};

/// What one pak's table of contents says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PakHashes {
    pub hashes: Vec<PathHash>,
    /// Feature flags outside the ones the references describe.
    pub unconfirmed: bool,
}

/// Why a pak's table of contents could not be used.
fn pak_unreadable(err: &PakReadError) -> String {
    match err {
        PakReadError::Pak(PakError::TooManyEntries { claimed, .. }) => {
            format!("its header claims {claimed} files, so it is damaged")
        }
        other => other.to_string(),
    }
}

/// Read a pak's head + table of contents from any seekable stream.
pub fn read_pak_hashes<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
) -> Result<PakHashes, String> {
    let toc = rift_pak::read_toc_from(reader).map_err(|err| pak_unreadable(&err))?;
    if toc.encrypted {
        return Err("its table of contents is encrypted".into());
    }
    Ok(PakHashes {
        unconfirmed: toc.unknown_flags() != 0,
        hashes: toc.entries.iter().map(|entry| entry.hash).collect(),
    })
}

/// How much of a pak inside a package the first read takes: the header and,
/// for a pak of up to ~21,800 files, its whole table of contents. A solid
/// archive decodes its block from the start on every read, so one read is
/// what matters; a bigger table costs a second, exact one.
pub const PAK_FIRST_READ: u64 = 1024 * 1024;

/// Read a pak entry's head + table of contents out of a package (an option
/// that is not installed yet): one bounded read, two for a giant table —
/// never the payload.
pub fn read_archive_pak_hashes(
    source: &mut dyn PackageSource,
    entry: &str,
) -> Result<PakHashes, String> {
    let head = source
        .read_entry(entry, Some(PAK_FIRST_READ))
        .map_err(|err| err.to_string())?;
    let toc_len = rift_pak::toc_len(&head).map_err(|err| match err {
        PakError::TooManyEntries { claimed, .. } => {
            format!("its header claims {claimed} files, so it is damaged")
        }
        other => other.to_string(),
    })?;
    let head = if u64::try_from(head.len()).unwrap_or(u64::MAX) >= toc_len {
        head
    } else {
        source
            .read_entry(entry, Some(toc_len))
            .map_err(|err| err.to_string())?
    };
    let toc = rift_pak::read_toc(&head).map_err(|err| err.to_string())?;
    if toc.encrypted {
        return Err("its table of contents is encrypted".into());
    }
    Ok(PakHashes {
        unconfirmed: toc.unknown_flags() != 0,
        hashes: toc.entries.iter().map(|entry| entry.hash).collect(),
    })
}

/// A pak on disk: lower-cased game path, size, modification time.
type DiskKey = (String, u64, u64);
/// A pak inside a package: the package, the entry.
type ArchiveKey = (PackageId, String);
/// What a read found: the hashes, or `None` for an unreadable pak.
type Known = Option<Arc<PakHashes>>;

/// Pak tables of contents already read, on disk and inside packages.
#[derive(Default)]
pub struct PakHashCache {
    disk: Mutex<HashMap<DiskKey, Known>>,
    archive: Mutex<HashMap<ArchiveKey, Known>>,
}

impl PakHashCache {
    /// The hashes of `game_root/game_path`, read once per name + size + mtime.
    /// `None` when the pak is gone or unreadable.
    pub fn on_disk(&self, game_root: &Path, game_path: &str) -> Option<Arc<PakHashes>> {
        let path = game_root.join(game_path);
        let meta = std::fs::metadata(&path).ok()?;
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX));
        let key = (lower_key(game_path), meta.len(), mtime);
        if let Some(known) = self
            .disk
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
        {
            return known.clone();
        }
        let read = File::open(&path)
            .map_err(|err| err.to_string())
            .and_then(|mut file| read_pak_hashes(&mut file));
        let value = match read {
            Ok(hashes) => Some(Arc::new(hashes)),
            Err(why) => {
                log::warn!(
                    "{game_path} is unreadable as a pak ({why}); it takes no part in conflicts"
                );
                None
            }
        };
        self.disk
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key, value.clone());
        value
    }

    /// The hashes of a pak inside a package, read once per package + entry.
    pub fn in_archive(
        &self,
        package: &PackageId,
        entry: &str,
        read: impl FnOnce() -> Result<PakHashes, String>,
    ) -> Option<Arc<PakHashes>> {
        let key = (package.clone(), entry.to_string());
        if let Some(known) = self
            .archive
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
        {
            return known.clone();
        }
        let value = match read() {
            Ok(hashes) => Some(Arc::new(hashes)),
            Err(why) => {
                log::warn!("{entry} is unreadable as a pak ({why}); it takes no part in conflicts");
                None
            }
        };
        self.archive
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key, value.clone());
        value
    }
}

/// Layers ready for `detect`, with what the views need on top.
#[derive(Debug, Default)]
pub struct Layers {
    pub layers: Vec<Layer>,
    /// Layers carrying a pak with unconfirmed hashes.
    pub unconfirmed: HashSet<OptionId>,
}

impl Layers {
    fn push(&mut self, layer: Layer, unconfirmed: bool) {
        if unconfirmed {
            self.unconfirmed.insert(layer.option_id.clone());
        }
        self.layers.push(layer);
    }

    fn highest_pak_order(&self) -> u32 {
        self.layers.iter().map(|l| l.pak_order).max().unwrap_or(0)
    }

    fn name_of(&self, id: &OptionId) -> String {
        self.layers
            .iter()
            .find(|l| &l.option_id == id)
            .map_or_else(|| id.0.clone(), |l| l.name.clone())
    }
}

/// Collect the hashes of a set of paks.
fn hashes_of<'a>(paks: impl Iterator<Item = Option<Arc<PakHashes>>> + 'a) -> (Vec<PathHash>, bool) {
    let mut hashes = Vec::new();
    let mut unconfirmed = false;
    for pak in paks.flatten() {
        unconfirmed |= pak.unconfirmed;
        hashes.extend(pak.hashes.iter().copied());
    }
    (hashes, unconfirmed)
}

/// What is in the game now, as layers: the journal's installs in install
/// order.
pub fn game_layers(
    game_root: &Path,
    journal: &Journal,
    state: &GameState,
    cache: &PakHashCache,
) -> Layers {
    // REFramework's real load position of each pak in pak_mods/ (1-based).
    let mut position: HashMap<String, u32> = HashMap::new();
    let mut install_position: HashMap<OptionId, u32> = HashMap::new();
    for pak in &state.paks {
        let Some(index) = pak.load_index else {
            continue;
        };
        let at = u32::try_from(index)
            .unwrap_or(u32::MAX - 1)
            .saturating_add(1);
        position.insert(lower_key(&pak.game_path), at);
        if let PakOwner::Riftstone { option_id, .. } = &pak.owner {
            let slot = install_position.entry(option_id.clone()).or_default();
            *slot = (*slot).max(at);
        }
    }

    let mut layers = Layers::default();
    for install in &journal.installs {
        let paths: Vec<String> = install
            .files
            .iter()
            .filter(|f| f.kind != DeployKind::Pak)
            .map(|f| f.game_path.clone())
            .collect();
        let (pak_hashes, unconfirmed) = hashes_of(
            install
                .pak_files()
                .map(|f| cache.on_disk(game_root, &f.game_path)),
        );
        let pak_order = install_position
            .get(&install.option_id)
            .copied()
            .or_else(|| {
                install
                    .pak_files()
                    .filter_map(|f| position.get(&lower_key(&f.game_path)).copied())
                    .max()
            })
            .unwrap_or(0);
        layers.push(
            Layer {
                option_id: install.option_id.clone(),
                name: install.name.clone(),
                paths,
                pak_hashes,
                pak_order,
            },
            unconfirmed,
        );
    }
    layers
}

/// Add an option that is not installed as the newest layer: its loose,
/// REFramework and root files by game path, its paks by the hashes `read`
/// returns for each pak entry. A new pak gets the highest number, so it loads
/// last and wins among paks.
pub fn add_candidate(
    layers: &mut Layers,
    option: &ModOption,
    mut read_pak: impl FnMut(&str) -> Option<Arc<PakHashes>>,
) {
    let mut paths = Vec::new();
    let mut paks = Vec::new();
    for file in &option.payload {
        match &file.target {
            DeployTarget::Pak { .. } => paks.push(read_pak(&file.entry)),
            DeployTarget::Loose { game_path }
            | DeployTarget::Reframework { game_path }
            | DeployTarget::Root { game_path } => paths.push(game_path.clone()),
        }
    }
    let (pak_hashes, unconfirmed) = hashes_of(paks.into_iter());
    let pak_order = layers.highest_pak_order().saturating_add(1);
    layers.push(
        Layer {
            option_id: option.id.clone(),
            name: option.name.clone(),
            paths,
            pak_hashes,
            pak_order,
        },
        unconfirmed,
    );
}

/// Detect, and turn each conflict into the view the window draws.
pub fn detect(layers: &Layers) -> Vec<ConflictView> {
    if layers.layers.len() < 2 {
        return Vec::new();
    }
    rift_catalog::conflicts::detect(&layers.layers)
        .iter()
        .map(|conflict| view(layers, conflict))
        .collect()
}

fn view(layers: &Layers, conflict: &Conflict) -> ConflictView {
    let layer_ref = |id: &OptionId| LayerRef {
        option_id: id.0.clone(),
        name: layers.name_of(id),
    };
    let (key, by_hash) = match &conflict.key {
        rift_catalog::ConflictKey::Path { path } => {
            (ConflictKey::Path { path: path.clone() }, false)
        }
        rift_catalog::ConflictKey::Hash { hash, path } => (
            ConflictKey::Hash {
                hash: PathHashView {
                    lower: hash.lower,
                    upper: hash.upper,
                },
                path: path.clone(),
            },
            true,
        ),
    };
    ConflictView {
        key,
        layers: conflict.layers.iter().map(layer_ref).collect(),
        winner: layer_ref(&conflict.winner),
        unconfirmed: by_hash
            && conflict
                .layers
                .iter()
                .any(|id| layers.unconfirmed.contains(id)),
    }
}

/// The options that take part in any of `conflicts`.
pub fn conflicted_ids(conflicts: &[ConflictView]) -> HashSet<String> {
    conflicts
        .iter()
        .flat_map(|c| c.layers.iter().map(|l| l.option_id.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// A plain KPKA v4.0 table with these hashes (no payload behind it).
    pub fn pak_bytes(hashes: &[PathHash], flags: u16) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"KPKA");
        bytes.push(4);
        bytes.push(0);
        bytes.extend_from_slice(&flags.to_le_bytes());
        bytes.extend_from_slice(&u32::try_from(hashes.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        for hash in hashes {
            bytes.extend_from_slice(&hash.lower.to_le_bytes());
            bytes.extend_from_slice(&hash.upper.to_le_bytes());
            bytes.extend_from_slice(&[0u8; 40]);
        }
        bytes
    }

    #[test]
    fn pak_heads_give_their_hashes_and_flag_unknown_bits() {
        let hash = rift_pak::path_hash("natives/stm/a.mesh.260421070");
        let plain = read_pak_hashes(&mut Cursor::new(pak_bytes(&[hash], 0))).unwrap();
        assert_eq!(plain.hashes, vec![hash]);
        assert!(!plain.unconfirmed);
        let odd = read_pak_hashes(&mut Cursor::new(pak_bytes(&[hash], 0x0100))).unwrap();
        assert!(
            odd.unconfirmed,
            "a flag no reference describes is unconfirmed"
        );
    }

    #[test]
    fn a_damaged_count_or_a_stranger_is_unreadable() {
        let mut damaged = pak_bytes(&[], 0);
        damaged[8..12].copy_from_slice(&(rift_pak::MAX_ENTRIES + 1).to_le_bytes());
        let why = read_pak_hashes(&mut Cursor::new(damaged)).unwrap_err();
        assert!(why.contains("damaged"), "{why}");
        assert!(read_pak_hashes(&mut Cursor::new(b"PK\x03\x04 not a pak".to_vec())).is_err());
    }

    /// One pak entry served from memory, counting the reads.
    struct OnePak {
        bytes: Vec<u8>,
        reads: Vec<Option<u64>>,
    }

    impl PackageSource for OnePak {
        fn format(&self) -> rift_core::PackageFormat {
            rift_core::PackageFormat::SevenZip
        }

        fn entries(&mut self) -> Result<Vec<rift_core::ArchiveEntry>, rift_core::RiftError> {
            Ok(vec![rift_core::ArchiveEntry {
                path: "Glow/Glow.pak".into(),
                size: self.bytes.len() as u64,
            }])
        }

        fn read_entry(
            &mut self,
            _entry: &str,
            limit: Option<u64>,
        ) -> Result<Vec<u8>, rift_core::RiftError> {
            self.reads.push(limit);
            let end = limit.map_or(self.bytes.len(), |l| {
                self.bytes.len().min(usize::try_from(l).unwrap())
            });
            Ok(self.bytes[..end].to_vec())
        }

        fn extract(
            &mut self,
            _items: &[rift_core::ExtractItem],
            _progress: &mut dyn FnMut(rift_core::ExtractProgress),
        ) -> Result<(), rift_core::RiftError> {
            unreachable!("conflicts never extract")
        }
    }

    #[test]
    fn a_pak_inside_a_package_is_read_once_unless_its_table_is_giant() {
        let hash = rift_pak::path_hash("natives/stm/a.mesh.260421070");
        let mut small = OnePak {
            bytes: pak_bytes(&[hash; 177], 0),
            reads: Vec::new(),
        };
        // A payload behind the table that must never be read.
        small
            .bytes
            .extend(std::iter::repeat_n(0xAB, 4 * 1024 * 1024));
        let read = read_archive_pak_hashes(&mut small, "Glow/Glow.pak").unwrap();
        assert_eq!(read.hashes.len(), 177);
        assert_eq!(
            small.reads,
            vec![Some(PAK_FIRST_READ)],
            "one bounded read: a solid block is decoded once"
        );

        let entries = usize::try_from(PAK_FIRST_READ).unwrap() / rift_pak::ENTRY_LEN + 10;
        let mut giant = OnePak {
            bytes: pak_bytes(&vec![hash; entries], 0),
            reads: Vec::new(),
        };
        let read = read_archive_pak_hashes(&mut giant, "Glow/Glow.pak").unwrap();
        assert_eq!(read.hashes.len(), entries);
        let toc_len = (rift_pak::HEADER_LEN + rift_pak::ENTRY_LEN * entries) as u64;
        assert_eq!(giant.reads, vec![Some(PAK_FIRST_READ), Some(toc_len)]);
    }

    #[test]
    fn the_disk_cache_rereads_only_when_the_file_changes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("pak_mods")).unwrap();
        let path = dir.path().join("pak_mods/zrift-0000-glow.pak");
        let first = rift_pak::path_hash("natives/stm/a.mesh");
        std::fs::write(&path, pak_bytes(&[first], 0)).unwrap();
        let cache = PakHashCache::default();
        let read = cache
            .on_disk(dir.path(), "pak_mods/zrift-0000-glow.pak")
            .unwrap();
        assert_eq!(read.hashes, vec![first]);
        // A rewritten pak (another size) is read again, never served stale.
        let second = rift_pak::path_hash("natives/stm/b.mesh");
        std::fs::write(&path, pak_bytes(&[first, second], 0)).unwrap();
        let reread = cache
            .on_disk(dir.path(), "pak_mods/zrift-0000-glow.pak")
            .unwrap();
        assert_eq!(
            reread.hashes,
            vec![first, second],
            "the size changed, so it is read again"
        );
        assert!(cache.on_disk(dir.path(), "pak_mods/gone.pak").is_none());
    }

    fn layer(id: &str, paths: &[&str], hashes: Vec<PathHash>, pak_order: u32) -> Layer {
        Layer {
            option_id: OptionId(id.into()),
            name: id.into(),
            paths: paths.iter().map(|p| (*p).to_string()).collect(),
            pak_hashes: hashes,
            pak_order,
        }
    }

    #[test]
    fn views_name_both_sides_and_mark_unconfirmed_hash_conflicts() {
        let hash = rift_pak::path_hash("natives/stm/a.mesh");
        let mut layers = Layers::default();
        layers.push(layer("old", &["natives/stm/a.mesh"], vec![], 0), false);
        layers.push(layer("new", &[], vec![hash], 2), true);
        let views = detect(&layers);
        assert!(!views.is_empty());
        let view = &views[0];
        assert_eq!(view.layers.len(), 2);
        assert!(matches!(view.key, ConflictKey::Hash { .. }));
        assert!(view.unconfirmed);
        assert_eq!(
            conflicted_ids(&views),
            HashSet::from(["old".to_string(), "new".to_string()])
        );
    }
}
