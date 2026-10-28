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
//! - **The bytes confirm a relink.** The pass matches by paths and sizes, and
//!   a version of a mod that kept every file's size (a recolor) fits those
//!   too. So a proposal for an install whose files carry fingerprints is made
//!   only once its candidate package's entries hash to them
//!   ([`relink::content_check`]): [`plan_rekey`] asks the caller's verdicts
//!   and leaves out, and lists, every proposal that has none yet
//!   ([`RekeyPlan::unconfirmed`]). The spine reads those entries before the
//!   migration, off the deploy lock, at most [`RELINK_READ_BUDGET`] bytes
//!   per scan ([`within_budget`]), and remembers each verdict for the
//!   session ([`crate::state::AppState::confirm_relinks`]). An install that
//!   recorded no fingerprint keeps the size rule.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

use rift_core::{ModOption, OptionId, Package, PackageFormat, PackageId};
use rift_deploy::relink::ContentCheck;
use rift_deploy::{relink, Below, BelowOwner, Journal};

/// The most bytes one scan unpacks to confirm relinks (the sizes the
/// packages list). A confirmation that would go past what is left waits for
/// the next scan; one that needs more than the whole budget on its own is
/// never read, and its install keeps its id.
pub const RELINK_READ_BUDGET: u64 = 4 << 30;

/// A relink the bytes must confirm before it is made: what to read, and the
/// package to read it from, as the scan found it. The spine remembers each
/// verdict by this for the session: a package that changed (another size or
/// time) or an install that changed asks again.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Confirmation {
    pub check: ContentCheck,
    /// The candidate's package, where the scan found it.
    pub package: PathBuf,
    /// Its size and modification time (ms) as the scan stamped them.
    pub stamp: (u64, u64),
}

impl Confirmation {
    /// Does confirming it read the package? A check with no entries is
    /// decided by its sizes alone.
    pub fn reads(&self) -> bool {
        self.check.entries().next().is_some()
    }

    fn new(check: ContentCheck, package: &Package) -> Confirmation {
        Confirmation {
            check,
            package: package.path.clone(),
            stamp: (package.size, package.modified_ms),
        }
    }
}

