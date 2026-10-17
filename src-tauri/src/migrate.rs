//! The v0.3 migration's pure half (docs/WINDOW-CONTRACT.md § v0.3 Legacy
//! data): which ids the journal and `nexus.json` trade for v0.3 ones. The
//! spine runs it after every scan, under the deploy lock
//! ([`crate::state::AppState::migrate`]).
//!
//! - **Folder ids follow the name.** v0.2 hashed a folder package's absolute
//!   path ([`PackageId::legacy_folder`]); v0.3 hashes its name
//!   ([`PackageId::for_folder`]). [`folder_moves`] pairs the two for every
//!   folder in the mods folder: exact, nothing guessed.
//! - **The relink pass.** An install whose package is not in the mods folder
//!   (a folder that moved, an adopted install's stand-in id) is matched to a
//!   library option by rift-deploy's [`relink::propose_on_shelf`], which
//!   reads the whole shelf: a package that is there but yielded no option
//!   this scan (unreadable, half-copied) keeps its installs. [`plan_rekey`]
//!   runs it over the journal as the exact re-key leaves it, and folds both
//!   into one map, so the journal is saved once.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use rift_core::{OptionId, Package, PackageFormat, PackageId};
use rift_deploy::{relink, Below, BelowOwner, Journal};

/// v0.2 id → v0.3 id of every folder package in the mods folder whose two
/// ids differ (`legacy_folder(<absolute path>)` → `for_folder(<name>)`).
pub fn folder_moves(packages: &[Package]) -> BTreeMap<PackageId, PackageId> {
    packages
        .iter()
        .filter(|package| package.format == PackageFormat::Folder)
        .filter_map(|package| {
            // v0.2 hashed `std::path::absolute` of the folder, never canonicalized.
            let at = std::path::absolute(&package.path).unwrap_or_else(|_| package.path.clone());
            let legacy = PackageId::legacy_folder(&at);
            (legacy != package.id).then(|| (legacy, package.id.clone()))
        })
        .collect()
}

/// What the migration asks of the journal, in one save.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RekeyPlan {
    /// Old option id → new one, for ids the journal holds: the exact folder
    /// re-key, then the relink pass's matches.
    pub map: BTreeMap<OptionId, OptionId>,
    /// New option id → its package's file name (what the installed list
    /// names the install by while its package is missing again).
    pub files: BTreeMap<OptionId, String>,
    /// Packages the relink pass moved whole (every install it matched from
    /// the old package went to the same new one): their link records follow.
    pub package_moves: BTreeMap<PackageId, PackageId>,
}

/// Every option id the journal mentions outside a pending intent: its
/// installs, the pak order, and the installs that own a backup or a file
/// kept aside.
pub fn journal_ids(journal: &Journal) -> BTreeSet<OptionId> {
    let owners = journal
        .installs
        .iter()
        .flat_map(|install| &install.files)
        .filter_map(|file| match &file.below {
            Below::Backup {
                owner: BelowOwner::Riftstone { option_id },
                ..
            } => Some(option_id),
            _ => None,
        });
    let kept = journal.parked.iter().filter_map(|kept| match &kept.owner {
        BelowOwner::Riftstone { option_id } => Some(option_id),
        BelowOwner::Unowned => None,
    });
    journal
        .installs
        .iter()
        .map(|install| &install.option_id)
        .chain(&journal.pak_order)
        .chain(owners)
        .chain(kept)
        .cloned()
        .collect()
}

