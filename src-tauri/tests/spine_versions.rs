//! Versions of one file (v0.5, WINDOW-CONTRACT § v0.5) through the spine: two invented versions
//! of one invented Nexus file in the bench's mods folder, tied by their links in `nexus.json`
//! (the same mod and file group). They are one entry in the library and on the Nexus page; an
//! install of one replaces the other's install in one step and puts the older version away;
//! keep only the newest cleans up both installed; the older-versions folder lists what it holds
//! and gives it back. Every name and id here is invented.

mod support;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use rift_core::{LinkSource, NexusLink, OptionId, PackageId, RiftError};
use riftstone_lib::commands::{deploy, library, nexus, versions};
use riftstone_lib::persist::DataDirs;
use riftstone_lib::state::AppState;
use serde_json::json;
use support::{write_zip, Bench};

const MOD: u64 = 7140;
const GROUP: u64 = 714_000;
const OLD: &str = "Hearth Embers 1.0.zip";
const NEW: &str = "Hearth Embers 1.1.zip";
const ROOT: &str = "Hearth Embers";
const EMBER: &str = "natives/stm/effect/hearth/ember.tex.251211553";
const SMOKE: &str = "natives/stm/effect/hearth/smoke.tex.251211553";
const SPARK: &str = "natives/stm/effect/hearth/spark.tex.251211553";

struct Versions {
    bench: Bench,
    dirs: DataDirs,
    old: PackageId,
    new: PackageId,
}

impl Versions {
    fn option(&self, package: &PackageId) -> String {
        OptionId::new(package, ROOT).0
    }

    /// Where the older versions go: beside the bench's mods folder.
    fn older_folder(&self) -> PathBuf {
        self.bench.mods.with_file_name("mods (older versions)")
    }

    fn open(&self) -> Arc<AppState> {
        let (state, _) = self.bench.open(self.bench.config(self.dirs.clone()));
        assert!(state.scan_blocking());
        state
    }

    fn in_mods(&self, name: &str) -> bool {
        self.bench.mods.join(name).exists()
    }

    fn game_file(&self, path: &str) -> Option<Vec<u8>> {
        fs::read(self.bench.game.join(path)).ok()
    }

    fn installed(&self, state: &AppState) -> Vec<String> {
        deploy::get_installed_impl(state)
            .unwrap()
            .installs
            .into_iter()
            .map(|row| row.option_id)
            .collect()
    }
}

fn link(file_id: u64, version: &str, uploaded_ms: u64) -> NexusLink {
    NexusLink {
        mod_id: MOD,
        file_id: Some(file_id),
        group_id: Some(GROUP),
        sqid: None,
        version: Some(version.into()),
        uploaded_ms: Some(uploaded_ms),
        source: LinkSource::Download,
    }
}

/// `nexus.json` holding these links, as the stone writes them.
fn seed_links(dirs: &DataDirs, links: &[(&PackageId, NexusLink)]) {
    let packages: serde_json::Map<String, serde_json::Value> = links
        .iter()
        .map(|(id, link)| (id.0.clone(), json!({ "link": link })))
        .collect();
    let file = dirs.nexus_file();
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(
        &file,
        serde_json::to_vec(&json!({ "schema": 1, "packages": packages })).unwrap(),
    )
    .unwrap();
}

fn both_linked(v: &Versions) {
    seed_links(
        &v.dirs,
        &[
            (&v.old, link(3001, "1.0", 1_000)),
            (&v.new, link(3002, "1.1", 2_000)),
        ],
    );
}

