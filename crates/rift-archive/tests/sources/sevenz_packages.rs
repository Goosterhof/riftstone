use std::fs;

use rift_archive::SevenZipSource;
use rift_core::{PackageFormat, PackageSource, RiftError};

use crate::fixtures::{
    compressible, corrupt_first_pack_tail, end_first_pack_stream_at_once, extract_to, modinfo,
    paths, sevenz_at, S,
};

#[test]
fn folders_blocks_and_empty_files_list_as_plain_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Vernworth Cloaks Collection.7z");
    let info = modinfo("Ermine Cape");
    sevenz_at(
        &path,
        vec![
            S::Dir("Vernworth Cloaks Collection"),
            S::Solid(vec![
                ("Vernworth Cloaks Collection/Cloaks/modinfo.ini", &info),
                ("../evil.dll", b"MZ"),
                (
                    r"Vernworth Cloaks Collection\Cloaks\Ermine Cape\natives\stm\cape.mesh",
                    b"cape",
                ),
            ]),
            S::Single("Vernworth Cloaks Collection/readme.txt", b"read me"),
            S::Empty("Vernworth Cloaks Collection/Cloaks/Ermine Cape/empty.user"),
        ],
    );
    let mut source = SevenZipSource::open(&path).unwrap();
    assert_eq!(source.format(), PackageFormat::SevenZip);
    assert_eq!(
        paths(&mut source),
        [
            "Vernworth Cloaks Collection/Cloaks/modinfo.ini",
            "Vernworth Cloaks Collection/Cloaks/Ermine Cape/natives/stm/cape.mesh",
            "Vernworth Cloaks Collection/readme.txt",
            "Vernworth Cloaks Collection/Cloaks/Ermine Cape/empty.user",
        ]
    );
    // The dropped `../evil.dll` sits between two files of the solid block:
    // the decoder must drain it so the next file starts in the right place.
    assert_eq!(
        source
            .read_entry(
                "vernworth cloaks collection/cloaks/ermine cape/natives/STM/CAPE.MESH",
                None
            )
            .unwrap(),
        b"cape"
    );
    assert_eq!(
        source
            .read_entry("Vernworth Cloaks Collection/readme.txt", None)
            .unwrap(),
        b"read me"
    );
    assert!(source
        .read_entry(
            "Vernworth Cloaks Collection/Cloaks/Ermine Cape/empty.user",
            None
        )
        .unwrap()
        .is_empty());
    assert!(matches!(
        source.read_entry("../evil.dll", None),
        Err(RiftError::EntryNotFound { .. })
    ));
}

#[test]
fn read_entry_reads_all_or_exactly_the_limit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Lantern Glow Recolor.7z");
    let big = compressible(768 * 1024, 5);
    sevenz_at(
        &path,
        vec![S::Solid(vec![
            ("Lantern Glow/modinfo.ini", b"name=Lantern Glow"),
            ("Lantern Glow/Lantern Glow.pak", &big),
            ("Lantern Glow/screenshot.png", b"\x89PNG"),
        ])],
    );
    let mut source = SevenZipSource::open(&path).unwrap();
    let pak = "Lantern Glow/Lantern Glow.pak";
    assert_eq!(source.read_entry(pak, Some(16)).unwrap(), big[..16]);
    assert_eq!(
        source.read_entry(pak, Some(16 + 48 * 177)).unwrap().len(),
        16 + 48 * 177
    );
    assert_eq!(source.read_entry(pak, None).unwrap(), big);
    assert!(source.read_entry(pak, Some(0)).unwrap().is_empty());
    assert_eq!(
        source
            .read_entry("Lantern Glow/screenshot.png", Some(2))
            .unwrap(),
        b"\x89P"
    );
    assert_eq!(
        source
            .read_entry("Lantern Glow/screenshot.png", None)
            .unwrap(),
        b"\x89PNG"
    );
}

