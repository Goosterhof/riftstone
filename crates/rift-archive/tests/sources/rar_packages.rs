//! RAR through the real UnRAR engine, on RAR5 archives forged at run time
//! (stored entries; see `fixtures::rar5_bytes`).

use std::fs;

use rift_archive::RarSource;
use rift_core::{PackageFormat, PackageSource, RiftError};

use crate::fixtures::{compressible, extract_to, modinfo, paths, rar5_at, rar5_bytes, R};

const SYMLINK: u64 = 1;
const HARD_LINK: u64 = 4;
const FILE_COPY: u64 = 5;

#[test]
fn folders_links_and_climbing_names_never_list_as_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Vernworth Cloaks Collection.rar");
    let info = modinfo("Ermine Cape");
    rar5_at(
        &path,
        false,
        &[
            R::Dir("Vernworth Cloaks Collection"),
            R::File("Vernworth Cloaks Collection/Cloaks/modinfo.ini", &info),
            R::File("../evil.dll", b"MZ"),
            R::File(
                "Vernworth Cloaks Collection/Cloaks/natives/stm/cape.mesh",
                b"cape",
            ),
            R::Link(
                "Vernworth Cloaks Collection/escape",
                SYMLINK,
                "/etc/passwd",
                0,
            ),
            R::Link(
                "Vernworth Cloaks Collection/Cloaks/natives/stm/cape copy.mesh",
                FILE_COPY,
                "Vernworth Cloaks Collection/Cloaks/natives/stm/cape.mesh",
                4,
            ),
            R::Link(
                "Vernworth Cloaks Collection/dangling",
                FILE_COPY,
                "Nowhere/at/all",
                9,
            ),
        ],
    );
    let mut source = RarSource::open(&path).unwrap();
    assert_eq!(source.format(), PackageFormat::Rar);
    assert!(!source.is_solid());
    assert_eq!(
        paths(&mut source),
        [
            "Vernworth Cloaks Collection/Cloaks/modinfo.ini",
            "Vernworth Cloaks Collection/Cloaks/natives/stm/cape.mesh",
            "Vernworth Cloaks Collection/Cloaks/natives/stm/cape copy.mesh",
        ]
    );
    assert_eq!(source.entries().unwrap()[0].size, info.len() as u64);
    assert_eq!(source.decode_passes(), 0, "listing reads headers only");
    assert!(matches!(
        source.read_entry("../evil.dll", None),
        Err(RiftError::EntryNotFound { .. })
    ));
}

#[test]
fn read_entry_is_case_blind_and_reads_all_or_exactly_the_limit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Lantern Glow Recolor.rar");
    let big = compressible(3 << 20, 21);
    rar5_at(
        &path,
        false,
        &[
            R::File("Lantern Glow/modinfo.ini", b"name=Lantern Glow"),
            R::File("Lantern Glow/Lantern Glow.pak", &big),
            R::File("Lantern Glow/screenshot.png", b"\x89PNG"),
        ],
    );
    let mut source = RarSource::open(&path).unwrap();
    assert_eq!(
        source
            .read_entry(r"LANTERN GLOW\lantern glow.PAK", Some(16))
            .unwrap(),
        big[..16]
    );
    assert_eq!(
        source
            .read_entry("Lantern Glow/Lantern Glow.pak", Some(16 + 48 * 31))
            .unwrap(),
        big[..16 + 48 * 31]
    );
    assert_eq!(
        source
            .read_entry("Lantern Glow/Lantern Glow.pak", None)
            .unwrap(),
        big
    );
    assert_eq!(
        source
            .read_entry("Lantern Glow/screenshot.png", None)
            .unwrap(),
        b"\x89PNG"
    );
    assert_eq!(
        source
            .read_entry("Lantern Glow/screenshot.png", Some(3))
            .unwrap(),
        b"\x89PN"
    );
    assert!(source
        .read_entry("Lantern Glow/Lantern Glow.pak", Some(0))
        .unwrap()
        .is_empty());
    assert!(matches!(
        source.read_entry("Lantern Glow/missing.pak", Some(16)),
        Err(RiftError::EntryNotFound { .. })
    ));
}