/// The bench with both versions in its mods folder, linked as one file when `linked`. The game
/// folder holds its own ember, which both versions replace.
fn versions_bench(linked: bool) -> Versions {
    let bench = Bench::new();
    fs::create_dir_all(bench.game.join("natives/stm/effect/hearth")).unwrap();
    fs::write(bench.game.join(EMBER), b"the game's own ember").unwrap();
    write_zip(
        &bench.mods.join(OLD),
        &[
            (
                "Hearth Embers/modinfo.ini",
                b"name=Hearth Embers\r\nversion=1.0\r\n",
            ),
            (&format!("Hearth Embers/{EMBER}"), b"ember 1.0"),
            (&format!("Hearth Embers/{SMOKE}"), b"smoke 1.0"),
        ],
    );
    write_zip(
        &bench.mods.join(NEW),
        &[
            (
                "Hearth Embers/modinfo.ini",
                b"name=Hearth Embers\r\nversion=1.1\r\n",
            ),
            (&format!("Hearth Embers/{EMBER}"), b"ember 1.1"),
            (&format!("Hearth Embers/{SPARK}"), b"spark 1.1"),
        ],
    );
    let id = |name: &str| {
        PackageId::for_archive(name, fs::metadata(bench.mods.join(name)).unwrap().len())
    };
    let (old, new) = (id(OLD), id(NEW));
    let dirs = DataDirs::under(&bench.data_home());
    let v = Versions {
        bench,
        dirs,
        old,
        new,
    };
    if linked {
        both_linked(&v);
    }
    v
}

#[test]
fn two_versions_of_one_file_are_one_entry_in_the_library_and_on_the_nexus_page() {
    let v = versions_bench(true);
    let state = v.open();
    let view = library::get_library_impl(&state);
    let entry = |id: &PackageId| {
        view.packages
            .iter()
            .find(|p| p.package.id == id.0)
            .unwrap_or_else(|| panic!("{id} is listed"))
    };
    // Both are still listed; the family says which leads.
    let lead = entry(&v.new)
        .family
        .as_ref()
        .expect("the newer version is in a family");
    assert_eq!(lead.lead, v.new.0);
    assert_eq!(lead.version.as_deref(), Some("1.1"));
    assert_eq!(lead.older.len(), 1);
    assert_eq!(lead.older[0].package_id, v.old.0);
    assert_eq!(lead.older[0].file_name, OLD);
    assert_eq!(lead.older[0].version.as_deref(), Some("1.0"));
    assert_eq!(lead.older[0].installed, 0);
    let older = entry(&v.old).family.as_ref().unwrap();
    assert_eq!(older.lead, v.new.0);
    assert!(
        older.older.is_empty(),
        "only the lead carries the older ones"
    );
    let alone = view
        .packages
        .iter()
        .find(|p| p.package.file_name == "Lantern Glow Recolor.zip")
        .unwrap();
    assert!(
        alone.family.is_none(),
        "a package with no other version is in no family"
    );

    let rows = nexus::get_updates_impl(&state).rows;
    assert!(
        rows.iter().all(|row| row.package_id != v.old.0),
        "an older version has no row of its own"
    );
    let row = rows.iter().find(|row| row.package_id == v.new.0).unwrap();
    assert_eq!(row.older.len(), 1);
    assert_eq!(row.older[0].file_name, OLD);
}