#[test]
fn the_limit_stops_the_block_decoder_long_before_a_damaged_tail() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Lantern Glow Recolor.7z");
    let big = compressible(1 << 20, 13);
    sevenz_at(
        &path,
        vec![S::Solid(vec![("Lantern Glow/Lantern Glow.pak", &big)])],
    );
    corrupt_first_pack_tail(&path, 4096, 2048);

    let mut source = SevenZipSource::open(&path).unwrap();
    let head = source
        .read_entry("Lantern Glow/Lantern Glow.pak", Some(64 * 1024))
        .unwrap();
    assert_eq!(head, big[..64 * 1024]);
    let whole = source.read_entry("Lantern Glow/Lantern Glow.pak", None);
    assert!(
        matches!(&whole, Err(RiftError::CorruptArchive { path: p, .. }) if p.ends_with("Lantern Glow Recolor.7z")),
        "{whole:?}"
    );
}

#[test]
fn extract_makes_one_pass_per_block_in_archive_order() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Vernworth Cloaks Collection.7z");
    let files: Vec<(String, Vec<u8>)> = (0..5)
        .map(|n| {
            (
                format!("Cloaks/Cape {n}/natives/stm/cape.mesh"),
                compressible(50_000 + n, n as u32),
            )
        })
        .collect();
    let second: Vec<(String, Vec<u8>)> = (0..2)
        .map(|n| {
            (
                format!("Hoods/Hood {n}/natives/stm/hood.mesh"),
                compressible(20_000, 90 + n),
            )
        })
        .collect();
    sevenz_at(
        &path,
        vec![
            S::Solid(
                files
                    .iter()
                    .map(|(n, b)| (n.as_str(), b.as_slice()))
                    .collect(),
            ),
            S::Solid(
                second
                    .iter()
                    .map(|(n, b)| (n.as_str(), b.as_slice()))
                    .collect(),
            ),
            S::Empty("Hoods/empty.user"),
        ],
    );
    let mut source = SevenZipSource::open(&path).unwrap();
    // Asked for in reverse, across both blocks, plus the empty file.
    let wanted = [
        second[1].0.as_str(),
        files[4].0.as_str(),
        "Hoods/empty.user",
        files[1].0.as_str(),
        second[0].0.as_str(),
    ];
    let (result, extracted, dests) = extract_to(&mut source, dir.path(), &wanted);
    result.unwrap();
    assert_eq!(
        source.decode_passes(),
        2,
        "one decode per block, not per file"
    );
    assert_eq!(fs::read(&dests[0]).unwrap(), second[1].1);
    assert_eq!(fs::read(&dests[1]).unwrap(), files[4].1);
    assert!(fs::read(&dests[2]).unwrap().is_empty());
    assert_eq!(fs::read(&dests[3]).unwrap(), files[1].1);
    assert_eq!(fs::read(&dests[4]).unwrap(), second[0].1);
    let done: Vec<usize> = extracted.progress.iter().map(|p| p.files_done).collect();
    assert_eq!(done, [1, 2, 3, 4, 5]);
    let last = extracted.progress.last().unwrap();
    assert_eq!(last.bytes_done, last.bytes_total);
}

#[test]
fn the_same_entry_twice_is_written_twice_in_one_pass() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Lantern Glow Recolor.7z");
    sevenz_at(
        &path,
        vec![S::Solid(vec![
            ("Lantern Glow/a.tex", b"aaaa"),
            ("Lantern Glow/b.tex", b"bbbb"),
        ])],
    );
    let mut source = SevenZipSource::open(&path).unwrap();
    let (result, extracted, dests) = extract_to(
        &mut source,
        dir.path(),
        &["Lantern Glow/b.tex", "lantern glow/B.TEX"],
    );
    result.unwrap();
    assert_eq!(fs::read(&dests[0]).unwrap(), b"bbbb");
    assert_eq!(fs::read(&dests[1]).unwrap(), b"bbbb");
    assert_eq!(extracted.progress.len(), 2);
    assert_eq!(source.decode_passes(), 1);
}

