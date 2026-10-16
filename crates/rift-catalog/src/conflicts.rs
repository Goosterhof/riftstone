//! Conflicts: two installed (or about-to-be-installed) options that put a
//! file at the same game path. Loose paths are compared as paths AND as the
//! engine's murmur3 hashes, so a loose mesh and a pak carrying the same mesh
//! are caught too.
//!
//! ## Keys
//!
//! - **Path** — every loose / REFramework / root game path of every layer,
//!   lower-cased with forward slashes (`NATIVES\STM\a.mesh` and
//!   `natives/stm/a.mesh` are one key, the way Windows sees them).
//! - **Hash** — every loose path under `natives/` is ALSO filed under
//!   [`rift_pak::path_hash`] of its own spelling (the hash folds ASCII case
//!   itself), and every pak contributes the hashes in its table of contents.
//!   A pak stores no names, only hashes: this is how a pak and a loose file —
//!   or two paks — are seen replacing the same game file.
//!
//! A key touched by two or more options is a conflict, reported once. A hash
//! that only loose layers touch is not reported again: those layers already
//! share a path key. So a hash conflict always involves at least one pak.
//!
//! And one game file is one conflict: when loose layers that share a
//! `natives/` path also meet paks carrying its hash, the hash conflict names
//! that path, lists every one of them and has the same winner, so it replaces
//! their path conflict instead of listing the file twice. (The path conflict
//! stands only where the hash conflict does not hold it: Windows folds the
//! case of every letter, the engine's hash only of ASCII letters, so two
//! loose spellings of one file on disk can be two files to the engine.)
//!
//! ## Who wins (§ The DD2 load rules)
//!
//! - Among loose / REFramework / root files the LATER layer in the slice wins
//!   (the deploy engine passes layers in install order; the last write sits on
//!   disk).
//! - A loose file beats every pak carrying the same hash: REFramework's
//!   LooseFileLoader answers the engine's path lookup for any file that exists
//!   on disk, ahead of every pak, `pak_mods` included.
//! - Among paks the higher `pak_order` wins (`pak_mods/` loads in file-name
//!   order and the last name wins); equal orders fall back to slice order.
//!
//! `layers` in a conflict lists every option touching the key from the
//! weakest to the strongest — loser first, winner last — each option once (at
//! its strongest touch, should it appear twice).

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use rift_core::{lower_key, normalize_entry_path, OptionId};
use rift_pak::{path_hash, PathHash};