#[test]
fn the_limit_cancels_unrar_before_it_reaches_the_checksum() {
    // The entry's header carries a wrong CRC. Reading it whole must fail when
    // UnRAR checks the CRC at the end; reading its head must succeed, because
    // UnRAR was told to stop long before the end.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Lantern Glow Recolor.rar");
    let big = compressible(3 << 20, 23);
    rar5_at(
        &path,
        false,
        &[R::BadCrc("Lantern Glow/Lantern Glow.pak", &big)],
    );
    let mut source = RarSource::open(&path).unwrap();
    let head = source
        .read_entry("Lantern Glow/Lantern Glow.pak", Some(16))
        .unwrap();
    assert_eq!(head, big[..16]);
    let whole = source.read_entry("Lantern Glow/Lantern Glow.pak", None);
    assert!(
        matches!(&whole, Err(RiftError::CorruptArchive { path: p, detail })
            if p.ends_with("Lantern Glow Recolor.rar")
                && detail.ends_with("while unpacking Lantern Glow/Lantern Glow.pak")),
        "{whole:?}"
    );
    // The bad entry poisons nothing: a fresh walk reads the head again.
    assert_eq!(
        source
            .read_entry("Lantern Glow/Lantern Glow.pak", Some(4))
            .unwrap(),
        big[..4]
    );
}

#[test]
fn extract_is_one_walk_in_archive_order() {
    for solid in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Vernworth Cloaks Collection.rar");
        let meshes: Vec<(String, Vec<u8>)> = (0..6)
            .map(|n| {
                (
                    format!("Cloaks/Cape {n}/natives/stm/cape.mesh"),
                    compressible(70_000 + n, n as u32),
                )
            })
            .collect();
        let items: Vec<R<'_>> = meshes
            .iter()
            .map(|(name, bytes)| R::File(name, bytes))
            .collect();
        rar5_at(&path, solid, &items);
        let mut source = RarSource::open(&path).unwrap();
        assert_eq!(source.is_solid(), solid);
        let wanted = [
            meshes[5].0.as_str(),
            meshes[0].0.as_str(),
            meshes[3].0.as_str(),
        ];
        let (result, extracted, dests) = extract_to(&mut source, dir.path(), &wanted);
        result.unwrap();
        assert_eq!(
            source.decode_passes(),
            1,
            "solid={solid}: one walk for the whole batch"
        );
        assert_eq!(fs::read(&dests[0]).unwrap(), meshes[5].1);
        assert_eq!(fs::read(&dests[1]).unwrap(), meshes[0].1);
        assert_eq!(fs::read(&dests[2]).unwrap(), meshes[3].1);
        // Files are reported as the walk passes them: archive order.
        let bytes: Vec<u64> = extracted.progress.iter().map(|p| p.bytes_done).collect();
        let (a, b, c) = (
            meshes[0].1.len() as u64,
            meshes[3].1.len() as u64,
            meshes[5].1.len() as u64,
        );
        assert_eq!(bytes, [a, a + b, a + b + c], "solid={solid}");
        assert!(extracted
            .progress
            .iter()
            .all(|p| p.files_total == 3 && p.bytes_total == a + b + c));
    }
}

#[test]
fn a_link_is_served_from_the_entry_it_points_at() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Vernworth Cloaks Collection.rar");
    let texture = compressible(5_000, 31);
    rar5_at(
        &path,
        true,
        &[
            R::File("Cloaks/Ermine Cape/natives/stm/fur.tex", &texture),
            R::File("Cloaks/Ermine Cape/modinfo.ini", b"name=Ermine Cape"),
            R::Link(
                "Cloaks/Wool Cape/natives/stm/fur.tex",
                FILE_COPY,
                "Cloaks/Ermine Cape/natives/stm/fur.tex",
                5_000,
            ),
            R::Link(
                "Cloaks/Silk Cape/natives/stm/fur.tex",
                HARD_LINK,
                "Cloaks/Ermine Cape/natives/stm/fur.tex",
                5_000,
            ),
        ],
    );
    let mut source = RarSource::open(&path).unwrap();
    assert_eq!(paths(&mut source).len(), 4);
    assert_eq!(
        source
            .read_entry("Cloaks/Wool Cape/natives/stm/fur.tex", None)
            .unwrap(),
        texture
    );
    let (result, extracted, dests) = extract_to(
        &mut source,
        dir.path(),
        &[
            "Cloaks/Silk Cape/natives/stm/fur.tex",
            "Cloaks/Wool Cape/natives/stm/fur.tex",
            "Cloaks/Ermine Cape/modinfo.ini",
        ],
    );
    result.unwrap();
    assert_eq!(fs::read(&dests[0]).unwrap(), texture);
    assert_eq!(fs::read(&dests[1]).unwrap(), texture);
    assert_eq!(fs::read(&dests[2]).unwrap(), b"name=Ermine Cape");
    assert_eq!(extracted.progress.len(), 3);
}

