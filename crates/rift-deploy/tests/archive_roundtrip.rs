//! Round trips through a real package reader: a zip built at run time,
//! opened with `rift_archive::open`, installed by the engine and uninstalled
//! again — and one install whose temporary name a rival takes while
//! rift-archive unpacks, which the rollback must leave to the rival.

mod common;

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use common::{assert_no_temporaries, snapshot, Bench};
use rift_core::{
    ArchiveEntry, DeployTarget, ExtractItem, ExtractProgress, ModOption, OptionId, Package,
    PackageFormat, PackageId, PackageSource, PayloadFile, PayloadKinds, RiftError,
};
use rift_deploy::{install, plan_install, uninstall, Below};

#[test]
fn a_real_zip_installs_through_rift_archive_and_uninstalls_clean() {
    let bench = Bench::new();
    let library = tempfile::tempdir().unwrap();
    let zip_path = library.path().join("Lantern Glow Recolor.zip");
    let mesh: Vec<u8> = (0..300_000u32).map(|i| (i % 7) as u8).collect();
    let pak = b"KPKA\x04\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00".to_vec();
    {
        let file = fs::File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.add_directory("Lantern Glow/", options).unwrap();
        zip.start_file("Lantern Glow/modinfo.ini", options).unwrap();
        zip.write_all(b"name=Lantern Glow Recolor\r\n").unwrap();
        zip.start_file(
            "Lantern Glow/natives/STM/effect/lantern.mesh.260421070",
            options,
        )
        .unwrap();
        zip.write_all(&mesh).unwrap();
        zip.start_file("Lantern Glow/x0001-Lantern Glow.pak", options)
            .unwrap();
        zip.write_all(&pak).unwrap();
        zip.finish().unwrap();
    }
    let library_before = snapshot(library.path());
    // An unowned file in the way, to exercise the backup store too.
    bench.write(
        "natives/stm/effect/lantern.mesh.260421070",
        b"what was there",
    );

    let mut source = rift_archive::open(&zip_path).unwrap();
    let entries = source.entries().unwrap();
    let size_of = |path: &str| {
        entries
            .iter()
            .find(|e| e.path == path)
            .unwrap_or_else(|| panic!("{path} listed"))
            .size
    };
    let package_size = fs::metadata(&zip_path).unwrap().len();
    let id = PackageId::for_archive("Lantern Glow Recolor.zip", package_size);
    let payload = vec![
        PayloadFile {
            entry: "Lantern Glow/natives/STM/effect/lantern.mesh.260421070".into(),
            target: DeployTarget::Loose {
                game_path: "natives/STM/effect/lantern.mesh.260421070".into(),
            },
            size: size_of("Lantern Glow/natives/STM/effect/lantern.mesh.260421070"),
        },
        PayloadFile {
            entry: "Lantern Glow/x0001-Lantern Glow.pak".into(),
            target: DeployTarget::Pak {
                file_name: "x0001-Lantern Glow.pak".into(),
            },
            size: size_of("Lantern Glow/x0001-Lantern Glow.pak"),
        },
    ];
    let package = Package {
        id: id.clone(),
        root_id: "custom-bench".into(),
        path: zip_path.clone(),
        file_name: "Lantern Glow Recolor.zip".into(),
        display_name: "Lantern Glow Recolor".into(),
        format: PackageFormat::Zip,
        size: package_size,
        modified_ms: 0,
        entry_count: entries.len(),
        options: Vec::new(),
        nexus: None,
        problems: Vec::new(),
    };
    let option = ModOption {
        id: OptionId::new(&id, "Lantern Glow"),
        package_id: id,
        root: "Lantern Glow".into(),
        name: "Lantern Glow Recolor".into(),
        info: None,
        screenshot: None,
        kinds: PayloadKinds::tally(&payload),
        payload,
        skipped: Vec::new(),
        parent: None,
        addon_for: None,
        bundle: None,
        menu_only: false,
        trail: Vec::new(),
    };

    let mut journal = bench.journal();
    let mut progress_calls = 0usize;
    let report = install(
        &bench.ctx(),
        &mut journal,
        &package,
        &option,
        source.as_mut(),
        &mut |_| progress_calls += 1,
    )
    .unwrap();
    assert_eq!(report.files_written, 2);
    assert_eq!(report.backups, 1);
    assert!(
        progress_calls >= 1,
        "extraction progress reaches the caller"
    );
    assert_eq!(
        bench
            .read("natives/stm/effect/lantern.mesh.260421070")
            .unwrap(),
        mesh
    );
    assert_eq!(
        bench.read("pak_mods/zrift-0000-Lantern Glow.pak").unwrap(),
        pak
    );
    assert!(matches!(
        journal.installs[0].files[0].below,
        Below::Backup { .. }
    ));
    assert_no_temporaries(&bench);
    assert_eq!(
        snapshot(library.path()),
        library_before,
        "RS-2: the library is read-only"
    );

    uninstall(&bench.ctx(), &mut journal, &option.id).unwrap();
    assert_eq!(
        bench
            .read("natives/stm/effect/lantern.mesh.260421070")
            .unwrap(),
        b"what was there"
    );
    assert!(!bench.exists("pak_mods"));
    assert!(bench.store_files().is_empty());
    assert_no_temporaries(&bench);
}

