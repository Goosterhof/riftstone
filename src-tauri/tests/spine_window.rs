//! What the window draws from the spine's answers, checked against the window
//! itself: the Preflight page shows every fact once (its own rows and the
//! spine's checks never repeat each other), the troubled list follows the
//! contract's rule, the library tree lists every package of the mods folder
//! (troubled ones too) in the contract's order, and a picture that cannot be
//! shown is not read again on every card.

mod support;

use std::fs;

use riftstone_lib::commands::{deploy, library, overview};
use riftstone_lib::dto::{ProblemKind, ThumbnailSize};
use riftstone_lib::persist::DataDirs;
use support::{option_named, write_zip, Bench};

#[test]
fn the_preflight_page_shows_every_fact_once() {
    let bench = Bench::new();
    let dirs = DataDirs::under(&bench.data_home());
    // REFramework with both switches off and one faulty file; Steam build A.
    bench.install_reframework(false, false, &["natives/stm/broken/thing.mesh.260421070"]);
    bench.set_steam_build("25290080");
    let (state, _) = bench.open(bench.config(dirs.clone()));
    assert!(state.scan_blocking());
    // A loose install and a pak install, recorded at build A.
    deploy::install_impl(&state, &option_named(&state, "Ermine Cape")).unwrap();
    deploy::install_impl(&state, &option_named(&state, "Lantern Glow Recolor")).unwrap();
    drop(state);
    // Steam updates the game; the Riftstone opens again.
    bench.set_steam_build("25391144");
    let (state, _) = bench.open(bench.config(dirs));
    let home = overview::get_overview_impl(&state).unwrap();

    // Every fact the page draws a row for is in the overview…
    let game = home.game.as_ref().unwrap();
    assert!(game.build_changed.is_some(), "the build changed");
    let reframework = home.reframework.as_ref().unwrap();
    assert!(reframework.installed);
    assert_eq!(reframework.pak_directory, Some(false));
    assert_eq!(reframework.loose_file_loader, Some(false));
    assert_eq!(reframework.faulty_files.len(), 1);

    // …and none of them comes back as a check the page would draw again.
    let own = Bench::preflight_own_rows();
    let rows: Vec<&str> = home
        .checks
        .iter()
        .map(|check| check.id.as_str())
        .filter(|id| !own.iter().any(|own| own == id))
        .collect();
    for drawn in [
        "buildChanged",
        "pakDirectory",
        "looseFileLoader",
        "faultyFiles",
        "reframework",
        "game",
    ] {
        assert!(
            !rows.contains(&drawn),
            "the page draws {drawn} itself, yet the overview's checks show it again: {rows:?}"
        );
    }
    // The page's own rows still carry their fixes: the switch checks cross
    // under the page's ids, with their actions.
    let pakdir = home.checks.iter().find(|c| c.id == "pakdir").unwrap();
    assert_eq!(
        serde_json::to_value(pakdir.action.as_ref().unwrap()).unwrap(),
        serde_json::json!({ "kind": "enableSwitch", "switch": "pakDirectory" })
    );
    // A check with no row of its own comes through as before.
    assert!(rows.contains(&"paksLoaded"), "{rows:?}");
}

#[test]
fn the_troubled_list_ranks_every_package_by_the_first_trouble_it_carries() {
    let bench = Bench::new();
    let old_mesh = "natives/stm/appsystem/cloak/grey_cloak.mesh.240423143";
    let new_mesh = "natives/stm/appsystem/cloak/grey_cloak_hood.mesh.260421070";
    // Opens and installs; one file sits in a folder the game never reads.
    write_zip(
        &bench.mods.join("Cloak Extras.zip"),
        &[
            (&format!("Cloak Extras/{new_mesh}"), b"hood"),
            ("Cloak Extras/sources/cloak.blend", b"blender scene"),
        ],
    );
    // One outdated file beside one that loads.
    write_zip(
        &bench.mods.join("Mixed Cloak.zip"),
        &[
            (&format!("Mixed Cloak/{old_mesh}"), b"old"),
            (&format!("Mixed Cloak/{new_mesh}"), b"new"),
        ],
    );
    // Everything it installs is in a pre-TU 3.2 format.
    write_zip(
        &bench.mods.join("Old Cloak.zip"),
        &[(&format!("Old Cloak/{old_mesh}"), b"old")],
    );
    // Opens, installs nothing.
    write_zip(
        &bench.mods.join("Readme Only.zip"),
        &[("Readme Only/README.txt", b"Thanks for downloading!")],
    );
    // Does not open at all.
    fs::write(
        bench.mods.join("Torn Download.zip"),
        b"PK\x03\x04 the rest never came",
    )
    .unwrap();

    let (state, _) = bench.open(bench.config(DataDirs::under(&bench.data_home())));
    assert!(state.scan_blocking());
    let troubled = overview::get_troubled_packages_impl(&state);
    let names: Vec<&str> = troubled.iter().map(|p| p.file_name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "Torn Download.zip",
            "Readme Only.zip",
            "Mixed Cloak.zip",
            "Old Cloak.zip",
            "Cloak Extras.zip"
        ],
        "every package with a trouble kind, by the first it carries, then library order"
    );
    for summary in &troubled {
        let catalog: Vec<ProblemKind> = state
            .shelf()
            .packages()
            .iter()
            .find(|p| p.id.0 == summary.id)
            .unwrap()
            .problems
            .iter()
            .map(|p| ProblemKind::from(p.kind))
            .collect();
        let listed: Vec<ProblemKind> = summary.problems.iter().map(|p| p.kind).collect();
        assert_eq!(
            listed, catalog,
            "{}: the catalog's problems, in its order",
            summary.file_name
        );
    }
}