#[test]
fn reading_every_modinfo_costs_one_walk_solid_or_not() {
    for solid in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Vernworth Cloaks Collection.rar");
        let options: Vec<(String, Vec<u8>, String, Vec<u8>)> = (0..12)
            .map(|n| {
                (
                    format!("Cloaks/Cape {n}/modinfo.ini"),
                    modinfo(&format!("Cape {n}")),
                    format!("Cloaks/Cape {n}/natives/stm/cape.mesh"),
                    compressible(300_000, 40 + n),
                )
            })
            .collect();
        let mut items = Vec::new();
        for (info_name, info, mesh_name, mesh) in &options {
            items.push(R::File(info_name, info));
            items.push(R::File(mesh_name, mesh));
        }
        rar5_at(&path, solid, &items);
        let mut source = RarSource::open(&path).unwrap();
        for (info_name, info, _, _) in &options {
            assert_eq!(
                &source.read_entry(info_name, Some(64 * 1024)).unwrap(),
                info
            );
        }
        assert_eq!(source.decode_passes(), 1, "solid={solid}");
    }
}

/// Four options; the third modinfo carries a wrong checksum.
fn four_capes_the_third_damaged(path: &std::path::Path, solid: bool) -> Vec<(String, Vec<u8>)> {
    let infos: Vec<(String, Vec<u8>)> = (0..4)
        .map(|n| {
            (
                format!("Cloaks/Cape {n}/modinfo.ini"),
                modinfo(&format!("Cape {n}")),
            )
        })
        .collect();
    rar5_at(
        path,
        solid,
        &[
            R::File(&infos[0].0, &infos[0].1),
            R::File(&infos[1].0, &infos[1].1),
            R::BadCrc(&infos[2].0, &infos[2].1),
            R::File(&infos[3].0, &infos[3].1),
        ],
    );
    infos
}

#[test]
fn a_damaged_modinfo_costs_no_other_option_its_own() {
    // Non-solid: entries do not depend on each other. Every modinfo but the
    // damaged one reads, before it and after it; only the damaged one fails,
    // and its error names it.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Vernworth Cloaks Collection.rar");
    let infos = four_capes_the_third_damaged(&path, false);
    let mut source = RarSource::open(&path).unwrap();
    for n in [0, 1, 3] {
        assert_eq!(
            source.read_entry(&infos[n].0, Some(64 * 1024)).unwrap(),
            infos[n].1,
            "Cape {n}"
        );
    }
    let err = source.read_entry(&infos[2].0, Some(64 * 1024)).unwrap_err();
    assert!(
        matches!(&err, RiftError::CorruptArchive { path: p, detail }
            if p.ends_with("Vernworth Cloaks Collection.rar")
                && detail.ends_with("while unpacking Cloaks/Cape 2/modinfo.ini")),
        "{err:?}"
    );
    // One walk brought Capes 0 and 1 and stopped at the damage; one more
    // brought Cape 3 without retrying the damaged sibling; the last read the
    // damaged entry on its own.
    assert_eq!(source.decode_passes(), 3);
}