#[test]
fn reading_every_modinfo_of_a_solid_collection_costs_one_pass() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Vernworth Cloaks Collection.7z");
    let options: Vec<(String, Vec<u8>, String, Vec<u8>)> = (0..24)
        .map(|n| {
            (
                format!("Cloaks/Cape {n}/modinfo.ini"),
                modinfo(&format!("Cape {n}")),
                format!("Cloaks/Cape {n}/natives/stm/cape.mesh"),
                compressible(12_000, n),
            )
        })
        .collect();
    let mut block = Vec::new();
    for (info_name, info, mesh_name, mesh) in &options {
        block.push((info_name.as_str(), info.as_slice()));
        block.push((mesh_name.as_str(), mesh.as_slice()));
    }
    sevenz_at(&path, vec![S::Solid(block)]);

    let mut source = SevenZipSource::open(&path).unwrap();
    for (info_name, info, _, _) in &options {
        assert_eq!(
            &source.read_entry(info_name, Some(64 * 1024)).unwrap(),
            info
        );
    }
    assert_eq!(
        source.decode_passes(),
        1,
        "the first modinfo read brought its 23 siblings along"
    );
    // A mesh is not small-and-same-named: it gets its own pass.
    assert_eq!(
        source.read_entry(&options[3].2, None).unwrap(),
        options[3].3
    );
    assert_eq!(source.decode_passes(), 2);
}

/// One solid block (`Ermine Cape/modinfo.ini`, `Silk Cape/modinfo.ini`,
/// Ermine Cape's 1 MiB mesh, then `Wool Cape/modinfo.ini`) whose packed tail
/// is damaged: the decoder breaks inside the mesh, after two modinfos and
/// before the third. Returns the first two.
fn damaged_after_two_modinfos(path: &std::path::Path) -> (Vec<u8>, Vec<u8>) {
    let ermine = modinfo("Ermine Cape");
    let silk = modinfo("Silk Cape");
    let wool = modinfo("Wool Cape");
    let mesh = compressible(1 << 20, 29);
    sevenz_at(
        path,
        vec![S::Solid(vec![
            ("Cloaks/Ermine Cape/modinfo.ini", &ermine),
            ("Cloaks/Silk Cape/modinfo.ini", &silk),
            ("Cloaks/Ermine Cape/natives/stm/cape.mesh", &mesh),
            ("Cloaks/Wool Cape/modinfo.ini", &wool),
        ])],
    );
    corrupt_first_pack_tail(path, 4096, 2048);
    (ermine, silk)
}

#[test]
fn damage_behind_a_small_entry_never_costs_it_its_bytes() {
    // The sibling pass that reads Ermine Cape's modinfo goes on towards Wool
    // Cape's and breaks in the mesh before it. That is the sibling's loss:
    // the entry asked for is returned, and Silk Cape's modinfo, read whole
    // before the break, is kept.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Vernworth Cloaks Collection.7z");
    let (ermine, silk) = damaged_after_two_modinfos(&path);
    for limit in [None, Some(64 * 1024)] {
        let mut source = SevenZipSource::open(&path).unwrap();
        assert_eq!(
            source
                .read_entry("Cloaks/Ermine Cape/modinfo.ini", limit)
                .unwrap(),
            ermine,
            "limit {limit:?}"
        );
        assert_eq!(source.decode_passes(), 1);
        assert_eq!(
            source
                .read_entry("cloaks/silk cape/MODINFO.INI", limit)
                .unwrap(),
            silk
        );
        assert_eq!(
            source.decode_passes(),
            1,
            "served from what the broken pass kept"
        );
        // The control: the entry behind the damage cannot be reached.
        let behind = source.read_entry("Cloaks/Wool Cape/modinfo.ini", limit);
        assert!(
            matches!(&behind, Err(RiftError::CorruptArchive { .. })),
            "{behind:?}"
        );
        assert_eq!(source.decode_passes(), 2);
    }
}