#[test]
fn a_picture_that_cannot_be_shown_is_not_read_again() {
    let bench = Bench::new();
    let folder = bench.mods.join("Grey Cloak");
    fs::create_dir_all(folder.join("natives/stm/appsystem/cloak")).unwrap();
    fs::write(
        folder.join("modinfo.ini"),
        b"name=Grey Cloak\r\nscreenshot=screenshot.png\r\n",
    )
    .unwrap();
    fs::write(folder.join("screenshot.png"), b"this is no picture").unwrap();
    fs::write(
        folder.join("natives/stm/appsystem/cloak/grey_cloak.mesh.260421070"),
        b"mesh",
    )
    .unwrap();
    let dirs = DataDirs::under(&bench.data_home());
    let (state, _) = bench.open(bench.config(dirs.clone()));
    assert!(state.scan_blocking());
    let cloak = option_named(&state, "Grey Cloak");

    assert_eq!(
        library::get_thumbnail_impl(&state, &cloak, ThumbnailSize::Card).unwrap(),
        None
    );
    let markers: Vec<String> = fs::read_dir(dirs.thumbs_dir())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(markers.len(), 1, "{markers:?}");
    assert!(markers[0].ends_with(".fail"), "{markers:?}");

    // The picture is gone from disk now: a second read would fail loudly.
    fs::remove_file(folder.join("screenshot.png")).unwrap();
    assert_eq!(
        library::get_thumbnail_impl(&state, &cloak, ThumbnailSize::Card).unwrap(),
        None,
        "the remembered failure answers; the package is not read again"
    );
    assert_eq!(
        library::get_thumbnail_impl(&state, &cloak, ThumbnailSize::Full).unwrap(),
        None,
        "a picture that cannot be shown fails at every size"
    );
}

#[test]
fn the_library_tree_lists_every_package_by_display_name_then_file_name() {
    let bench = Bench::new();
    let new_mesh = "natives/stm/appsystem/cloak/grey_cloak_hood.mesh.260421070";
    // Two packages that read the same: a zip and the folder it was unpacked to.
    write_zip(
        &bench.mods.join("Cloak Extras.zip"),
        &[(&format!("Cloak Extras/{new_mesh}"), b"hood")],
    );
    let unpacked = bench.mods.join("Cloak Extras").join(new_mesh);
    fs::create_dir_all(unpacked.parent().unwrap()).unwrap();
    fs::write(&unpacked, b"hood").unwrap();
    // Troubled ones: one that opens and installs nothing, one that does not open.
    write_zip(
        &bench.mods.join("Readme Only.zip"),
        &[("Readme Only/README.txt", b"Thanks for downloading!")],
    );
    fs::write(
        bench.mods.join("Torn Download.zip"),
        b"PK\x03\x04 the rest never came",
    )
    .unwrap();
    let (state, _) = bench.open(bench.config(DataDirs::under(&bench.data_home())));
    assert!(state.scan_blocking());

    let view = library::get_library_impl(&state);
    let order: Vec<(&str, &str)> = view
        .packages
        .iter()
        .map(|p| {
            (
                p.package.display_name.as_str(),
                p.package.file_name.as_str(),
            )
        })
        .collect();
    assert_eq!(
        order,
        vec![
            ("Cloak Extras", "Cloak Extras"),
            ("Cloak Extras", "Cloak Extras.zip"),
            ("Lantern Glow Recolor", "Lantern Glow Recolor.zip"),
            ("Pawn Banter", support::BANTER_ZIP),
            ("Readme Only", "Readme Only.zip"),
            ("Torn Download", "Torn Download.zip"),
            (
                "Vernworth Cloaks Collection",
                "Vernworth Cloaks Collection.zip"
            ),
        ],
        "every package of the mods folder, troubled ones too"
    );
    for troubled in &view.packages[4..6] {
        assert!(troubled.tree.is_empty(), "{}", troubled.package.file_name);
        assert_eq!(troubled.installable, 0);
        assert!(!troubled.package.problems.is_empty());
    }
    let readme = &view.packages[4];
    assert_eq!(
        readme.package.problems[0].kind,
        ProblemKind::NothingToInstall
    );
    assert_eq!(
        view.packages[5].package.problems[0].kind,
        ProblemKind::Unreadable
    );
    assert_eq!(
        view.packages.len(),
        overview::get_overview_impl(&state)
            .unwrap()
            .library
            .packages
    );
}
