//! § v0.5 rift-deploy: an install that takes the place of other installs (a newer version of the
//! same file replacing the older one) in one intent, against a fake game folder.

mod common;

use std::panic::{catch_unwind, AssertUnwindSafe};

use common::{assert_no_temporaries, Bench, TestMod};
use rift_core::RiftError;
use rift_deploy::{install_replacing, plan_install_replacing, recover, PlanWarning, WriteAction};

fn replace(bench: &Bench, journal: &mut rift_deploy::Journal, new: &TestMod, old: &[&TestMod]) {
    let replacing: Vec<_> = old.iter().map(|m| m.id()).collect();
    install_replacing(
        &bench.ctx(),
        journal,
        &new.package,
        &new.option,
        &replacing,
        &mut new.source(),
        &mut |_| {},
    )
    .unwrap();
}

#[test]
fn a_newer_version_takes_the_older_ones_place_in_one_step() {
    let bench = Bench::new();
    bench.write("natives/stm/a.mesh", b"original a");
    let mut journal = bench.journal();
    let old = TestMod::new("Lantern Glow 1.0")
        .loose("natives/stm/a.mesh", b"glow 1.0")
        .loose("natives/stm/only_old.mesh", b"old only");
    let new = TestMod::new("Lantern Glow 1.1")
        .loose("natives/stm/a.mesh", b"glow 1.1")
        .loose("natives/stm/only_new.mesh", b"new only");
    old.install(&bench, &mut journal);

    let plan = plan_install_replacing(
        &bench.ctx(),
        &journal,
        &new.package,
        &new.option,
        &[old.id()],
    );
    assert_eq!(plan.refused, None);
    let action = |path: &str| {
        plan.writes
            .iter()
            .find(|w| w.game_path == path)
            .map(|w| w.action.clone())
    };
    assert_eq!(
        action("natives/stm/a.mesh"),
        Some(WriteAction::Reinstall),
        "the older version's file goes and the newer comes"
    );
    assert_eq!(
        action("natives/stm/only_new.mesh"),
        Some(WriteAction::Create)
    );
    assert!(
        !plan
            .warnings
            .iter()
            .any(|w| matches!(w, PlanWarning::Conflicts { .. })),
        "a replace is no conflict: {:?}",
        plan.warnings
    );

    replace(&bench, &mut journal, &new, &[&old]);
    assert!(!journal.is_installed(&old.id()) && journal.is_installed(&new.id()));
    assert_eq!(journal.installs.len(), 1, "one version installed, not two");
    assert_eq!(bench.read("natives/stm/a.mesh").unwrap(), b"glow 1.1");
    assert!(
        !bench.exists("natives/stm/only_old.mesh"),
        "what only the older version had leaves with it"
    );
    assert_eq!(
        bench.read("natives/stm/only_new.mesh").unwrap(),
        b"new only"
    );
    let on_disk = bench.journal();
    assert_eq!(on_disk, journal);
    assert!(on_disk.pending.is_none());
    assert_no_temporaries(&bench);

    // The newer version uninstalls to the game's own file, as the older one would have.
    new.uninstall(&bench, &mut journal);
    assert_eq!(bench.read("natives/stm/a.mesh").unwrap(), b"original a");
    assert!(!bench.exists("natives/stm/only_new.mesh"));
    assert!(bench.store_files().is_empty());
}

#[test]
fn a_replace_keeps_the_older_versions_place_in_the_pak_order() {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let base = TestMod::new("Base Armour 1.0").pak("base.pak", b"KPKA base 1.0");
    let patch = TestMod::new("Armour Patch").pak("patch.pak", b"KPKA patch");
    let newer = TestMod::new("Base Armour 1.1").pak("base.pak", b"KPKA base 1.1");
    base.install(&bench, &mut journal);
    patch.install(&bench, &mut journal);

    let plan = plan_install_replacing(
        &bench.ctx(),
        &journal,
        &newer.package,
        &newer.option,
        &[base.id()],
    );
    assert_eq!(
        plan.writes[0].game_path, "pak_mods/zrift-0000-base.pak",
        "the newer version loads where the older one did, under its patch"
    );
    replace(&bench, &mut journal, &newer, &[&base]);
    assert_eq!(
        bench.names_in("pak_mods"),
        ["zrift-0000-base.pak", "zrift-0001-patch.pak"]
    );
    assert_eq!(
        bench.read("pak_mods/zrift-0000-base.pak").unwrap(),
        b"KPKA base 1.1"
    );
    assert_eq!(journal.pak_order, [newer.id(), patch.id()]);
    assert_no_temporaries(&bench);
}

