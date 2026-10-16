use std::fs;

use rift_archive::{open, sniff};
use rift_core::{PackageFormat, RiftError};

use crate::fixtures::{rar5_at, sevenz_at, zip_at, R, S, Z};

#[test]
fn magic_bytes_decide_never_the_extension() {
    let dir = tempfile::tempdir().unwrap();
    // A zip wearing a .rar extension, a 7z wearing .zip, a RAR wearing .7z.
    let zip = dir.path().join("Lantern Glow Recolor.rar");
    zip_at(&zip, &[Z::Stored("Lantern Glow/a.txt", b"a")]);
    let sevenz = dir.path().join("Lantern Glow Recolor.zip");
    sevenz_at(&sevenz, vec![S::Single("Lantern Glow/a.txt", b"a")]);
    let rar = dir.path().join("Lantern Glow Recolor.7z");
    rar5_at(&rar, false, &[R::File("Lantern Glow/a.txt", b"a")]);

    assert_eq!(sniff(&zip).unwrap(), Some(PackageFormat::Zip));
    assert_eq!(sniff(&sevenz).unwrap(), Some(PackageFormat::SevenZip));
    assert_eq!(sniff(&rar).unwrap(), Some(PackageFormat::Rar));
    assert_eq!(sniff(dir.path()).unwrap(), Some(PackageFormat::Folder));

    for (path, format) in [
        (&zip, PackageFormat::Zip),
        (&sevenz, PackageFormat::SevenZip),
        (&rar, PackageFormat::Rar),
        (&dir.path().to_path_buf(), PackageFormat::Folder),
    ] {
        let mut source = open(path).unwrap();
        assert_eq!(source.format(), format);
        if format != PackageFormat::Folder {
            assert_eq!(source.read_entry("Lantern Glow/A.TXT", None).unwrap(), b"a");
        }
    }
}

#[test]
fn anything_else_is_not_a_package() {
    let dir = tempfile::tempdir().unwrap();
    for (name, bytes) in [
        (
            "readme.txt",
            &b"Lantern Glow Recolor, by the Pawn Guild"[..],
        ),
        ("Lantern Glow Recolor.zip.crdownload", b"PK\x03"),
        ("empty.zip", b""),
        ("installer.exe", b"MZ\x90\x00\x03\x00\x00\x00"),
    ] {
        let path = dir.path().join(name);
        fs::write(&path, bytes).unwrap();
        assert_eq!(sniff(&path).unwrap(), None, "{name}");
        assert!(
            matches!(open(&path), Err(RiftError::UnsupportedFormat { path: p }) if p.ends_with(name)),
            "{name}"
        );
    }
}

#[test]
fn a_path_that_cannot_be_read_is_unreadable() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("Lantern Glow Recolor.zip");
    assert!(matches!(
        sniff(&missing),
        Err(RiftError::Unreadable { path, .. }) if path.ends_with("Lantern Glow Recolor.zip")
    ));
    assert!(matches!(open(&missing), Err(RiftError::Unreadable { .. })));
}