#[test]
fn installing_the_newer_version_replaces_the_older_install_and_puts_it_away() {
    let v = versions_bench(true);
    let state = v.open();
    deploy::install_impl(&state, &v.option(&v.old)).unwrap();
    assert_eq!(v.game_file(EMBER).unwrap(), b"ember 1.0");
    let old_bytes = fs::read(v.bench.mods.join(OLD)).unwrap();

    let plan = deploy::plan_install_impl(&state, &v.option(&v.new)).unwrap();
    assert_eq!(plan.refused, None);
    assert_eq!(plan.replaces.len(), 1, "{:?}", plan.replaces);
    let replaced = &plan.replaces[0];
    assert_eq!(replaced.option_id, v.option(&v.old));
    assert_eq!(replaced.name, ROOT);
    assert_eq!(replaced.version.as_deref(), Some("1.0"));
    assert_eq!(
        replaced.removals, 2,
        "the ember it restores and the smoke it removes"
    );

    let result = deploy::install_impl(&state, &v.option(&v.new)).unwrap();
    assert_eq!(result.put_away_failed, None);
    assert_eq!(result.put_away.len(), 1);
    assert_eq!(result.put_away[0].file_name, OLD);
    assert_eq!(result.put_away[0].landed_as, OLD);
    assert_eq!(
        v.installed(&state),
        [v.option(&v.new)],
        "one version installed, not two"
    );
    assert_eq!(v.game_file(EMBER).unwrap(), b"ember 1.1");
    assert_eq!(v.game_file(SPARK).unwrap(), b"spark 1.1");
    assert_eq!(
        v.game_file(SMOKE),
        None,
        "what only the older version had left with it"
    );

    // The older archive moved whole, beside the mods folder, out of the library.
    assert!(!v.in_mods(OLD) && v.in_mods(NEW));
    assert_eq!(fs::read(v.older_folder().join(OLD)).unwrap(), old_bytes);
    state.wait_until_scanned();
    let view = library::get_library_impl(&state);
    assert!(view.packages.iter().all(|p| p.package.id != v.old.0));
    let lead = view
        .packages
        .iter()
        .find(|p| p.package.id == v.new.0)
        .unwrap();
    assert!(lead.family.is_none(), "alone in the mods folder now");

    // Shown in Explorer, Rust-side, now that it is there.
    versions::open_older_versions_folder_impl(&state).unwrap();
    assert_eq!(
        v.bench.launcher.seen().last(),
        Some(&riftstone_lib::play::Launched::Folder(v.older_folder()))
    );

    // The folder lists it with the record nexus.json kept for it.
    let shelf = versions::get_older_versions_impl(&state).unwrap();
    assert!(shelf.exists);
    assert_eq!(shelf.items.len(), 1);
    let item = &shelf.items[0];
    assert_eq!(item.file_name, OLD);
    assert_eq!(item.size, old_bytes.len() as u64);
    assert_eq!(item.version.as_deref(), Some("1.0"));
    assert!(item.newer_in_mods, "1.1 is in the mods folder");

    // The newer version uninstalls to the game's own file, as the older one would have.
    deploy::uninstall_impl(&state, &v.option(&v.new)).unwrap();
    assert_eq!(v.game_file(EMBER).unwrap(), b"the game's own ember");
    assert_eq!(v.game_file(SPARK), None);
}

#[test]
fn a_version_put_away_comes_back_into_its_family() {
    let v = versions_bench(true);
    let state = v.open();
    deploy::install_impl(&state, &v.option(&v.old)).unwrap();
    deploy::install_impl(&state, &v.option(&v.new)).unwrap();
    state.wait_until_scanned();
    assert!(!v.in_mods(OLD));

    let back = versions::bring_back_impl(&state, "hearth embers 1.0.ZIP").unwrap();
    assert_eq!(
        back.landed_as, OLD,
        "found case-blind, back under its own name"
    );
    assert!(v.in_mods(OLD));
    assert!(!v.older_folder().join(OLD).exists());
    state.wait_until_scanned();
    let view = library::get_library_impl(&state);
    let lead = view
        .packages
        .iter()
        .find(|p| p.package.id == v.new.0)
        .unwrap();
    let family = lead.family.as_ref().expect("in its family again");
    assert_eq!(family.older[0].package_id, v.old.0);
    assert!(versions::get_older_versions_impl(&state)
        .unwrap()
        .items
        .is_empty());

    // Only what the folder lists comes back: never a path, never another file.
    for name in [
        "../mods/Lantern Glow Recolor.zip",
        "Hearth Embers 1.0.zip",
        "",
    ] {
        assert!(
            matches!(
                versions::bring_back_impl(&state, name),
                Err(RiftError::EntryNotFound { .. })
            ),
            "{name:?}"
        );
    }
}