/// What a rival leaves at one of the install's temporary names.
const PLANTED: &[u8] = b"not the Riftstone's";

/// A real package reader with a rival beside it: once `after` files are
/// unpacked, something else creates `plant` — the moment the engine's own
/// check cannot cover (the name was free when it looked).
struct Rival {
    inner: Box<dyn PackageSource>,
    plant: PathBuf,
    after: usize,
}

impl PackageSource for Rival {
    fn format(&self) -> PackageFormat {
        self.inner.format()
    }

    fn entries(&mut self) -> Result<Vec<ArchiveEntry>, RiftError> {
        self.inner.entries()
    }

    fn read_entry(&mut self, entry: &str, limit: Option<u64>) -> Result<Vec<u8>, RiftError> {
        self.inner.read_entry(entry, limit)
    }

    fn extract(
        &mut self,
        items: &[ExtractItem],
        progress: &mut dyn FnMut(ExtractProgress),
    ) -> Result<(), RiftError> {
        let plant = self.plant.clone();
        let after = self.after;
        self.inner.extract(items, &mut |report| {
            progress(report);
            if report.files_done == after {
                fs::write(&plant, PLANTED).unwrap();
            }
        })
    }
}

const CLOAK: &str = "natives/stm/appsystem/cloak";
const NAMES: [&str; 3] = ["ermine_cape", "wool_hood", "silk_mantle"];

