use std::fs;

use rift_archive::ZipSource;
use rift_core::{PackageSource, RiftError};

use crate::fixtures::{
    assert_refused_at, compressible, corrupt_before_central_directory, extract_racing, extract_to,
    mark_zip_entry_encrypted, modinfo, patch_bytes, paths, zip_at, Z,
};

const COLLECTION: &str = "Vernworth Cloaks Collection";

#[test]
fn nested_folders_stored_dirs_and_backslashes_list_as_plain_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Vernworth Cloaks Collection.zip");
    let deep = format!("{COLLECTION}/{}deep.user", "level/".repeat(16));
    let info = modinfo("Ermine Cape");
    zip_at(
        &path,
        &[
            Z::Dir(COLLECTION),
            Z::Dir("Vernworth Cloaks Collection/Cloaks"),
            Z::Deflated("Vernworth Cloaks Collection/Cloaks/modinfo.ini", &info),
            Z::Stored("Lantern Glow|natives|stm|glow.tex", b"glow"),
            Z::Deflated(&deep, b"deep"),
        ],
    );
    // A Windows tool's backslash-separated name.
    patch_bytes(
        &path,
        b"Lantern Glow|natives|stm|glow.tex",
        br"Lantern Glow\natives\stm\glow.tex",
    );

    let mut source = ZipSource::open(&path).unwrap();
    assert_eq!(source.format(), rift_core::PackageFormat::Zip);
    assert_eq!(
        paths(&mut source),
        [
            "Vernworth Cloaks Collection/Cloaks/modinfo.ini",
            "Lantern Glow/natives/stm/glow.tex",
            deep.as_str(),
        ]
    );
    let entries = source.entries().unwrap();
    assert_eq!(entries[0].size, info.len() as u64);
    assert_eq!(deep.split('/').count(), 18, "17 folders deep");
    assert_eq!(
        source
            .read_entry(r"lantern glow\NATIVES\stm\GLOW.TEX", None)
            .unwrap(),
        b"glow"
    );
}

#[test]
fn an_entry_that_climbs_out_is_dropped_and_can_never_be_read_or_extracted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Lantern Glow Recolor.zip");
    zip_at(
        &path,
        &[
            Z::Stored("Lantern Glow/modinfo.ini", b"name=Lantern Glow"),
            Z::Stored("zz/evil.dll", b"MZ"),
            Z::Stored("C:/Windows/evil.dll", b"MZ"),
        ],
    );
    patch_bytes(&path, b"zz/evil.dll", b"../evil.dll");

    let mut source = ZipSource::open(&path).unwrap();
    assert_eq!(paths(&mut source), ["Lantern Glow/modinfo.ini"]);
    assert!(matches!(
        source.read_entry("../evil.dll", None),
        Err(RiftError::EntryNotFound { .. })
    ));
    let (result, extracted, dests) = extract_to(&mut source, dir.path(), &["../evil.dll"]);
    assert!(matches!(result, Err(RiftError::EntryNotFound { .. })));
    assert!(extracted.progress.is_empty());
    assert!(!dests[0].exists());
    assert!(!dir.path().join("evil.dll").exists());
}

#[test]
fn names_honour_utf8_and_fall_back_to_code_page_437() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Lantern Glow Recolor.zip");
    zip_at(
        &path,
        &[
            // Non-ASCII: the writer sets the UTF-8 (EFS) flag.
            Z::Stored("Lantern Glow/Crème brûlée.txt", b"a"),
            // ASCII placeholders, patched below without touching the flag.
            Z::Stored("Lantern Glow/Caf_.txt", b"b"),
            Z::Stored("Lantern Glow/Ny__.txt", b"c"),
        ],
    );
    // 0x82 is "é" in code page 437 and invalid as UTF-8.
    patch_bytes(&path, b"Lantern Glow/Caf_.txt", b"Lantern Glow/Caf\x82.txt");
    // A flagless name that is valid UTF-8 stays UTF-8 (tools that forget the flag).
    patch_bytes(
        &path,
        b"Lantern Glow/Ny__.txt",
        "Lantern Glow/Nyé.txt".as_bytes(),
    );

    let mut source = ZipSource::open(&path).unwrap();
    assert_eq!(
        paths(&mut source),
        [
            "Lantern Glow/Crème brûlée.txt",
            "Lantern Glow/Café.txt",
            "Lantern Glow/Nyé.txt",
        ]
    );
    assert_eq!(
        source.read_entry("lantern glow/CAFÉ.TXT", None).unwrap(),
        b"b"
    );
}