#[test]
fn installing_an_older_version_on_purpose_replaces_the_newer_install_and_puts_nothing_away() {
    let v = versions_bench(true);
    let state = v.open();
    // An install of the newer version puts an older one away even when it was never installed.
    let first = deploy::install_impl(&state, &v.option(&v.new)).unwrap();
    assert_eq!(first.put_away.len(), 1);
    assert!(!v.in_mods(OLD));
    state.wait_until_scanned();
    // Rolling back: bring it back, then install it.
    versions::bring_back_impl(&state, OLD).unwrap();
    state.wait_until_scanned();
    let result = deploy::install_impl(&state, &v.option(&v.old)).unwrap();
    assert!(result.put_away.is_empty() && result.put_away_failed.is_none());
    assert_eq!(v.installed(&state), [v.option(&v.old)]);
    assert_eq!(v.game_file(EMBER).unwrap(), b"ember 1.0");
    assert_eq!(v.game_file(SPARK), None);
    assert!(
        v.in_mods(OLD) && v.in_mods(NEW),
        "a newer version is never put away"
    );
    assert!(versions::get_older_versions_impl(&state)
        .unwrap()
        .items
        .is_empty());
}

#[test]
fn a_put_away_that_cannot_move_leaves_the_install_standing_and_says_so() {
    let v = versions_bench(true);
    // Something that is not a folder stands where the older-versions folder goes.
    fs::write(v.older_folder(), b"not a folder").unwrap();
    let state = v.open();
    deploy::install_impl(&state, &v.option(&v.old)).unwrap();
    let result = deploy::install_impl(&state, &v.option(&v.new)).unwrap();
    assert!(result.put_away.is_empty());
    let sentence = result.put_away_failed.expect("the window is told");
    assert!(sentence.contains(OLD), "{sentence}");
    assert!(
        sentence.starts_with(&format!(
            "\"{OLD}\" could not be moved to the older-versions folder: "
        )),
        "{sentence}"
    );
    let bench_dir = v.bench.dir.path().to_string_lossy().into_owned();
    assert!(
        !sentence.contains(&bench_dir),
        "a file's own name, never a path: {sentence}"
    );
    assert_eq!(
        v.installed(&state),
        [v.option(&v.new)],
        "the install stands"
    );
    assert_eq!(v.game_file(EMBER).unwrap(), b"ember 1.1");
    assert!(v.in_mods(OLD), "the older version stays where it was");
    assert_eq!(fs::read(v.older_folder()).unwrap(), b"not a folder");

    // Try again, while the folder is still blocked: the same sentence, nothing moved.
    let again = versions::retry_put_away_impl(&state, &v.new.0).unwrap();
    assert!(again.put_away.is_empty());
    assert!(again.put_away_failed.is_some_and(|s| s.contains(OLD)));
    assert!(v.in_mods(OLD));

    // The folder is free: the spine works out what goes and says what moved.
    fs::remove_file(v.older_folder()).unwrap();
    let again = versions::retry_put_away_impl(&state, &v.new.0).unwrap();
    assert_eq!(again.put_away_failed, None);
    assert_eq!(again.put_away.len(), 1);
    assert_eq!(again.put_away[0].file_name, OLD);
    assert!(!v.in_mods(OLD) && v.older_folder().join(OLD).exists());
    assert_eq!(
        v.installed(&state),
        [v.option(&v.new)],
        "the install stands"
    );
    state.wait_until_scanned();

    // Nothing older is left: a third press moves nothing and names nothing.
    let again = versions::retry_put_away_impl(&state, &v.new.0).unwrap();
    assert!(again.put_away.is_empty() && again.put_away_failed.is_none());
}

#[test]
fn trying_a_put_away_again_never_moves_an_older_version_that_still_has_an_install() {
    let v = versions_bench(true);
    let state = v.open();
    // The older version is installed on purpose; the newer one is the lead.
    deploy::install_impl(&state, &v.option(&v.old)).unwrap();
    let again = versions::retry_put_away_impl(&state, &v.new.0).unwrap();
    assert!(again.put_away.is_empty() && again.put_away_failed.is_none());
    assert!(v.in_mods(OLD) && v.in_mods(NEW));
    assert!(matches!(
        versions::retry_put_away_impl(&state, "no such package"),
        Err(RiftError::EntryNotFound { .. })
    ));
}