#[test]
fn in_a_solid_rar_damage_costs_only_what_lies_behind_it() {
    // Solid: UnRAR's DLL tests every entry a walk passes, skips included (it
    // matches every file, so a solid skip is a test without the data
    // callback). Capes 0 and 1 lie in front of the damage and must read —
    // the sibling pass that reads them runs into it on the way to Cape 3.
    // Cape 3 lies behind it: unreachable, as it would be with no sibling
    // capture at all, and the error says where the walk broke.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Vernworth Cloaks Collection.rar");
    let infos = four_capes_the_third_damaged(&path, true);
    let mut source = RarSource::open(&path).unwrap();
    for n in [0, 1] {
        assert_eq!(
            source.read_entry(&infos[n].0, Some(64 * 1024)).unwrap(),
            infos[n].1,
            "Cape {n}"
        );
    }
    let behind = source.read_entry(&infos[3].0, Some(64 * 1024)).unwrap_err();
    assert!(
        matches!(&behind, RiftError::CorruptArchive { detail, .. }
            if detail.contains("while skipping Cloaks/Cape 2/modinfo.ini")
                && detail.contains("Cloaks/Cape 3/modinfo.ini lies past that point")),
        "{behind:?}"
    );
    let own = source.read_entry(&infos[2].0, Some(64 * 1024)).unwrap_err();
    assert!(
        matches!(&own, RiftError::CorruptArchive { detail, .. }
            if detail.ends_with("while unpacking Cloaks/Cape 2/modinfo.ini")),
        "{own:?}"
    );
    assert_eq!(
        source.decode_passes(),
        3,
        "the damaged sibling is never retried"
    );
}

#[test]
fn a_damaged_sibling_ahead_of_the_entry_asked_for_is_stepped_over() {
    // Non-solid: the damaged modinfo sits in front of the one asked for.
    // Capturing it on the way fails; the read must still succeed, as it does
    // with no sibling capture at all (a non-solid skip is a seek).
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Vernworth Cloaks Collection.rar");
    let (ermine, wool) = (modinfo("Ermine Cape"), modinfo("Wool Cape"));
    for solid in [false, true] {
        rar5_at(
            &path,
            solid,
            &[
                R::BadCrc("Cloaks/Ermine Cape/modinfo.ini", &ermine),
                R::File("Cloaks/Wool Cape/modinfo.ini", &wool),
            ],
        );
        let mut source = RarSource::open(&path).unwrap();
        let wool_read = source.read_entry("Cloaks/Wool Cape/modinfo.ini", None);
        if solid {
            // Solid: Wool Cape lies behind the damage, as above.
            assert!(
                matches!(&wool_read, Err(RiftError::CorruptArchive { detail, .. })
                    if detail.contains("while skipping Cloaks/Ermine Cape/modinfo.ini")),
                "{wool_read:?}"
            );
        } else {
            assert_eq!(wool_read.unwrap(), wool);
            assert_eq!(
                source.decode_passes(),
                2,
                "one walk spent on the sibling, one for the entry"
            );
        }
        let err = source
            .read_entry("Cloaks/Ermine Cape/modinfo.ini", None)
            .unwrap_err();
        assert!(
            matches!(&err, RiftError::CorruptArchive { detail, .. }
                if detail.ends_with("while unpacking Cloaks/Ermine Cape/modinfo.ini")),
            "solid={solid}: {err:?}"
        );
    }
}

#[test]
fn a_locked_entry_in_a_solid_rar_locks_only_what_lies_behind_it() {
    // Solid: reaching Wool Cape means decompressing the locked texture, which
    // needs the password. Ermine Cape lies in front of it and is not
    // encrypted: reading it must succeed, although the sibling pass that
    // reads it runs into the lock on the way to Wool Cape.
    for solid in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Vernworth Cloaks Collection.rar");
        let (ermine, wool) = (modinfo("Ermine Cape"), modinfo("Wool Cape"));
        rar5_at(
            &path,
            solid,
            &[
                R::File("Cloaks/Ermine Cape/modinfo.ini", &ermine),
                R::Encrypted("Cloaks/Locked/natives/stm/fur.tex", 40),
                R::File("Cloaks/Wool Cape/modinfo.ini", &wool),
            ],
        );
        let mut source = RarSource::open(&path).unwrap();
        assert_eq!(
            source
                .read_entry("Cloaks/Ermine Cape/modinfo.ini", Some(64 * 1024))
                .unwrap(),
            ermine,
            "solid={solid}"
        );
        let behind = source.read_entry("Cloaks/Wool Cape/modinfo.ini", Some(64 * 1024));
        if solid {
            assert!(
                matches!(behind, Err(RiftError::Encrypted { .. })),
                "behind the lock in a solid stream: {behind:?}"
            );
        } else {
            assert_eq!(behind.unwrap(), wool, "non-solid: a skip is a seek");
        }
    }
}