#[test]
fn a_decode_failure_names_the_entry_it_broke_in() {
    // The decoder breaks inside the mesh. Reading the modinfo behind it must
    // say so — naming the mesh, and the entry asked for — never blame the
    // block's first file, which is intact.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Vernworth Cloaks Collection.7z");
    damaged_after_two_modinfos(&path);
    let mut source = SevenZipSource::open(&path).unwrap();
    let err = source
        .read_entry("Cloaks/Wool Cape/modinfo.ini", None)
        .unwrap_err();
    let RiftError::CorruptArchive { path: p, detail } = &err else {
        panic!("{err:?}");
    };
    assert!(p.ends_with("Vernworth Cloaks Collection.7z"), "{err:?}");
    assert!(
        detail.contains("Cloaks/Ermine Cape/natives/stm/cape.mesh"),
        "names where the decoder broke: {detail}"
    );
    assert!(
        detail.contains("Cloaks/Wool Cape/modinfo.ini"),
        "names the entry asked for: {detail}"
    );
    assert!(
        !detail.contains("Ermine Cape/modinfo.ini"),
        "never blames the intact first file: {detail}"
    );
    // The mesh itself, read on its own, names itself.
    let err = source
        .read_entry("Cloaks/Ermine Cape/natives/stm/cape.mesh", None)
        .unwrap_err();
    assert!(
        matches!(&err, RiftError::CorruptArchive { detail, .. }
            if detail.starts_with("Cloaks/Ermine Cape/natives/stm/cape.mesh")),
        "{err:?}"
    );
    // So does an extraction that needs it.
    let (result, extracted, dests) =
        extract_to(&mut source, dir.path(), &["Cloaks/Wool Cape/modinfo.ini"]);
    assert!(
        matches!(&result, Err(RiftError::CorruptArchive { detail, .. })
            if detail.contains("cape.mesh") && !detail.contains("Ermine Cape/modinfo.ini")),
        "{result:?}"
    );
    assert!(extracted.progress.is_empty());
    assert!(!dests[0].exists());
}

#[test]
fn a_stream_that_ends_early_is_cut_off_never_short() {
    // The LZMA2 stream ends before its first byte, although the archive
    // promises every file's full size. No file may come back short or empty
    // as if that were its content, and no extraction may leave one behind.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Lantern Glow Recolor.7z");
    let texture = compressible(40_000, 37);
    sevenz_at(
        &path,
        vec![S::Solid(vec![
            ("Lantern Glow/modinfo.ini", b"name=Lantern Glow"),
            ("Lantern Glow/natives/stm/glow.tex", &texture),
        ])],
    );
    end_first_pack_stream_at_once(&path);
    let mut source = SevenZipSource::open(&path).unwrap();
    for (entry, limit) in [
        ("Lantern Glow/modinfo.ini", None),
        ("Lantern Glow/modinfo.ini", Some(64 * 1024)),
        ("Lantern Glow/natives/stm/glow.tex", None),
        ("Lantern Glow/natives/stm/glow.tex", Some(16)),
    ] {
        let read = source.read_entry(entry, limit);
        assert!(
            matches!(&read, Err(RiftError::CorruptArchive { detail, .. })
                if detail.contains("cut off") && detail.contains("Lantern Glow/modinfo.ini")),
            "{entry} with limit {limit:?}: {read:?}"
        );
    }
    let (result, extracted, dests) = extract_to(
        &mut source,
        dir.path(),
        &["Lantern Glow/natives/stm/glow.tex"],
    );
    assert!(
        matches!(&result, Err(RiftError::CorruptArchive { detail, .. }) if detail.contains("cut off")),
        "{result:?}"
    );
    assert!(extracted.progress.is_empty());
    assert!(!dests[0].exists(), "no truncated file is left behind");
}

#[test]
fn a_batch_with_a_missing_entry_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Lantern Glow Recolor.7z");
    sevenz_at(&path, vec![S::Single("Lantern Glow/a.tex", b"a")]);
    let mut source = SevenZipSource::open(&path).unwrap();
    let (result, extracted, dests) = extract_to(
        &mut source,
        dir.path(),
        &["Lantern Glow/a.tex", "Lantern Glow/b.tex"],
    );
    assert!(matches!(result, Err(RiftError::EntryNotFound { .. })));
    assert!(extracted.progress.is_empty());
    assert!(!dests[0].exists());
    assert_eq!(source.decode_passes(), 0);
}

#[test]
fn a_damaged_7z_says_so_with_its_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Lantern Glow Recolor.7z");
    fs::write(&path, b"7z\xBC\xAF\x27\x1C\x00\x04 not really a 7z").unwrap();
    let err = SevenZipSource::open(&path).unwrap_err();
    assert!(err.to_string().contains("Lantern Glow Recolor.7z"), "{err}");
    assert!(
        matches!(
            err,
            RiftError::CorruptArchive { .. } | RiftError::Unreadable { .. }
        ),
        "{err:?}"
    );
}