/// The investor's state from before v0.5: the newer version installed over the older one, both
/// in the journal. With no family known (no links yet), v0.4's install stacked them.
fn stacked() -> (Versions, Arc<AppState>) {
    let v = versions_bench(false);
    {
        let state = v.open();
        deploy::install_impl(&state, &v.option(&v.old)).unwrap();
        deploy::install_impl(&state, &v.option(&v.new)).unwrap();
        assert_eq!(v.installed(&state).len(), 2, "stacked, as v0.4 left them");
    }
    both_linked(&v);
    let state = v.open();
    (v, state)
}

#[test]
fn keep_only_the_newest_uninstalls_the_older_install_and_puts_it_away() {
    let (v, state) = stacked();
    let view = library::get_library_impl(&state);
    let lead = view
        .packages
        .iter()
        .find(|p| p.package.id == v.new.0)
        .unwrap();
    assert_eq!(lead.family.as_ref().unwrap().older[0].installed, 1);

    // Any member names the family.
    let plan = versions::plan_keep_newest_impl(&state, &v.old.0).unwrap();
    assert_eq!(plan.refused, None);
    assert_eq!(plan.package_id, v.new.0, "the family's lead");
    assert_eq!(plan.keeps.as_deref(), Some(NEW));
    assert_eq!(plan.uninstalls.len(), 1);
    assert_eq!(plan.uninstalls[0].option_id, v.option(&v.old));
    assert_eq!(plan.puts_away, [OLD]);

    let done = versions::keep_newest_impl(&state, &v.new.0).unwrap();
    assert_eq!(done.uninstalled, 1);
    assert_eq!(done.put_away_failed, None);
    assert_eq!(done.put_away[0].file_name, OLD);
    assert_eq!(v.installed(&state), [v.option(&v.new)]);
    assert_eq!(v.game_file(EMBER).unwrap(), b"ember 1.1");
    assert_eq!(v.game_file(SMOKE), None);
    assert!(!v.in_mods(OLD) && v.older_folder().join(OLD).exists());

    // The older install handed the game's own ember to the newer one.
    deploy::uninstall_impl(&state, &v.option(&v.new)).unwrap();
    assert_eq!(v.game_file(EMBER).unwrap(), b"the game's own ember");
    state.wait_until_scanned();
    let plan = versions::plan_keep_newest_impl(&state, &v.new.0).unwrap();
    assert!(plan.refused.is_some(), "alone now: {plan:?}");
}

#[test]
fn keep_only_the_newest_refuses_when_nothing_is_older_than_the_version_kept() {
    let v = versions_bench(true);
    let state = v.open();
    // The older version is the one installed and the newer one is not: nothing is older.
    deploy::install_impl(&state, &v.option(&v.old)).unwrap();
    fs::remove_dir_all(v.older_folder()).ok();
    let plan = versions::plan_keep_newest_impl(&state, &v.new.0).unwrap();
    assert_eq!(plan.keeps.as_deref(), Some(OLD));
    let refused = plan.refused.expect("nothing to do");
    assert!(refused.contains(OLD), "{refused}");
    assert!(matches!(
        versions::keep_newest_impl(&state, &v.new.0),
        Err(RiftError::Refused { .. })
    ));
    assert_eq!(v.installed(&state), [v.option(&v.old)]);
    assert!(v.in_mods(OLD) && v.in_mods(NEW));
}