#[test]
fn read_entry_reads_all_or_exactly_the_limit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Lantern Glow Recolor.zip");
    let big = compressible(1 << 20, 7);
    zip_at(
        &path,
        &[
            Z::Deflated("Lantern Glow/modinfo.ini", b"name=Lantern Glow"),
            Z::Deflated("Lantern Glow/Lantern Glow.pak", &big),
        ],
    );
    let mut source = ZipSource::open(&path).unwrap();
    let pak = "Lantern Glow/Lantern Glow.pak";
    assert_eq!(source.read_entry(pak, None).unwrap(), big);
    let head = source.read_entry(pak, Some(16 + 48 * 2)).unwrap();
    assert_eq!(head.len(), 16 + 48 * 2);
    assert_eq!(head, big[..16 + 48 * 2]);
    assert!(source.read_entry(pak, Some(0)).unwrap().is_empty());
    assert_eq!(source.read_entry(pak, Some(u64::MAX)).unwrap(), big);
    assert!(matches!(
        source.read_entry("Lantern Glow/missing.pak", Some(16)),
        Err(RiftError::EntryNotFound { entry }) if entry == "Lantern Glow/missing.pak"
    ));
}

#[test]
fn the_limit_stops_the_inflater_long_before_a_damaged_tail() {
    // The last ~4 KiB of the compressed stream is garbage. Inflating the whole
    // entry must fail; reading its head must not even get there.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Lantern Glow Recolor.zip");
    let big = compressible(2 << 20, 11);
    zip_at(&path, &[Z::Deflated("Lantern Glow/Lantern Glow.pak", &big)]);
    corrupt_before_central_directory(&path, 4096, 2048);

    let mut source = ZipSource::open(&path).unwrap();
    let head = source
        .read_entry("Lantern Glow/Lantern Glow.pak", Some(64 * 1024))
        .unwrap();
    assert_eq!(head, big[..64 * 1024]);
    let whole = source.read_entry("Lantern Glow/Lantern Glow.pak", None);
    assert!(
        matches!(&whole, Err(RiftError::CorruptArchive { path: p, .. }) if p.ends_with("Lantern Glow Recolor.zip")),
        "{whole:?}"
    );
}

#[test]
fn extract_writes_each_item_fully_and_reports_after_each() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Vernworth Cloaks Collection.zip");
    let cape = compressible(300_000, 3);
    zip_at(
        &path,
        &[
            Z::Deflated("Cloaks/Ermine Cape/modinfo.ini", b"name=Ermine Cape"),
            Z::Deflated("Cloaks/Ermine Cape/natives/stm/cape.mesh", &cape),
            Z::Stored("Cloaks/Ermine Cape/screenshot.png", b"\x89PNG"),
        ],
    );
    let mut source = ZipSource::open(&path).unwrap();
    let (result, extracted, dests) = extract_to(
        &mut source,
        dir.path(),
        &[
            "Cloaks/Ermine Cape/natives/stm/cape.mesh",
            "cloaks/ermine cape/MODINFO.INI",
        ],
    );
    result.unwrap();
    assert_eq!(fs::read(&dests[0]).unwrap(), cape);
    assert_eq!(fs::read(&dests[1]).unwrap(), b"name=Ermine Cape");
    let total = cape.len() as u64 + 16;
    let done: Vec<(usize, u64)> = extracted
        .progress
        .iter()
        .map(|p| (p.files_done, p.bytes_done))
        .collect();
    assert_eq!(done, [(1, cape.len() as u64), (2, total)]);
    assert!(extracted
        .progress
        .iter()
        .all(|p| p.files_total == 2 && p.bytes_total == total));
    let items: Vec<usize> = extracted.progress.iter().map(|p| p.item).collect();
    assert_eq!(items, [0, 1], "each report names the file it finished");
    assert!(path.exists(), "the library stays untouched");
}