/// The invented collection as a real zip in `library`: three loose meshes
/// under one option. Returns the zip, its package and its option.
fn cloaks_collection(library: &std::path::Path) -> (PathBuf, Package, ModOption) {
    let zip_path = library.join("Vernworth Cloaks Collection.zip");
    {
        let file = fs::File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for name in NAMES {
            zip.start_file(format!("Vernworth Cloaks/{CLOAK}/{name}.mesh"), options)
                .unwrap();
            zip.write_all(format!("{name} bytes").as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }
    let size = fs::metadata(&zip_path).unwrap().len();
    let id = PackageId::for_archive("Vernworth Cloaks Collection.zip", size);
    let payload: Vec<PayloadFile> = NAMES
        .iter()
        .map(|name| PayloadFile {
            entry: format!("Vernworth Cloaks/{CLOAK}/{name}.mesh"),
            target: DeployTarget::Loose {
                game_path: format!("{CLOAK}/{name}.mesh"),
            },
            size: format!("{name} bytes").len() as u64,
        })
        .collect();
    let package = Package {
        id: id.clone(),
        root_id: "custom-bench".into(),
        path: zip_path.clone(),
        file_name: "Vernworth Cloaks Collection.zip".into(),
        display_name: "Vernworth Cloaks Collection".into(),
        format: PackageFormat::Zip,
        size,
        modified_ms: 0,
        entry_count: NAMES.len(),
        options: Vec::new(),
        nexus: None,
        problems: Vec::new(),
    };
    let option = ModOption {
        id: OptionId::new(&id, "Vernworth Cloaks"),
        package_id: id,
        root: "Vernworth Cloaks".into(),
        name: "Vernworth Cloaks Collection".into(),
        info: None,
        screenshot: None,
        kinds: PayloadKinds::tally(&payload),
        payload,
        skipped: Vec::new(),
        parent: None,
        addon_for: None,
        bundle: None,
        menu_only: false,
        trail: Vec::new(),
    };
    (zip_path, package, option)
}

/// Whether a refusal names `part` (a game-relative path with `/`). The
/// extractor names the full destination in the platform's own separators, so
/// on Windows the sentence carries `natives\stm\…`: compare separator-blind.
fn names(reason: &str, part: &str) -> bool {
    reason.replace('\\', "/").contains(part)
}

/// Install the collection through `source`.
fn install_from(
    bench: &Bench,
    journal: &mut rift_deploy::Journal,
    (package, option): (&Package, &ModOption),
    source: &mut dyn PackageSource,
) -> Result<rift_deploy::InstallReport, RiftError> {
    install(&bench.ctx(), journal, package, option, source, &mut |_| {})
}

#[test]
fn a_part_name_taken_while_rift_archive_unpacks_is_refused_and_outlives_the_rollback() {
    // The rival takes the second file's name once the first is unpacked, or
    // the third's once two are: the extraction stops there either way.
    for (taken, after) in [(1usize, 1usize), (2, 2)] {
        let bench = Bench::new();
        let library = tempfile::tempdir().unwrap();
        let (zip_path, package, option) = cloaks_collection(library.path());
        // The wool hood replaces a file nobody owns: it stays where it is.
        bench.write(
            &format!("{CLOAK}/wool_hood.mesh"),
            b"the hood that was there",
        );
        let before = bench.snapshot();

        let part = format!("{CLOAK}/{}.mesh.riftstone-part", NAMES[taken]);
        let mut rival = Rival {
            inner: rift_archive::open(&zip_path).unwrap(),
            plant: bench.root().join(&part),
            after,
        };
        let mut journal = bench.journal();
        let err = install_from(&bench, &mut journal, (&package, &option), &mut rival).unwrap_err();
        assert!(
            matches!(&err, RiftError::Refused { reason } if names(reason, &part)),
            "taken {taken}: rift-archive refuses the taken name, naming it: {err:?}"
        );

        // The rollback put back everything the install did — and left the
        // rival's file exactly as the rival left it.
        assert_eq!(
            bench.read(&part).as_deref(),
            Some(PLANTED),
            "taken {taken}: the rival's file is not the install's to delete"
        );
        let mut expected = before.clone();
        expected.insert(part.clone(), Some(PLANTED.to_vec()));
        assert_eq!(bench.snapshot(), expected, "taken {taken}");
        assert!(journal.pending.is_none() && journal.installs.is_empty());
        assert_eq!(bench.journal(), journal, "taken {taken}: on disk too");
        assert!(bench.store_files().is_empty(), "taken {taken}");

        // The name stays taken: the next install says so before it starts…
        let plan = plan_install(&bench.ctx(), &journal, &package, &option);
        assert!(names(&plan.refused.unwrap(), &part), "taken {taken}");
        // …and installs once the rival's file is out of the way.
        fs::remove_file(bench.root().join(&part)).unwrap();
        let mut source = rift_archive::open(&zip_path).unwrap();
        install_from(&bench, &mut journal, (&package, &option), source.as_mut()).unwrap();
        assert_eq!(
            bench.read(&format!("{CLOAK}/wool_hood.mesh")).unwrap(),
            b"wool_hood bytes"
        );
        assert_no_temporaries(&bench);
    }
}

#[test]
fn a_reinstall_whose_part_name_is_taken_puts_the_old_copy_back_and_spares_the_rival() {
    // A reinstall first moves the old copy out of the way, then unpacks the
    // new one. The rival's file at the new copy's temporary name is not the
    // install's, and it must not keep the old copy from coming back either.
    let bench = Bench::new();
    let library = tempfile::tempdir().unwrap();
    let (zip_path, package, option) = cloaks_collection(library.path());
    bench.write(
        &format!("{CLOAK}/wool_hood.mesh"),
        b"the hood that was there",
    );
    let mut journal = bench.journal();
    let mut source = rift_archive::open(&zip_path).unwrap();
    install_from(&bench, &mut journal, (&package, &option), source.as_mut()).unwrap();
    let installed = bench.snapshot();
    let record = journal.clone();
    let store_before = bench.store_files();

    let part = format!("{CLOAK}/wool_hood.mesh.riftstone-part");
    let mut rival = Rival {
        inner: rift_archive::open(&zip_path).unwrap(),
        plant: bench.root().join(&part),
        after: 1,
    };
    let err = install_from(&bench, &mut journal, (&package, &option), &mut rival).unwrap_err();
    assert!(
        matches!(&err, RiftError::Refused { reason } if names(reason, &part)),
        "{err:?}"
    );
    let mut expected = installed.clone();
    expected.insert(part.clone(), Some(PLANTED.to_vec()));
    assert_eq!(
        bench.snapshot(),
        expected,
        "the old copy is back in place, the rival's file untouched"
    );
    assert_eq!(
        journal.installs, record.installs,
        "still installed as it was"
    );
    assert!(journal.pending.is_none());
    assert_eq!(bench.journal(), journal);
    assert_eq!(
        bench.store_files(),
        store_before,
        "the old backup is still kept"
    );
}