#[test]
fn put_away_moves_only_an_older_version_with_no_install() {
    let v = versions_bench(true);
    let state = v.open();
    let refused = |id: &str| match versions::put_away_impl(&state, id) {
        Err(RiftError::Refused { reason }) => reason,
        other => panic!("{other:?}"),
    };
    assert!(refused(&v.new.0).contains("newest version"));
    let alone = PackageId::for_archive(
        "Lantern Glow Recolor.zip",
        fs::metadata(v.bench.mods.join("Lantern Glow Recolor.zip"))
            .unwrap()
            .len(),
    );
    assert!(refused(&alone.0).contains("only version"));
    deploy::install_impl(&state, &v.option(&v.old)).unwrap();
    // The install of 1.0 above replaced nothing and put nothing away; 1.1 is not installed.
    assert!(refused(&v.old.0).contains("still has an install"));
    deploy::uninstall_impl(&state, &v.option(&v.old)).unwrap();

    // Another file already holds its name there: the next free name, and the record follows.
    fs::create_dir_all(v.older_folder()).unwrap();
    fs::write(v.older_folder().join(OLD), b"an unrelated archive").unwrap();
    let moved = versions::put_away_impl(&state, &v.old.0).unwrap();
    assert_eq!(moved.file_name, OLD);
    assert_eq!(moved.landed_as, "Hearth Embers 1.0 (2).zip");
    assert_eq!(
        fs::read(v.older_folder().join(OLD)).unwrap(),
        b"an unrelated archive",
        "never over an existing file"
    );
    state.wait_until_scanned();
    let shelf = versions::get_older_versions_impl(&state).unwrap();
    let names: Vec<(&str, Option<&str>)> = shelf
        .items
        .iter()
        .map(|item| (item.file_name.as_str(), item.version.as_deref()))
        .collect();
    assert_eq!(
        names,
        [("Hearth Embers 1.0 (2).zip", Some("1.0")), (OLD, None)],
        "by name, as a scan lists them; the record followed the version to its new name"
    );

    // Back under the name it has now, the record follows again and the family is whole.
    let back = versions::bring_back_impl(&state, "Hearth Embers 1.0 (2).zip").unwrap();
    assert_eq!(back.landed_as, "Hearth Embers 1.0 (2).zip");
    state.wait_until_scanned();
    let view = library::get_library_impl(&state);
    let lead = view
        .packages
        .iter()
        .find(|p| p.package.id == v.new.0)
        .unwrap();
    assert_eq!(
        lead.family.as_ref().unwrap().older[0].file_name,
        "Hearth Embers 1.0 (2).zip"
    );
}

#[test]
fn no_link_no_family_and_an_install_stacks_as_before() {
    let v = versions_bench(false);
    let state = v.open();
    let view = library::get_library_impl(&state);
    assert!(view.packages.iter().all(|p| p.family.is_none()));
    deploy::install_impl(&state, &v.option(&v.old)).unwrap();
    let plan = deploy::plan_install_impl(&state, &v.option(&v.new)).unwrap();
    assert!(plan.replaces.is_empty(), "names never make a family");
    let result = deploy::install_impl(&state, &v.option(&v.new)).unwrap();
    assert!(result.put_away.is_empty());
    assert_eq!(v.installed(&state).len(), 2);
    assert!(v.in_mods(OLD) && !v.older_folder().exists());
    assert!(versions::put_away_impl(&state, &v.old.0).is_err());
    assert!(!versions::get_older_versions_impl(&state).unwrap().exists);
    assert!(
        matches!(
            versions::open_older_versions_folder_impl(&state),
            Err(RiftError::Refused { .. })
        ),
        "nothing to show before the first put-away"
    );
}

fn search(state: &AppState, text: &str) -> Vec<String> {
    library::search_impl(
        state,
        &riftstone_lib::dto::SearchQuery {
            text: text.into(),
            filters: riftstone_lib::dto::SearchFilters::default(),
            limit: 100,
        },
    )
    .into_iter()
    .map(|hit| hit.option_id)
    .collect()
}