/// The re-key the journal needs against the library `packages` (the mods
/// folder as the last scan found it, every package of it, those that yielded
/// no option included): (1) every id whose package half is a
/// folder's v0.2 id takes the folder's v0.3 id, root half kept, unless the
/// journal already holds the new id; (2) the relink pass over the journal as
/// (1) leaves it. A map the engine would refuse whole (an id the journal
/// holds as a target) is never proposed: such an entry is left out.
pub fn plan_rekey(
    journal: &Journal,
    folder_moves: &BTreeMap<PackageId, PackageId>,
    packages: &[Package],
) -> RekeyPlan {
    let mut exact = BTreeMap::new();
    for id in journal_ids(journal) {
        let Some(to_package) = folder_moves.get(&id.package()) else {
            continue;
        };
        let to = OptionId::new(to_package, id.root());
        if journal.holds(&to) {
            log::warn!("{id} keeps its id: the journal already holds {to}");
            continue;
        }
        exact.insert(id, to);
    }

    let mut moved = journal.clone();
    if let Err(err) = moved.rekey(&exact) {
        log::warn!("the folder ids could not be renewed ({err}); only the relink pass runs");
        exact.clear();
        moved = journal.clone();
    }
    let relinked = relink::propose_on_shelf(&moved.installs, packages);

    // Fold the relink's matches back onto the ids the journal holds now.
    let back: HashMap<&OptionId, &OptionId> = exact.iter().map(|(from, to)| (to, from)).collect();
    let mut map = exact.clone();
    let mut targets: BTreeMap<PackageId, BTreeSet<PackageId>> = BTreeMap::new();
    for (from, to) in &relinked {
        if moved.holds(to) {
            continue;
        }
        let original = back
            .get(from)
            .map_or_else(|| from.clone(), |&id| id.clone());
        targets
            .entry(original.package())
            .or_default()
            .insert(to.package());
        map.insert(original, to.clone());
    }
    let package_moves = targets
        .into_iter()
        .filter_map(|(from, to)| {
            let mut to = to.into_iter();
            match (to.next(), to.next()) {
                (Some(only), None) => Some((from, only)),
                _ => None,
            }
        })
        .collect();

    let by_id: HashMap<&PackageId, &Package> = packages.iter().map(|p| (&p.id, p)).collect();
    let files = map
        .values()
        .filter_map(|to| {
            by_id
                .get(&to.package())
                .map(|package| (to.clone(), package.file_name.clone()))
        })
        .collect();
    RekeyPlan {
        map,
        files,
        package_moves,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rift_core::{DeployTarget, ModOption, PayloadFile, PayloadKinds};
    use rift_deploy::{DeployKind, DeployedFile, InstalledOption, Origin};
    use std::path::{Path, PathBuf};

    const HOOD_MESH: &str = "natives/stm/appsystem/hood/wool_hood.mesh.260421070";

    fn folder(library: &Path, name: &str, roots: &[&str]) -> Package {
        let id = PackageId::for_folder(name);
        Package {
            id: id.clone(),
            path: library.join(name),
            file_name: name.into(),
            display_name: name.into(),
            format: PackageFormat::Folder,
            size: 1,
            modified_ms: 1,
            entry_count: roots.len(),
            options: roots
                .iter()
                .map(|root| ModOption {
                    id: OptionId::new(&id, root),
                    package_id: id.clone(),
                    root: (*root).into(),
                    name: (*root).into(),
                    info: None,
                    screenshot: None,
                    payload: vec![PayloadFile {
                        entry: format!("{root}/{HOOD_MESH}"),
                        target: DeployTarget::Loose {
                            game_path: HOOD_MESH.into(),
                        },
                        size: 4,
                    }],
                    skipped: Vec::new(),
                    kinds: PayloadKinds {
                        loose: 1,
                        ..PayloadKinds::default()
                    },
                    parent: None,
                    addon_for: None,
                    bundle: None,
                    menu_only: false,
                    trail: vec![name.into()],
                })
                .collect(),
            nexus: None,
            problems: Vec::new(),
        }
    }

    fn install(id: &OptionId, root: &str, seq: u64) -> InstalledOption {
        InstalledOption {
            seq,
            option_id: id.clone(),
            name: root.into(),
            package_file: "Wool Hood".into(),
            package_path: PathBuf::from("/old/Wool Hood"),
            package_size: 1,
            option_root: root.into(),
            version: None,
            installed_at: "2026-10-16T10:00:00Z".into(),
            game_build: None,
            origin: Origin::Riftstone,
            files: vec![DeployedFile {
                game_path: HOOD_MESH.into(),
                kind: DeployKind::Loose,
                size: 4,
                entry: Some(format!("{root}/{HOOD_MESH}")),
                below: Below::Nothing,
                fingerprint: None,
            }],
        }
    }

    fn journal(installs: Vec<InstalledOption>) -> Journal {
        let mut journal = Journal::new(Path::new("/game"));
        journal.installs = installs;
        journal
    }

    #[test]
    fn a_folder_in_the_mods_folder_trades_its_path_id_for_its_name_id() {
        let library = std::path::absolute("/bench/mods").unwrap();
        let hood = folder(&library, "Wool Hood", &["Hood"]);
        let moves = folder_moves(std::slice::from_ref(&hood));
        let legacy = PackageId::legacy_folder(&library.join("Wool Hood"));
        assert_eq!(moves, BTreeMap::from([(legacy.clone(), hood.id.clone())]));

        let old = OptionId::new(&legacy, "Hood");
        let plan = plan_rekey(
            &journal(vec![install(&old, "Hood", 1)]),
            &moves,
            std::slice::from_ref(&hood),
        );
        let new = OptionId::new(&hood.id, "Hood");
        assert_eq!(plan.map, BTreeMap::from([(old, new.clone())]));
        assert_eq!(plan.files, BTreeMap::from([(new, "Wool Hood".to_string())]));
        assert!(
            plan.package_moves.is_empty(),
            "the exact moves are the caller's own"
        );
    }

    #[test]
    fn an_install_whose_folder_moved_elsewhere_is_found_by_its_files() {
        let library = std::path::absolute("/bench/new mods").unwrap();
        let hood = folder(&library, "Wool Hood", &["Hood"]);
        // The v0.2 id of the folder where it used to be: no exact move fits.
        let gone = PackageId::legacy_folder(Path::new("/bench/old mods/Wool Hood"));
        let old = OptionId::new(&gone, "Hood");
        let moves = folder_moves(std::slice::from_ref(&hood));
        let plan = plan_rekey(
            &journal(vec![install(&old, "Hood", 1)]),
            &moves,
            std::slice::from_ref(&hood),
        );
        let new = OptionId::new(&hood.id, "Hood");
        assert_eq!(plan.map, BTreeMap::from([(old, new)]));
        assert_eq!(
            plan.package_moves,
            BTreeMap::from([(gone, hood.id.clone())]),
            "the whole package moved: its link record may follow"
        );
    }

    #[test]
    fn a_new_id_the_journal_already_holds_is_never_proposed() {
        let library = std::path::absolute("/bench/mods").unwrap();
        let hood = folder(&library, "Wool Hood", &["Hood"]);
        let moves = folder_moves(std::slice::from_ref(&hood));
        let legacy = PackageId::legacy_folder(&library.join("Wool Hood"));
        let old = OptionId::new(&legacy, "Hood");
        let held = OptionId::new(&hood.id, "Hood");
        let plan = plan_rekey(
            &journal(vec![install(&old, "Hood", 1), install(&held, "Hood", 2)]),
            &moves,
            &[hood],
        );
        assert!(plan.map.is_empty(), "{plan:?}");
    }

    #[test]
    fn ids_held_as_owners_and_in_the_pak_order_move_with_their_install() {
        let library = std::path::absolute("/bench/mods").unwrap();
        let hood = folder(&library, "Wool Hood", &["Hood"]);
        let legacy = PackageId::legacy_folder(&library.join("Wool Hood"));
        let old = OptionId::new(&legacy, "Hood");
        let mut journal = journal(vec![install(&old, "Hood", 1)]);
        journal.pak_order = vec![old.clone()];
        let ids = journal_ids(&journal);
        assert_eq!(ids, BTreeSet::from([old.clone()]));
        let plan = plan_rekey(
            &journal,
            &folder_moves(std::slice::from_ref(&hood)),
            &[hood],
        );
        assert_eq!(plan.map.len(), 1);
        let mut rekeyed = journal.clone();
        assert_eq!(rekeyed.rekey(&plan.map).unwrap(), 1);
        assert_eq!(rekeyed.pak_order, vec![plan.map[&old].clone()]);
    }
}