/// One option's footprint in the game, with its place in the load order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layer {
    pub option_id: OptionId,
    pub name: String,
    /// Game paths of loose / reframework / root files (any case).
    pub paths: Vec<String>,
    /// Path hashes read from the table of contents of each pak it installs.
    pub pak_hashes: Vec<PathHash>,
    /// Pak load order position (higher loads later and wins among paks).
    pub pak_order: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ConflictKey {
    /// Same game path (lower-cased).
    Path { path: String },
    /// Same engine hash; `path` is filled when a loose layer names it.
    Hash {
        hash: PathHash,
        path: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    pub key: ConflictKey,
    /// Every layer touching the key, loser first, winner last.
    pub layers: Vec<OptionId>,
    pub winner: OptionId,
}

/// Who touches one engine hash: layers by index, each list ascending.
#[derive(Debug, Default)]
struct HashTouches {
    /// Layers with a loose file hashing here.
    loose: Vec<usize>,
    /// Layers with a pak carrying this hash.
    paks: Vec<usize>,
    /// The (lower-cased) loose path first seen with this hash.
    path: Option<String>,
}

/// Find every conflict among `layers`, each game file once: path conflicts
/// first (by path), then hash conflicts (those a loose path names, by path;
/// then the rest by hash).
pub fn detect(layers: &[Layer]) -> Vec<Conflict> {
    let mut by_path: HashMap<String, Vec<usize>> = HashMap::new();
    let mut by_hash: HashMap<PathHash, HashTouches> = HashMap::new();

    for (index, layer) in layers.iter().enumerate() {
        let mut own_paths = HashSet::with_capacity(layer.paths.len());
        for raw in &layer.paths {
            let spelled = normalize_entry_path(raw).unwrap_or_else(|_| raw.replace('\\', "/"));
            let key = lower_key(&spelled);
            if key.is_empty() || !own_paths.insert(key.clone()) {
                continue;
            }
            if key.starts_with("natives/") {
                let touches = by_hash.entry(path_hash(&spelled)).or_default();
                push_once(&mut touches.loose, index);
                touches.path.get_or_insert_with(|| key.clone());
            }
            by_path.entry(key).or_default().push(index);
        }
        for hash in &layer.pak_hashes {
            push_once(&mut by_hash.entry(*hash).or_default().paks, index);
        }
    }

    let mut path_conflicts: Vec<(String, Vec<usize>)> = by_path
        .into_iter()
        .filter(|(_, touching)| touching.len() >= 2)
        .collect();
    path_conflicts.sort_unstable_by(|a, b| a.0.cmp(&b.0));

    let mut hash_conflicts: Vec<(PathHash, HashTouches)> = by_hash
        .into_iter()
        .filter(|(_, touches)| {
            !touches.paks.is_empty() && touches.loose.len() + touches.paks.len() >= 2
        })
        .collect();
    hash_conflicts.sort_unstable_by(|a, b| {
        (a.1.path.is_none(), &a.1.path, a.0.combined()).cmp(&(
            b.1.path.is_none(),
            &b.1.path,
            b.0.combined(),
        ))
    });

    // Slice order is already weakest first: the later layer wins. Still
    // sorted by path, for the look-ups below.
    let mut path_reports: Vec<(String, Option<Conflict>)> = path_conflicts
        .into_iter()
        .map(|(path, touching)| {
            let report = conflict(ConflictKey::Path { path: path.clone() }, layers, &touching);
            (path, report)
        })
        .collect();

    let mut hash_reports = Vec::with_capacity(hash_conflicts.len());
    for (hash, touches) in hash_conflicts {
        // Paks by load order, then loose layers by slice order: a loose file
        // beats every pak. A layer with both keeps its loose (stronger) touch,
        // and when no pak of ANOTHER layer is left, the loose layers already
        // share a path key — reported there, not twice.
        let mut paks: Vec<usize> = touches
            .paks
            .into_iter()
            .filter(|index| !touches.loose.contains(index))
            .collect();
        if paks.is_empty() {
            continue;
        }
        paks.sort_by_key(|&index| (layers[index].pak_order, index));
        let order: Vec<usize> = paks.into_iter().chain(touches.loose).collect();
        let path_slot = touches.path.as_deref().and_then(|path| {
            path_reports
                .binary_search_by(|(held, _)| held.as_str().cmp(path))
                .ok()
        });
        let key = ConflictKey::Hash {
            hash,
            path: touches.path,
        };
        let Some(report) = conflict(key, layers, &order) else {
            continue;
        };
        // One game file, one conflict: when the loose layers meeting these
        // paks already share the path's own conflict, this one names the
        // path, lists them all and has the same winner — it replaces that one.
        if let Some(slot) = path_slot {
            let held = &mut path_reports[slot].1;
            if held.as_ref().is_some_and(|held| contains(&report, held)) {
                *held = None;
            }
        }
        hash_reports.push(report);
    }
    path_reports
        .into_iter()
        .filter_map(|(_, report)| report)
        .chain(hash_reports)
        .collect()
}

/// Does `outer` say everything `inner` says: every layer of it, and the
/// same winner?
fn contains(outer: &Conflict, inner: &Conflict) -> bool {
    outer.winner == inner.winner && inner.layers.iter().all(|id| outer.layers.contains(id))
}

/// A conflict over `key` from layer indices in precedence order (weakest
/// first). The same option appearing twice keeps only its strongest place;
/// fewer than two distinct options is no conflict at all.
fn conflict(key: ConflictKey, layers: &[Layer], order: &[usize]) -> Option<Conflict> {
    let mut seen: HashSet<&OptionId> = HashSet::with_capacity(order.len());
    let mut strongest_first: Vec<OptionId> = Vec::with_capacity(order.len());
    for &index in order.iter().rev() {
        let id = &layers[index].option_id;
        if seen.insert(id) {
            strongest_first.push(id.clone());
        }
    }
    if strongest_first.len() < 2 {
        return None;
    }
    let winner = strongest_first[0].clone();
    let mut weakest_first = strongest_first;
    weakest_first.reverse();
    Some(Conflict {
        key,
        layers: weakest_first,
        winner,
    })
}

/// Push `index` unless it is already the last entry (layers are visited in
/// ascending order, so this keeps each list distinct and sorted).
fn push_once(list: &mut Vec<usize>, index: usize) {
    if list.last() != Some(&index) {
        list.push(index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAPE_MESH: &str = "natives/stm/appsystem/cloakwb/mesh/cloak_301_wb_f.mesh.260421070";

    fn layer(name: &str, paths: &[&str], pak_hashes: &[PathHash], pak_order: u32) -> Layer {
        Layer {
            option_id: OptionId(format!("0123456789abcdef#{name}")),
            name: name.to_string(),
            paths: paths.iter().map(|p| (*p).to_string()).collect(),
            pak_hashes: pak_hashes.to_vec(),
            pak_order,
        }
    }

    fn ids(layers: &[&Layer]) -> Vec<OptionId> {
        layers.iter().map(|l| l.option_id.clone()).collect()
    }

    #[test]
    fn loose_against_loose_the_later_layer_wins_case_and_slashes_ignored() {
        let white = layer(
            "White",
            &[CAPE_MESH, "reframework/autorun/cape.lua"],
            &[],
            0,
        );
        let black = layer(
            "Black",
            &[r"NATIVES\STM\AppSystem\CloakWB\mesh\cloak_301_wb_f.mesh.260421070"],
            &[],
            0,
        );
        let grey = layer(
            "Grey",
            &[CAPE_MESH, "REFramework/autorun/Cape.lua", "dinput8.dll"],
            &[],
            0,
        );
        let conflicts = detect(&[white.clone(), black.clone(), grey.clone()]);
        assert_eq!(
            conflicts,
            vec![
                Conflict {
                    key: ConflictKey::Path {
                        path: CAPE_MESH.to_string()
                    },
                    layers: ids(&[&white, &black, &grey]),
                    winner: grey.option_id.clone(),
                },
                Conflict {
                    key: ConflictKey::Path {
                        path: "reframework/autorun/cape.lua".to_string()
                    },
                    layers: ids(&[&white, &grey]),
                    winner: grey.option_id.clone(),
                },
            ],
            "the shared natives path is reported once — as a path, never again as a hash"
        );

        let reversed = detect(&[grey.clone(), white.clone()]);
        assert_eq!(reversed[0].winner, white.option_id, "install order decides");
    }

    #[test]
    fn a_loose_file_beats_any_pak_with_its_hash_wherever_it_sits() {
        let mesh_hash = path_hash(CAPE_MESH);
        let other_hash =
            path_hash("natives/stm/appsystem/cloakwb/tex/cloak_301_alba.tex.251211553");
        // The pak installs AFTER the loose file and still loses.
        let loose = layer(
            "Ermine Cape - White",
            &["Natives/STM/AppSystem/CloakWB/Mesh/cloak_301_wb_f.mesh.260421070"],
            &[],
            0,
        );
        let pak = layer("Lantern Glow Recolor", &[], &[other_hash, mesh_hash], 7);
        let conflicts = detect(&[loose.clone(), pak.clone()]);
        assert_eq!(
            conflicts,
            vec![Conflict {
                key: ConflictKey::Hash {
                    hash: mesh_hash,
                    path: Some(CAPE_MESH.to_string()),
                },
                layers: ids(&[&pak, &loose]),
                winner: loose.option_id.clone(),
            }]
        );
        let json = serde_json::to_value(&conflicts[0]).unwrap();
        assert_eq!(json["key"]["kind"], "hash");
        assert_eq!(json["key"]["path"], CAPE_MESH);
        assert_eq!(json["winner"], loose.option_id.0);
    }

    #[test]
    fn among_paks_the_higher_pak_order_wins_not_the_slice_order() {
        let shared = path_hash("natives/stm/gui/lantern_glow.tex.251211553");
        let late_low = layer("Ember Glow", &[], &[shared], 1);
        let early_high = layer("Azure Glow", &[], &[shared], 4);
        let middle = layer("Violet Glow", &[], &[shared, shared], 2);
        let conflicts = detect(&[early_high.clone(), middle.clone(), late_low.clone()]);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(
            conflicts[0].key,
            ConflictKey::Hash {
                hash: shared,
                path: None
            },
            "no loose file names a pak-only hash"
        );
        assert_eq!(conflicts[0].layers, ids(&[&late_low, &middle, &early_high]));
        assert_eq!(conflicts[0].winner, early_high.option_id);
    }

    #[test]
    fn paks_and_loose_files_on_one_path_list_everyone_weakest_first() {
        let mesh_hash = path_hash(CAPE_MESH);
        let pak_high = layer("Pak High", &[], &[mesh_hash], 9);
        let loose_first = layer("Loose First", &[CAPE_MESH], &[], 0);
        let pak_low = layer("Pak Low", &[], &[mesh_hash], 3);
        let loose_last = layer("Loose Last", &[CAPE_MESH], &[], 0);
        let conflicts = detect(&[
            pak_high.clone(),
            loose_first.clone(),
            pak_low.clone(),
            loose_last.clone(),
        ]);
        assert_eq!(
            conflicts,
            vec![Conflict {
                key: ConflictKey::Hash {
                    hash: mesh_hash,
                    path: Some(CAPE_MESH.to_string()),
                },
                layers: ids(&[&pak_low, &pak_high, &loose_first, &loose_last]),
                winner: loose_last.option_id.clone(),
            }],
            "one game file, one conflict: the hash conflict names the path and holds the loose \
             layers' own path conflict"
        );
    }

    #[test]
    fn three_loose_layers_and_two_paks_on_one_mesh_are_one_conflict() {
        let mesh_hash = path_hash(CAPE_MESH);
        let white = layer("Ermine Cape - White", &[CAPE_MESH], &[], 0);
        let black = layer("Ermine Cape - Black", &[&CAPE_MESH.to_uppercase()], &[], 0);
        let grey = layer("Ermine Cape - Grey", &[CAPE_MESH], &[], 0);
        let azure = layer("Azure Capes", &[], &[mesh_hash], 4);
        let ember = layer("Ember Capes", &[], &[mesh_hash], 2);
        let conflicts = detect(&[
            white.clone(),
            black.clone(),
            grey.clone(),
            azure.clone(),
            ember.clone(),
        ]);
        assert_eq!(
            conflicts,
            vec![Conflict {
                key: ConflictKey::Hash {
                    hash: mesh_hash,
                    path: Some(CAPE_MESH.to_string()),
                },
                layers: ids(&[&ember, &azure, &white, &black, &grey]),
                winner: grey.option_id.clone(),
            }]
        );
    }

    #[test]
    fn a_path_conflict_the_hash_conflict_does_not_hold_still_stands() {
        // Windows folds the case of every letter, the engine's hash only of
        // ASCII ones: two loose spellings of one file on disk can be two
        // files to the engine, and a pak can carry only one of them.
        let pale = "natives/stm/appsystem/cloakwb/mesh/Äsche_301.mesh.260421070";
        let dark = "natives/stm/appsystem/cloakwb/mesh/äsche_301.mesh.260421070";
        assert_ne!(path_hash(pale), path_hash(dark));
        let first = layer("Ash Cape - Pale", &[pale], &[], 0);
        let second = layer("Ash Cape - Dark", &[dark], &[], 0);
        let pak = layer("Ash Capes", &[], &[path_hash(dark)], 3);
        let conflicts = detect(&[first.clone(), second.clone(), pak.clone()]);
        assert_eq!(
            conflicts,
            vec![
                Conflict {
                    key: ConflictKey::Path {
                        path: dark.to_string()
                    },
                    layers: ids(&[&first, &second]),
                    winner: second.option_id.clone(),
                },
                Conflict {
                    key: ConflictKey::Hash {
                        hash: path_hash(dark),
                        path: Some(dark.to_string()),
                    },
                    layers: ids(&[&pak, &second]),
                    winner: second.option_id.clone(),
                },
            ],
            "the pale spelling is in the path conflict only, so both stand"
        );
    }

    #[test]
    fn one_option_never_conflicts_with_itself() {
        let mesh_hash = path_hash(CAPE_MESH);
        // A loose copy and a pak copy of the same file in ONE option, and the
        // same path listed twice under two spellings.
        let both = layer(
            "Ermine Cape",
            &[CAPE_MESH, &CAPE_MESH.to_uppercase()],
            &[mesh_hash, mesh_hash],
            2,
        );
        assert!(detect(std::slice::from_ref(&both)).is_empty());

        // The same option passed twice (installed and about to reinstall).
        assert!(detect(&[both.clone(), both.clone()]).is_empty());

        // With a rival, the twice-passed option appears once, at its strongest.
        let rival = layer("Rival", &[CAPE_MESH], &[], 0);
        let conflicts = detect(&[both.clone(), rival.clone(), both.clone()]);
        assert_eq!(
            conflicts.len(),
            1,
            "its own pak copy adds no second (hash) conflict: {conflicts:#?}"
        );
        assert_eq!(conflicts[0].layers, ids(&[&rival, &both]));
        assert_eq!(conflicts[0].winner, both.option_id);
    }

    #[test]
    fn root_and_reframework_paths_are_never_hashed() {
        let a = layer("A", &["dinput8.dll", "reframework/plugins/x.dll"], &[], 0);
        let pak = layer(
            "Pak",
            &[],
            &[
                path_hash("dinput8.dll"),
                path_hash("reframework/plugins/x.dll"),
            ],
            1,
        );
        assert!(
            detect(&[a, pak]).is_empty(),
            "only natives/ paths are addressed by the engine's hash"
        );
    }

    #[test]
    fn no_layers_no_conflicts() {
        assert!(detect(&[]).is_empty());
        let lonely = layer("Lonely", &[CAPE_MESH], &[path_hash(CAPE_MESH)], 0);
        assert!(detect(&[lonely]).is_empty());
    }
}