#[test]
fn an_older_versions_answer_goes_when_the_leads_same_option_answers_too() {
    let v = versions_bench(true);
    let state = v.open();
    let (old, new) = (v.option(&v.old), v.option(&v.new));
    assert_eq!(
        search(&state, "hearth embers"),
        std::slice::from_ref(&new),
        "one entry, the lead's"
    );
    assert_eq!(
        search(&state, "ember.tex"),
        [new],
        "both hold the file: the lead's answer"
    );
    assert_eq!(
        search(&state, "smoke"),
        [old],
        "a file only the older version holds is still found"
    );

    // The lead's family names which older option is the same option as which of its own.
    let view = library::get_library_impl(&state);
    let lead = view
        .packages
        .iter()
        .find(|p| p.package.id == v.new.0)
        .unwrap();
    let matches = &lead.family.as_ref().unwrap().older[0].matches;
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].option_id, v.option(&v.old));
    assert_eq!(matches[0].lead_option_id, v.option(&v.new));
    assert!(!matches[0].installed);
}

#[test]
fn two_versions_stacked_in_your_world_are_an_older_version_installed_not_a_conflict() {
    let v = versions_bench(false);
    {
        let state = v.open();
        deploy::install_impl(&state, &v.option(&v.old)).unwrap();
        deploy::install_impl(&state, &v.option(&v.new)).unwrap();
        let world = deploy::get_installed_impl(&state).unwrap();
        assert_eq!(
            world.conflicts.len(),
            1,
            "unproven, they are two mods on one file"
        );
    }
    both_linked(&v);
    let state = v.open();
    let world = deploy::get_installed_impl(&state).unwrap();
    assert!(world.conflicts.is_empty(), "{:?}", world.conflicts);
    let view = library::get_library_impl(&state);
    let lead = view
        .packages
        .iter()
        .find(|p| p.package.id == v.new.0)
        .unwrap();
    assert!(!lead.conflicted);
    let family = lead.family.as_ref().unwrap();
    assert_eq!(family.older[0].installed, 1, "the family tells it instead");
    assert!(family.older[0].matches[0].installed);
}

#[test]
fn a_download_an_older_versions_check_named_is_current_even_after_that_version_is_put_away() {
    let v = versions_bench(true);
    // The older version's last check named 1.1's very file as its newer one; 1.1, just
    // downloaded, has no verdict of its own.
    let file = v.dirs.nexus_file();
    let mut store: serde_json::Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    store["packages"][&v.old.0]["state"] = json!({
        "kind": "updateAvailable",
        "newer": { "fileId": 3002, "name": "Hearth Embers", "version": "1.1", "uploadedMs": 2_000 }
    });
    store["packages"][&v.old.0]["stateModId"] = json!(MOD);
    store["packages"][&v.old.0]["checkedMs"] = json!(1_792_000_000_000_u64);
    fs::write(&file, serde_json::to_vec(&store).unwrap()).unwrap();
    let state = v.open();
    let lead_row = |state: &AppState| {
        nexus::get_updates_impl(state)
            .rows
            .into_iter()
            .find(|row| row.package_id == v.new.0)
            .unwrap()
    };
    let row = lead_row(&state);
    assert_eq!(row.state, riftstone_lib::dto::UpdateStateView::Current);
    assert_eq!(row.checked_ms, Some(1_792_000_000_000));

    deploy::install_impl(&state, &v.option(&v.new)).unwrap();
    state.wait_until_scanned();
    assert!(!v.in_mods(OLD), "put away");
    let row = lead_row(&state);
    assert_eq!(
        row.state,
        riftstone_lib::dto::UpdateStateView::Current,
        "the record the older version left in nexus.json still vouches"
    );
    assert!(row.older.is_empty());
    let hits = library::search_impl(
        &state,
        &riftstone_lib::dto::SearchQuery {
            text: "hearth".into(),
            filters: riftstone_lib::dto::SearchFilters::default(),
            limit: 10,
        },
    );
    assert_eq!(
        hits[0].update,
        Some(riftstone_lib::dto::UpdateBadge::Current),
        "the search card agrees"
    );
}