/// Which of `unconfirmed` one scan reads (in order, each that still fits
/// what is left of `budget` bytes), and which need more than the whole
/// budget on their own (never read). The rest wait for the next scan.
pub fn within_budget(
    unconfirmed: &[Confirmation],
    budget: u64,
) -> (Vec<&Confirmation>, Vec<&Confirmation>) {
    let mut left = budget;
    let mut read = Vec::new();
    let mut too_large = Vec::new();
    for confirmation in unconfirmed {
        let bytes = confirmation.check.bytes();
        if bytes > budget {
            too_large.push(confirmation);
        } else if bytes <= left {
            left -= bytes;
            read.push(confirmation);
        }
    }
    (read, too_large)
}

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
    /// Relinks the pass proposed whose bytes have no verdict yet: left out
    /// of `map` until the spine has read them.
    pub unconfirmed: Vec<Confirmation>,
    /// Every confirmation the plan asked a verdict for, answered or not.
    pub asked: Vec<Confirmation>,
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
/// (1) leaves it, each proposal for an install that recorded fingerprints
/// kept only when `verdict` says its bytes confirmed it (`Some(true)`):
/// `Some(false)` drops it, and `None` leaves it out and lists it in
/// [`RekeyPlan::unconfirmed`]. A map the engine would refuse whole (an id the
/// journal holds as a target) is never proposed: such an entry is left out.
pub fn plan_rekey(
    journal: &Journal,
    folder_moves: &BTreeMap<PackageId, PackageId>,
    packages: &[Package],
    verdict: &dyn Fn(&Confirmation) -> Option<bool>,
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
    let proposed = relink::propose_on_shelf(&moved.installs, packages);

    // The sizes proposed; the bytes confirm. A proposal whose install
    // recorded fingerprints stands only on a verdict that its candidate's
    // entries hash to them.
    let options: HashMap<&OptionId, (&Package, &ModOption)> = packages
        .iter()
        .flat_map(|package| package.options.iter().map(move |o| (&o.id, (package, o))))
        .collect();
    let mut unconfirmed = Vec::new();
    let mut asked = Vec::new();
    let mut relinked = BTreeMap::new();
    for (from, to) in proposed {
        let (Some(install), Some(&(package, option))) = (moved.find(&from), options.get(&to))
        else {
            continue;
        };
        let Some(check) = relink::content_check(install, option) else {
            // Nothing fingerprinted: the sizes are all there is.
            relinked.insert(from, to);
            continue;
        };
        let confirmation = Confirmation::new(check, package);
        match verdict(&confirmation) {
            Some(true) => {
                relinked.insert(from, to);
            }
            Some(false) => {}
            None => unconfirmed.push(confirmation.clone()),
        }
        asked.push(confirmation);
    }

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
        unconfirmed,
        asked,
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
            actor: rift_deploy::Actor::Player,
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
            &|_| None,
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
            &|_| panic!("an install with no fingerprint asks no verdict"),
        );
        let new = OptionId::new(&hood.id, "Hood");
        assert_eq!(plan.map, BTreeMap::from([(old, new)]));
        assert!(
            plan.asked.is_empty() && plan.unconfirmed.is_empty(),
            "the sizes are all there is"
        );
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
            &|_| None,
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
            &|_| None,
        );
        assert_eq!(plan.map.len(), 1);
        let mut rekeyed = journal.clone();
        assert_eq!(rekeyed.rekey(&plan.map).unwrap(), 1);
        assert_eq!(rekeyed.pak_order, vec![plan.map[&old].clone()]);
    }

    /// [`install`] as the engine records one: its file fingerprinted.
    fn fingerprinted(id: &OptionId, root: &str, seq: u64) -> InstalledOption {
        let mut install = install(id, root, seq);
        install.files[0].fingerprint = Some("ab".repeat(16));
        install
    }

    #[test]
    fn a_relink_whose_install_recorded_fingerprints_waits_for_its_bytes() {
        let library = std::path::absolute("/bench/new mods").unwrap();
        let hood = folder(&library, "Wool Hood", &["Hood"]);
        let gone = PackageId::legacy_folder(Path::new("/bench/old mods/Wool Hood"));
        let old = OptionId::new(&gone, "Hood");
        let new = OptionId::new(&hood.id, "Hood");
        let moves = folder_moves(std::slice::from_ref(&hood));
        let journal = journal(vec![fingerprinted(&old, "Hood", 1)]);
        let plan_with = |verdict: Option<bool>| {
            plan_rekey(&journal, &moves, std::slice::from_ref(&hood), &|_| verdict)
        };

        let waiting = plan_with(None);
        assert!(waiting.map.is_empty(), "no verdict yet: {waiting:?}");
        assert!(waiting.package_moves.is_empty());
        assert_eq!(waiting.unconfirmed.len(), 1);
        let asked = &waiting.unconfirmed[0];
        assert_eq!(asked.check.install(), &old);
        assert_eq!(asked.check.option(), &new);
        assert_eq!(
            asked.check.entries().collect::<Vec<_>>(),
            vec![(format!("Hood/{HOOD_MESH}").as_str(), 4)]
        );
        assert_eq!(asked.package, hood.path);
        assert_eq!(asked.stamp, (hood.size, hood.modified_ms));
        assert_eq!(waiting.asked, waiting.unconfirmed);

        let confirmed = plan_with(Some(true));
        assert_eq!(confirmed.map, BTreeMap::from([(old.clone(), new.clone())]));
        assert_eq!(
            confirmed.package_moves,
            BTreeMap::from([(gone, hood.id.clone())])
        );
        assert!(confirmed.unconfirmed.is_empty());
        assert_eq!(confirmed.asked, waiting.asked);

        let refused = plan_with(Some(false));
        assert!(refused.map.is_empty(), "other bytes: no relink");
        assert!(refused.package_moves.is_empty());
        assert!(refused.unconfirmed.is_empty());
        assert_eq!(refused.asked, waiting.asked);
    }

    #[test]
    fn an_exact_move_needs_no_verdict_while_a_relink_waits_for_one() {
        // One folder still where v0.2 knew it (an exact move), one install
        // whose folder moved elsewhere (a relink): the exact move is planned
        // even while the relink waits for its bytes.
        let library = std::path::absolute("/bench/mods").unwrap();
        let hood = folder(&library, "Wool Hood", &["Hood", "Cowl"]);
        let here = OptionId::new(
            &PackageId::legacy_folder(&library.join("Wool Hood")),
            "Hood",
        );
        let gone = PackageId::legacy_folder(Path::new("/bench/old mods/Wool Hood"));
        let moved = OptionId::new(&gone, "Cowl");
        let plan = plan_rekey(
            &journal(vec![
                fingerprinted(&here, "Hood", 1),
                fingerprinted(&moved, "Cowl", 2),
            ]),
            &folder_moves(std::slice::from_ref(&hood)),
            std::slice::from_ref(&hood),
            &|_| None,
        );
        assert_eq!(
            plan.map,
            BTreeMap::from([(here, OptionId::new(&hood.id, "Hood"))])
        );
        assert_eq!(plan.unconfirmed.len(), 1);
        assert_eq!(plan.unconfirmed[0].check.install(), &moved);
    }

    /// A confirmation that reads `bytes` (one loose file of that size).
    fn confirmation(bytes: u64) -> Confirmation {
        let library = std::path::absolute("/bench/mods").unwrap();
        let mut hood = folder(&library, "Wool Hood", &["Hood"]);
        hood.options[0].payload[0].size = bytes;
        let mut install = fingerprinted(&OptionId("adopted#Hood".into()), "Hood", 1);
        install.files[0].size = bytes;
        let check = relink::content_check(&install, &hood.options[0]).unwrap();
        Confirmation::new(check, &hood)
    }

    #[test]
    fn a_scan_reads_in_order_within_its_budget_and_never_a_check_larger_than_it() {
        let checks: Vec<Confirmation> = [10, 30, 30, 200, 15].map(confirmation).to_vec();
        let (read, too_large) = within_budget(&checks, 60);
        let bytes =
            |list: &[&Confirmation]| -> Vec<u64> { list.iter().map(|c| c.check.bytes()).collect() };
        assert_eq!(
            bytes(&read),
            vec![10, 30, 15],
            "each that still fits, in order; the second 30 waits for the next scan"
        );
        assert_eq!(bytes(&too_large), vec![200]);
        let (read, too_large) = within_budget(&checks, 0);
        assert!(read.is_empty() && too_large.len() == 5);
    }
}
