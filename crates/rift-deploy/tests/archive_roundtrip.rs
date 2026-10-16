//! The one round trip through a real package reader: a zip built at run
//! time, opened with `rift_archive::open`, installed by the engine and
//! uninstalled again.

mod common;

use std::fs;
use std::io::Write;

use common::{assert_no_temporaries, snapshot, Bench};
use rift_core::{
    DeployTarget, ModOption, OptionId, Package, PackageFormat, PackageId, PayloadFile, PayloadKinds,
};
use rift_deploy::{install, uninstall, Below};

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