#[test]
fn a_batch_with_a_missing_entry_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Lantern Glow Recolor.zip");
    zip_at(&path, &[Z::Stored("Lantern Glow/a.txt", b"a")]);
    let mut source = ZipSource::open(&path).unwrap();
    let (result, extracted, dests) = extract_to(
        &mut source,
        dir.path(),
        &["Lantern Glow/a.txt", "Lantern Glow/b.txt"],
    );
    assert!(
        matches!(result, Err(RiftError::EntryNotFound { entry }) if entry == "Lantern Glow/b.txt")
    );
    assert!(extracted.progress.is_empty());
    assert!(!dests[0].exists());
}

#[test]
fn a_partly_encrypted_zip_lists_and_refuses_only_the_locked_entry() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Lantern Glow Recolor.zip");
    zip_at(
        &path,
        &[
            Z::Stored("Lantern Glow/modinfo.ini", b"name=Lantern Glow"),
            Z::Stored("Lantern Glow/natives/stm/glow.tex", b"locked"),
        ],
    );
    mark_zip_entry_encrypted(&path, "Lantern Glow/natives/stm/glow.tex");
    let mut source = ZipSource::open(&path).unwrap();
    assert_eq!(paths(&mut source).len(), 2);
    assert_eq!(
        source.read_entry("Lantern Glow/modinfo.ini", None).unwrap(),
        b"name=Lantern Glow"
    );
    assert!(matches!(
        source.read_entry("Lantern Glow/natives/stm/glow.tex", None),
        Err(RiftError::Encrypted { .. })
    ));
    let (result, extracted, dests) = extract_to(
        &mut source,
        dir.path(),
        &[
            "Lantern Glow/modinfo.ini",
            "Lantern Glow/natives/stm/glow.tex",
        ],
    );
    assert!(matches!(result, Err(RiftError::Encrypted { .. })));
    assert!(extracted.progress.is_empty(), "refused before a byte moved");
    assert!(!dests[0].exists());
}

#[test]
fn a_fully_encrypted_zip_is_encrypted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Lantern Glow Recolor.zip");
    zip_at(
        &path,
        &[
            Z::Dir("Lantern Glow"),
            Z::Stored("Lantern Glow/modinfo.ini", b"name=Lantern Glow"),
            Z::Stored("Lantern Glow/natives/stm/glow.tex", b"locked"),
        ],
    );
    mark_zip_entry_encrypted(&path, "Lantern Glow/modinfo.ini");
    mark_zip_entry_encrypted(&path, "Lantern Glow/natives/stm/glow.tex");
    let err = ZipSource::open(&path).unwrap_err();
    assert!(
        matches!(&err, RiftError::Encrypted { path: p } if p.ends_with("Lantern Glow Recolor.zip")),
        "{err:?}"
    );
}

#[test]
fn case_folded_duplicates_keep_the_later_copy() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Lantern Glow Recolor.zip");
    zip_at(
        &path,
        &[
            Z::Stored("Lantern Glow/modinfo.ini", b"name=Old"),
            Z::Stored("lantern glow/MODINFO.INI", b"name=New"),
        ],
    );
    let mut source = ZipSource::open(&path).unwrap();
    assert_eq!(paths(&mut source), ["lantern glow/MODINFO.INI"]);
    assert_eq!(
        source.read_entry("Lantern Glow/modinfo.ini", None).unwrap(),
        b"name=New"
    );
}

#[test]
fn an_empty_zip_and_a_symlink_entry_hold_no_files() {
    let dir = tempfile::tempdir().unwrap();
    let empty = dir.path().join("Empty Satchel.zip");
    zip_at(&empty, &[]);
    assert_eq!(fs::read(&empty).unwrap()[..4], *b"PK\x05\x06");
    assert!(paths(&mut ZipSource::open(&empty).unwrap()).is_empty());

    let linked = dir.path().join("Lantern Glow Recolor.zip");
    let mut writer = zip::ZipWriter::new(fs::File::create(&linked).unwrap());
    writer
        .add_symlink(
            "Lantern Glow/natives",
            "/etc",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    writer.finish().unwrap();
    assert!(paths(&mut ZipSource::open(&linked).unwrap()).is_empty());
}

#[test]
fn a_damaged_zip_says_so_with_its_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Lantern Glow Recolor.zip");
    fs::write(&path, b"PK\x03\x04 this is not a zip at all").unwrap();
    let err = ZipSource::open(&path).unwrap_err();
    assert!(
        err.to_string().contains("Lantern Glow Recolor.zip"),
        "{err}"
    );
    assert!(matches!(err, RiftError::CorruptArchive { .. }), "{err:?}");
}