#[test]
fn a_partly_encrypted_rar_lists_and_refuses_only_the_locked_entry() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Lantern Glow Recolor.rar");
    rar5_at(
        &path,
        false,
        &[
            R::File("Lantern Glow/modinfo.ini", b"name=Lantern Glow"),
            R::Encrypted("Lantern Glow/natives/stm/glow.tex", 40),
            R::File("Lantern Glow/readme.txt", b"read me"),
        ],
    );
    let mut source = RarSource::open(&path).unwrap();
    assert_eq!(paths(&mut source).len(), 3);
    assert!(matches!(
        source.read_entry("Lantern Glow/natives/stm/glow.tex", None),
        Err(RiftError::Encrypted { .. })
    ));
    // Entries after the locked one are still reachable (non-solid: a seek).
    assert_eq!(
        source.read_entry("Lantern Glow/readme.txt", None).unwrap(),
        b"read me"
    );
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
fn a_fully_encrypted_rar_is_encrypted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Lantern Glow Recolor.rar");
    rar5_at(
        &path,
        false,
        &[
            R::Dir("Lantern Glow"),
            R::Encrypted("Lantern Glow/modinfo.ini", 18),
            R::Encrypted("Lantern Glow/natives/stm/glow.tex", 40),
        ],
    );
    let err = RarSource::open(&path).unwrap_err();
    assert!(
        matches!(&err, RiftError::Encrypted { path: p } if p.ends_with("Lantern Glow Recolor.rar")),
        "{err:?}"
    );
}

#[test]
fn a_cut_off_or_damaged_rar_is_damaged_not_empty() {
    let dir = tempfile::tempdir().unwrap();
    let whole = rar5_bytes(
        false,
        &[
            R::File("Lantern Glow/modinfo.ini", b"name=Lantern Glow"),
            R::File(
                "Lantern Glow/natives/stm/glow.tex",
                &compressible(10_000, 3),
            ),
        ],
        true,
    );
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("cut mid-entry", whole[..whole.len() - 3_000].to_vec()),
        (
            "no end block",
            rar5_bytes(false, &[R::File("Lantern Glow/a.txt", b"a")], false),
        ),
        (
            "signature and nonsense",
            b"Rar!\x1A\x07\x01\x00 this is not a header chain".to_vec(),
        ),
        ("a flipped header byte", {
            let mut bytes = whole.clone();
            // Byte 15 is the main header's archive flags (signature 8 +
            // CRC 4 + size 1 + type 1 + flags 1): its CRC no longer matches.
            bytes[15] ^= 0x01;
            bytes
        }),
    ];
    for (case, bytes) in cases {
        let path = dir.path().join("Lantern Glow Recolor.rar");
        fs::write(&path, bytes).unwrap();
        let err = RarSource::open(&path).unwrap_err();
        assert!(
            matches!(&err, RiftError::CorruptArchive { path: p, .. } if p.ends_with("Lantern Glow Recolor.rar")),
            "{case}: {err:?}"
        );
    }
    let path = dir.path().join("Lantern Glow Recolor.rar");
    fs::write(&path, &whole[..whole.len() - 3_000]).unwrap();
    let err = RarSource::open(&path).unwrap_err();
    assert!(err.to_string().contains("cut off"), "{err}");
}

#[test]
fn an_archive_with_only_folders_lists_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Empty Satchel.rar");
    rar5_at(&path, false, &[R::Dir("Satchel"), R::Dir("Satchel/Pocket")]);
    let mut source = RarSource::open(&path).unwrap();
    assert!(paths(&mut source).is_empty());
}

#[test]
fn a_batch_with_a_missing_entry_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Lantern Glow Recolor.rar");
    rar5_at(&path, false, &[R::File("Lantern Glow/a.txt", b"a")]);
    let mut source = RarSource::open(&path).unwrap();
    let (result, extracted, dests) = extract_to(
        &mut source,
        dir.path(),
        &["Lantern Glow/a.txt", "Lantern Glow/b.txt"],
    );
    assert!(matches!(result, Err(RiftError::EntryNotFound { .. })));
    assert!(extracted.progress.is_empty());
    assert!(!dests[0].exists());
    assert_eq!(source.decode_passes(), 0);
}