#[test]
fn a_replace_that_fails_puts_the_older_version_back_whole() {
    let bench = Bench::new();
    bench.write("natives/stm/a.mesh", b"original a");
    let mut journal = bench.journal();
    let old = TestMod::new("Lantern Glow 1.0")
        .loose("natives/stm/a.mesh", b"glow 1.0")
        .loose("natives/stm/b.mesh", b"glow b 1.0");
    let new = TestMod::new("Lantern Glow 1.1")
        .loose("natives/stm/a.mesh", b"glow 1.1")
        .loose("natives/stm/b.mesh", b"glow b 1.1");
    old.install(&bench, &mut journal);
    let installed = bench.snapshot();
    let saved = bench.journal();

    let mut source = new.source().failing_at(2);
    let err = install_replacing(
        &bench.ctx(),
        &mut journal,
        &new.package,
        &new.option,
        &[old.id()],
        &mut source,
        &mut |_| {},
    )
    .unwrap_err();
    assert!(matches!(err, RiftError::CorruptArchive { .. }), "{err}");
    assert!(journal.is_installed(&old.id()) && !journal.is_installed(&new.id()));
    assert_eq!(bench.snapshot(), installed, "the older install is whole");
    assert_eq!(bench.journal().installs, saved.installs);
    assert!(bench.journal().pending.is_none());
    old.uninstall(&bench, &mut journal);
    assert_eq!(bench.read("natives/stm/a.mesh").unwrap(), b"original a");
    assert!(bench.store_files().is_empty());
}

#[test]
fn a_replace_cut_short_is_rolled_back_at_the_next_launch() {
    let bench = Bench::new();
    bench.write("natives/stm/a.mesh", b"original a");
    let mut journal = bench.journal();
    let old = TestMod::new("Lantern Glow 1.0").loose("natives/stm/a.mesh", b"glow 1.0");
    let new = TestMod::new("Lantern Glow 1.1").loose("natives/stm/a.mesh", b"glow 1.1");
    old.install(&bench, &mut journal);
    let installed = bench.snapshot();

    let mut source = new.source().panicking_at(1);
    let crashed = catch_unwind(AssertUnwindSafe(|| {
        install_replacing(
            &bench.ctx(),
            &mut journal,
            &new.package,
            &new.option,
            &[old.id()],
            &mut source,
            &mut |_| {},
        )
    }));
    assert!(crashed.is_err(), "the fake source crashed the replace");
    let mut relaunched = bench.journal();
    assert!(
        relaunched.pending.is_some(),
        "the intent was on disk when it crashed"
    );
    recover(&bench.ctx(), &mut relaunched).unwrap();
    assert!(relaunched.pending.is_none());
    assert!(relaunched.is_installed(&old.id()) && !relaunched.is_installed(&new.id()));
    assert_eq!(
        bench.snapshot(),
        installed,
        "the older version is back as it was"
    );
    assert_no_temporaries(&bench);
}

#[test]
fn two_stacked_versions_are_both_replaced_through_their_own_temporaries() {
    // v0.4 stacked a newer version over an older one; replacing both moves one path twice.
    let bench = Bench::new();
    bench.write("natives/stm/a.mesh", b"original a");
    let mut journal = bench.journal();
    let v1 = TestMod::new("Lantern Glow 1.0").loose("natives/stm/a.mesh", b"glow 1.0");
    let v2 = TestMod::new("Lantern Glow 1.1").loose("natives/stm/a.mesh", b"glow 1.1");
    let v3 = TestMod::new("Lantern Glow 1.2").loose("natives/stm/a.mesh", b"glow 1.2");
    v1.install(&bench, &mut journal);
    v2.install(&bench, &mut journal);

    replace(&bench, &mut journal, &v3, &[&v1, &v2]);
    assert_eq!(journal.installs.len(), 1);
    assert!(journal.is_installed(&v3.id()));
    assert_eq!(bench.read("natives/stm/a.mesh").unwrap(), b"glow 1.2");
    assert_no_temporaries(&bench);
    v3.uninstall(&bench, &mut journal);
    assert_eq!(bench.read("natives/stm/a.mesh").unwrap(), b"original a");
    assert!(bench.store_files().is_empty());
}

#[test]
fn a_replace_of_something_not_installed_is_a_plain_install() {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let old = TestMod::new("Lantern Glow 1.0").loose("natives/stm/a.mesh", b"glow 1.0");
    let new = TestMod::new("Lantern Glow 1.1").loose("natives/stm/a.mesh", b"glow 1.1");
    replace(&bench, &mut journal, &new, &[&old]);
    assert!(journal.is_installed(&new.id()));
    assert_eq!(bench.read("natives/stm/a.mesh").unwrap(), b"glow 1.1");
}

#[test]
fn the_top_of_two_stacked_versions_replaced_first_still_moves_through_its_own_names() {
    // Top first: its uninstall restores the lower version's bytes, and the lower version's own
    // uninstall then moves the same path again, through a second temporary.
    let bench = Bench::new();
    bench.write("natives/stm/a.mesh", b"original a");
    let mut journal = bench.journal();
    let v1 = TestMod::new("Lantern Glow 1.0").loose("natives/stm/a.mesh", b"glow 1.0");
    let v2 = TestMod::new("Lantern Glow 1.1").loose("natives/stm/a.mesh", b"glow 1.1");
    let v3 = TestMod::new("Lantern Glow 1.2").loose("natives/stm/a.mesh", b"glow 1.2");
    v1.install(&bench, &mut journal);
    v2.install(&bench, &mut journal);

    replace(&bench, &mut journal, &v3, &[&v2, &v1]);
    assert_eq!(journal.installs.len(), 1);
    assert_eq!(bench.read("natives/stm/a.mesh").unwrap(), b"glow 1.2");
    assert_no_temporaries(&bench);
    v3.uninstall(&bench, &mut journal);
    assert_eq!(bench.read("natives/stm/a.mesh").unwrap(), b"original a");
    assert!(bench.store_files().is_empty());
}
