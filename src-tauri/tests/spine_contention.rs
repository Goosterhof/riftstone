//! The lock rule under load: one command's slow read never makes another
//! command wait, and a burst of searches after a change reads the game folder
//! once.
//!
//! A named pipe stands in for a pak inside a package that takes as long to
//! decompress as the test wants: `get_option` reads its head to preview the
//! option's conflicts and sits in that read until the test feeds the pipe.
//! Meanwhile an install (which must swap its journal in) and a search (which
//! must read the journal) have to finish.

mod support;

#[cfg(unix)]
use std::fs;
use std::sync::Arc;
#[cfg(unix)]
use std::time::Duration;

use riftstone_lib::commands::{deploy, library};
use riftstone_lib::dto::{SearchFilters, SearchQuery};
use riftstone_lib::persist::DataDirs;
use support::{option_named, Bench};
// The named-pipe test runs on unix only (mkfifo); its helpers come with it.
#[cfg(unix)]
use support::{pak, within, ERMINE_MESH};

fn query(text: &str) -> SearchQuery {
    SearchQuery {
        text: text.into(),
        filters: SearchFilters::default(),
        limit: 0,
    }
}

#[cfg(unix)]
#[test]
fn a_slow_archive_read_never_stalls_an_install_or_a_search() {
    let bench = Bench::new();
    let twin = bench.mods.join("Twin Paks");
    let first_pak = pak(&[rift_pak::path_hash("natives/stm/twin/first.tex.251211553")]);
    let second_pak = pak(&[rift_pak::path_hash("natives/stm/twin/second.tex.251211553")]);
    for (folder, name, bytes) in [
        ("First", "Twin First", &first_pak),
        ("Second", "Twin Second", &second_pak),
    ] {
        fs::create_dir_all(twin.join(folder)).unwrap();
        fs::write(
            twin.join(folder).join("modinfo.ini"),
            format!("name={name}\r\n"),
        )
        .unwrap();
        fs::write(twin.join(folder).join(format!("{folder}.pak")), bytes).unwrap();
    }
    let (state, _) = bench.open(bench.config(DataDirs::under(&bench.data_home())));
    assert!(state.scan_blocking());
    let first = option_named(&state, "Twin First");
    let second = option_named(&state, "Twin Second");
    let ermine = option_named(&state, "Ermine Cape");

    // The first preview opens the folder package (and keeps it open).
    library::get_option_impl(&state, &first).unwrap();
    // Then Second.pak turns into a pipe nobody writes to yet: the next
    // preview sits in its read, the way a slow solid archive would. The pipe
    // gets a second name outside the library, to feed it through later.
    let pak_path = twin.join("Second/Second.pak");
    let door = bench.dir.path().join("second-pak.pipe");
    fs::remove_file(&pak_path).unwrap();
    support::mkfifo(&pak_path);
    fs::hard_link(&pak_path, &door).unwrap();
    let preview = {
        let state = Arc::clone(&state);
        std::thread::spawn(move || library::get_option_impl(&state, &second))
    };
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        !preview.is_finished(),
        "the preview is stuck in its read, as staged"
    );

    let installed = {
        let state = Arc::clone(&state);
        let ermine = ermine.clone();
        within(Duration::from_secs(10), move || {
            deploy::install_impl(&state, &ermine).map(|r| r.files_written)
        })
    };
    let searched = {
        let state = Arc::clone(&state);
        within(Duration::from_secs(10), move || {
            library::search_impl(&state, &query("ermine"))
                .first()
                .and_then(|hit| hit.installed.clone())
                .is_some()
        })
    };
    // Whatever happened, let the preview go before judging: the pak is a
    // plain file again for any later read, and the read stuck in the pipe
    // gets the same bytes through its other name.
    let plain = bench.dir.path().join("second-pak.plain");
    fs::write(&plain, &second_pak).unwrap();
    fs::rename(&plain, &pak_path).unwrap();
    support::feed_fifo(&door, &second_pak);
    let detail = within(Duration::from_secs(10), move || preview.join())
        .expect("the preview finishes once fed")
        .expect("the preview did not panic");

    assert_eq!(
        installed.map(|r| r.ok()),
        Some(Some(1)),
        "the install finished while the preview read its package"
    );
    assert_eq!(
        searched,
        Some(true),
        "the search finished meanwhile, and saw the new install"
    );
    assert_eq!(detail.unwrap().option.name, "Twin Second");
    assert_eq!(
        fs::read(bench.game.join(ERMINE_MESH)).unwrap(),
        b"ermine cape mesh bytes"
    );
}

#[test]
fn searches_between_changes_read_nothing_and_a_burst_after_one_reads_once() {
    let bench = Bench::new();
    let (state, _) = bench.open(bench.config(DataDirs::under(&bench.data_home())));
    assert!(state.scan_blocking());
    library::search_impl(&state, &query("ermine"));
    let built = state.snapshot_builds();
    for _ in 0..5 {
        library::search_impl(&state, &query("lantern"));
    }
    assert_eq!(
        state.snapshot_builds(),
        built,
        "keystrokes between changes never read the game folder"
    );

    let ermine = option_named(&state, "Ermine Cape");
    deploy::install_impl(&state, &ermine).unwrap();
    let searches: Vec<_> = (0..8)
        .map(|_| {
            let state = Arc::clone(&state);
            std::thread::spawn(move || {
                library::search_impl(&state, &query("ermine"))[0]
                    .installed
                    .is_some()
            })
        })
        .collect();
    for search in searches {
        assert!(search.join().unwrap(), "every search sees the install");
    }
    assert_eq!(
        state.snapshot_builds(),
        built + 1,
        "eight searches after one change read the game folder once"
    );
}