#[test]
fn a_bad_entry_mid_batch_stops_the_batch_and_leaves_no_half_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Lantern Glow Recolor.rar");
    let bad = compressible(200_000, 8);
    rar5_at(
        &path,
        false,
        &[
            R::File("Lantern Glow/a.txt", b"first"),
            R::BadCrc("Lantern Glow/b.tex", &bad),
            R::File("Lantern Glow/c.txt", b"third"),
        ],
    );
    let mut source = RarSource::open(&path).unwrap();
    let (result, extracted, dests) = extract_to(
        &mut source,
        dir.path(),
        &[
            "Lantern Glow/a.txt",
            "Lantern Glow/b.tex",
            "Lantern Glow/c.txt",
        ],
    );
    assert!(
        matches!(result, Err(RiftError::CorruptArchive { .. })),
        "{result:?}"
    );
    assert_eq!(
        fs::read(&dests[0]).unwrap(),
        b"first",
        "written before the failure"
    );
    assert!(!dests[1].exists(), "the failed file is removed");
    assert!(!dests[2].exists(), "nothing after the failure");
    assert_eq!(extracted.progress.len(), 1);
}

#[test]
fn a_rar_with_encrypted_headers_is_encrypted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Lantern Glow Recolor.rar");
    fs::write(&path, crate::fixtures::rar5_encrypted_headers_bytes()).unwrap();
    let err = RarSource::open(&path).unwrap_err();
    assert!(matches!(err, RiftError::Encrypted { .. }), "{err:?}");
}

#[test]
fn rar4_lists_reads_and_extracts_through_the_same_door() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Lantern Glow Recolor.rar");
    let texture = compressible(90_000, 17);
    fs::write(
        &path,
        crate::fixtures::rar4_bytes(
            &[
                (r"Lantern Glow\modinfo.ini", b"name=Lantern Glow"),
                (r"Lantern Glow\natives\stm\glow.tex", &texture),
            ],
            true,
        ),
    )
    .unwrap();
    assert_eq!(
        rift_archive::sniff(&path).unwrap(),
        Some(PackageFormat::Rar)
    );
    let mut source = RarSource::open(&path).unwrap();
    assert_eq!(
        paths(&mut source),
        [
            "Lantern Glow/modinfo.ini",
            "Lantern Glow/natives/stm/glow.tex"
        ]
    );
    assert_eq!(
        source
            .read_entry("lantern glow/natives/stm/GLOW.TEX", Some(10))
            .unwrap(),
        texture[..10]
    );
    let (result, _, dests) = extract_to(
        &mut source,
        dir.path(),
        &[
            "Lantern Glow/natives/stm/glow.tex",
            "Lantern Glow/modinfo.ini",
        ],
    );
    result.unwrap();
    assert_eq!(fs::read(&dests[0]).unwrap(), texture);
    assert_eq!(fs::read(&dests[1]).unwrap(), b"name=Lantern Glow");
}

#[test]
fn rar4_may_end_without_an_end_block_but_not_mid_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Lantern Glow Recolor.rar");
    let files: [(&str, &[u8]); 2] = [
        (r"Lantern Glow\modinfo.ini", b"name=Lantern Glow"),
        (r"Lantern Glow\readme.txt", b"read me, carefully"),
    ];
    // RAR 1.5-2.x archives may simply stop after their last block.
    fs::write(&path, crate::fixtures::rar4_bytes(&files, false)).unwrap();
    let mut source = RarSource::open(&path).unwrap();
    assert_eq!(paths(&mut source).len(), 2);
    // Cut inside the last file's data: that is an unfinished download.
    let whole = crate::fixtures::rar4_bytes(&files, true);
    fs::write(&path, &whole[..whole.len() - 7 - 5]).unwrap();
    let err = RarSource::open(&path).unwrap_err();
    assert!(
        matches!(&err, RiftError::CorruptArchive { detail, .. } if detail.contains("cut off")),
        "{err:?}"
    );
}