#[test]
fn a_zip_behind_a_program_stub_lists_reads_and_extracts() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Lantern Glow Setup.exe");
    let info = modinfo("Lantern Glow");
    zip_at(
        &path,
        &[
            Z::Deflated("Lantern Glow/modinfo.ini", &info),
            Z::Stored("Lantern Glow/natives/stm/glow.tex", b"glow"),
        ],
    );
    // A self-extracting download: a program glued in front, offsets not adjusted.
    let archive = fs::read(&path).unwrap();
    fs::write(
        &path,
        [b"MZ".as_slice(), &[0_u8; 70_000], &archive].concat(),
    )
    .unwrap();

    let mut source = ZipSource::open(&path).unwrap();
    assert_eq!(
        paths(&mut source),
        [
            "Lantern Glow/modinfo.ini",
            "Lantern Glow/natives/stm/glow.tex"
        ]
    );
    assert_eq!(
        source
            .read_entry("lantern glow/natives/stm/GLOW.TEX", None)
            .unwrap(),
        b"glow"
    );
    let (result, _, dests) = extract_to(&mut source, dir.path(), &["Lantern Glow/modinfo.ini"]);
    result.unwrap();
    assert_eq!(fs::read(&dests[0]).unwrap(), info);
}

#[test]
fn a_destination_taken_mid_batch_stops_the_batch_and_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Vernworth Cloaks Collection.zip");
    let cape = compressible(120_000, 5);
    zip_at(
        &path,
        &[
            Z::Deflated("Cloaks/Ermine Cape/natives/stm/cape.mesh", &cape),
            Z::Stored("Cloaks/Ermine Cape/modinfo.ini", b"name=Ermine Cape"),
            Z::Stored("Cloaks/Ermine Cape/screenshot.png", b"\x89PNG"),
        ],
    );
    let mut source = ZipSource::open(&path).unwrap();
    // The rival creates item 1's destination once item 0 is done.
    let (result, extracted, dests) = extract_racing(
        &mut source,
        dir.path(),
        &[
            "Cloaks/Ermine Cape/natives/stm/cape.mesh",
            "Cloaks/Ermine Cape/modinfo.ini",
            "Cloaks/Ermine Cape/screenshot.png",
        ],
        1,
        1,
    );
    assert_refused_at(&result, &dests[1]);
    assert_eq!(
        fs::read(&dests[0]).unwrap(),
        cape,
        "written whole before the rival came"
    );
    assert!(!dests[2].exists(), "nothing after the refusal");
    assert_eq!(extracted.progress.len(), 1);
    assert_eq!(
        extracted.progress[0].item, 0,
        "only the file it wrote is reported: the caller leaves the taken one be"
    );
}

#[cfg(unix)]
#[test]
fn a_link_planted_at_a_destination_is_never_written_through() {
    // RS-3: a link at a temporary name pointing at a vanilla file must not
    // turn the extraction into a write to that file.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Lantern Glow Recolor.zip");
    zip_at(
        &path,
        &[Z::Stored("Lantern Glow/Lantern Glow.pak", b"KPKA modded")],
    );
    let vanilla = dir.path().join("re_chunk_000.pak");
    fs::write(&vanilla, b"KPKA vanilla").unwrap();
    let dest = dir.path().join("staging/0.out");
    fs::create_dir_all(dest.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&vanilla, &dest).unwrap();
    let mut source = ZipSource::open(&path).unwrap();
    let (result, extracted, _) =
        extract_to(&mut source, dir.path(), &["Lantern Glow/Lantern Glow.pak"]);
    assert!(
        matches!(&result, Err(RiftError::Refused { reason })
            if reason.contains(&dest.display().to_string())),
        "{result:?}"
    );
    assert_eq!(fs::read(&vanilla).unwrap(), b"KPKA vanilla");
    assert!(dest.is_symlink(), "the link is left as it was too");
    assert!(extracted.progress.is_empty());
}
