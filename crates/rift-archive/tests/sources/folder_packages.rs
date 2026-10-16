use std::fs;
use std::path::Path;

use rift_archive::FolderSource;
use rift_core::{PackageFormat, PackageSource, RiftError};

use crate::fixtures::{assert_refused_at, extract_racing, extract_to, paths};

fn write(root: &Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

#[test]
fn a_folder_lists_its_files_in_name_order_with_forward_slashes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Lantern Glow Recolor");
    write(&root, "modinfo.ini", b"name=Lantern Glow");
    write(&root, "natives/stm/glow.tex", b"glow");
    write(&root, "Lantern Glow.pak", b"KPKA");
    fs::create_dir_all(root.join("empty folder")).unwrap();
    let mut source = FolderSource::open(&root).unwrap();
    assert_eq!(source.format(), PackageFormat::Folder);
    assert_eq!(
        paths(&mut source),
        ["Lantern Glow.pak", "modinfo.ini", "natives/stm/glow.tex"]
    );
    assert_eq!(source.entries().unwrap()[2].size, 4);
    assert_eq!(
        source.read_entry(r"NATIVES\STM\GLOW.TEX", None).unwrap(),
        b"glow"
    );
    assert_eq!(
        source.read_entry("Lantern Glow.pak", Some(2)).unwrap(),
        b"KP"
    );
    assert!(matches!(
        source.read_entry("natives/stm/missing.tex", None),
        Err(RiftError::EntryNotFound { .. })
    ));
}

#[test]
fn extract_copies_and_leaves_the_library_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Vernworth Cloaks Collection");
    write(&root, "Cloaks/Ermine Cape/natives/stm/cape.mesh", b"cape");
    write(&root, "Cloaks/Ermine Cape/modinfo.ini", b"name=Ermine Cape");
    let mut source = FolderSource::open(&root).unwrap();
    let out = dir.path().join("game");
    let (result, extracted, dests) = extract_to(
        &mut source,
        &out,
        &[
            "Cloaks/Ermine Cape/natives/stm/cape.mesh",
            "Cloaks/Ermine Cape/modinfo.ini",
        ],
    );
    result.unwrap();
    assert_eq!(fs::read(&dests[0]).unwrap(), b"cape");
    assert_eq!(fs::read(&dests[1]).unwrap(), b"name=Ermine Cape");
    assert_eq!(extracted.progress.len(), 2);
    let items: Vec<usize> = extracted.progress.iter().map(|p| p.item).collect();
    assert_eq!(items, [0, 1], "each report names the file it finished");
    assert_eq!(
        fs::read(root.join("Cloaks/Ermine Cape/natives/stm/cape.mesh")).unwrap(),
        b"cape",
        "copied, never moved (RS-2)"
    );
}

#[test]
fn the_walk_stops_sixty_four_folders_down() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Deep Satchel");
    let at_64 = format!("{}file.txt", "d/".repeat(63)); // depth 64: listed
    let at_65 = format!("{}file.txt", "d/".repeat(64)); // depth 65: beyond the cap
    write(&root, &at_64, b"64");
    write(&root, &at_65, b"65");
    let mut source = FolderSource::open(&root).unwrap();
    assert_eq!(paths(&mut source), [at_64]);
}

#[cfg(unix)]
#[test]
fn links_are_neither_followed_nor_listed() {
    let dir = tempfile::tempdir().unwrap();
    let outside = dir.path().join("outside");
    write(&outside, "secret.txt", b"not yours");
    let root = dir.path().join("Lantern Glow Recolor");
    write(&root, "modinfo.ini", b"name=Lantern Glow");
    std::os::unix::fs::symlink(&outside, root.join("natives")).unwrap();
    std::os::unix::fs::symlink(outside.join("secret.txt"), root.join("readme.txt")).unwrap();
    let mut source = FolderSource::open(&root).unwrap();
    assert_eq!(paths(&mut source), ["modinfo.ini"]);
}

#[test]
fn a_file_is_not_a_folder_package() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("Lantern Glow Recolor.txt");
    fs::write(&file, b"just text").unwrap();
    assert!(matches!(
        FolderSource::open(&file),
        Err(RiftError::UnsupportedFormat { .. })
    ));
    assert!(matches!(
        FolderSource::open(&dir.path().join("missing")),
        Err(RiftError::Unreadable { .. })
    ));
}

#[test]
fn a_destination_taken_mid_copy_stops_the_copy_and_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Vernworth Cloaks Collection");
    write(&root, "Cloaks/Ermine Cape/natives/stm/cape.mesh", b"cape");
    write(&root, "Cloaks/Ermine Cape/modinfo.ini", b"name=Ermine Cape");
    write(&root, "Cloaks/Ermine Cape/screenshot.png", b"\x89PNG");
    let mut source = FolderSource::open(&root).unwrap();
    let out = dir.path().join("game");
    let (result, extracted, dests) = extract_racing(
        &mut source,
        &out,
        &[
            "Cloaks/Ermine Cape/natives/stm/cape.mesh",
            "Cloaks/Ermine Cape/modinfo.ini",
            "Cloaks/Ermine Cape/screenshot.png",
        ],
        1,
        1,
    );
    assert_refused_at(&result, &dests[1]);
    assert_eq!(fs::read(&dests[0]).unwrap(), b"cape");
    assert!(!dests[2].exists(), "nothing after the refusal");
    assert_eq!(extracted.progress.len(), 1);
    assert_eq!(extracted.progress[0].item, 0, "the file it wrote, by name");
    assert_eq!(
        fs::read(root.join("Cloaks/Ermine Cape/modinfo.ini")).unwrap(),
        b"name=Ermine Cape",
        "the library is untouched (RS-2)"
    );
}
