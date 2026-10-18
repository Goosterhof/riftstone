//! Putting a file or folder in place without ever writing over one (v0.3's download landing,
//! shared since v0.5 with the older-versions folder).
//!
//! Every way in refuses an existing name (`AlreadyExists`), compared the way Windows compares
//! names: a download lands by a hard link or a no-replace move, an older version moves whole
//! ([`move_aside`]), and where a file system cannot refuse in the move itself, a file is copied
//! into a file made new. A folder is only ever moved, never copied.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rift_core::lower_key;

/// How many ` (n)` names [`move_aside`] tries before it gives up.
const MOST_NAMES: u32 = 999;

/// The entry of `folder` named `name`, compared the way Windows compares
/// names (case-blind): its own spelling and its path.
pub(crate) fn occupant(folder: &Path, name: &str) -> Option<(String, PathBuf)> {
    let wanted = lower_key(name);
    let listed = fs::read_dir(folder).ok().and_then(|entries| {
        entries.flatten().find_map(|entry| {
            let there = entry.file_name().to_string_lossy().into_owned();
            (lower_key(&there) == wanted).then(|| (there, entry.path()))
        })
    });
    listed.or_else(|| {
        let path = folder.join(name);
        fs::symlink_metadata(&path)
            .is_ok()
            .then(|| (name.to_string(), path))
    })
}

/// Rename `from` to `to` only when `to` does not exist, never over a file:
/// a hard link where the file system has them (a link fails on an existing
/// name; then the partial name goes), elsewhere (FAT32, exFAT, some network
/// shares) a move that itself refuses an existing name ([`move_new`]). An
/// existing name is `AlreadyExists` either way, whenever it appeared.
pub(crate) fn rename_new(from: &Path, to: &Path) -> io::Result<()> {
    rename_new_by(from, to, |from, to| fs::hard_link(from, to))
}

/// [`rename_new`] with the hard link as a seam (the tests make it fail).
pub(crate) fn rename_new_by(
    from: &Path,
    to: &Path,
    link: impl FnOnce(&Path, &Path) -> io::Result<()>,
) -> io::Result<()> {
    match link(from, to) {
        Ok(()) => {
            if let Err(err) = fs::remove_file(from) {
                log::warn!(
                    "the partial name of a landed download could not be removed ({err}); the next launch removes it"
                );
            }
            Ok(())
        }
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => Err(err),
        Err(_) => move_new(from, to),
    }
}

/// Move `from` to `to` in one step that fails with `AlreadyExists` when `to`
/// exists (std's `rename` would replace it): `MoveFileExW` without
/// `MOVEFILE_REPLACE_EXISTING` on Windows (whose `ERROR_ALREADY_EXISTS` and
/// `ERROR_FILE_EXISTS` std reads as `AlreadyExists`).
#[cfg(windows)]
pub(crate) fn move_new(from: &Path, to: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::MoveFileExW;

    // Through the folders' own verbatim (`\\?\`) spelling, so a name deep in
    // a long install path still moves.
    let wide = |path: &Path| -> io::Result<Vec<u16>> {
        let folder = path
            .parent()
            .filter(|folder| !folder.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let name = path
            .file_name()
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
        Ok(fs::canonicalize(folder)?
            .join(name)
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect())
    };
    let (from, to) = (wide(from)?, wide(to)?);
    // SAFETY: both are NUL-terminated UTF-16 strings that live across the call.
    let moved = unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0) };
    if moved == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// [`move_new`] on Linux (the bench): `renameat2` with `RENAME_NOREPLACE`. A
/// file system that cannot refuse in the move itself copies into a new file
/// instead ([`copy_new`]).
#[cfg(target_os = "linux")]
pub(crate) fn move_new(from: &Path, to: &Path) -> io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let c_path = |path: &Path| {
        CString::new(path.as_os_str().as_bytes())
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))
    };
    let (from_c, to_c) = (c_path(from)?, c_path(to)?);
    // SAFETY: both are NUL-terminated paths that live across the call.
    let moved = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            from_c.as_ptr(),
            libc::AT_FDCWD,
            to_c.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if moved == 0 {
        return Ok(());
    }
    let err = io::Error::last_os_error();
    match err.raw_os_error() {
        Some(libc::EINVAL | libc::ENOSYS | libc::EOPNOTSUPP) => copy_new(from, to),
        _ => Err(err),
    }
}

/// [`move_new`] elsewhere: [`copy_new`].
#[cfg(not(any(windows, target_os = "linux")))]
pub(crate) fn move_new(from: &Path, to: &Path) -> io::Result<()> {
    copy_new(from, to)
}

/// The last way to put a file in place without writing over one: copy it
/// into a file made new (`create_new` fails on an existing name), flushed to
/// disk, then remove the partial file. A copy that fails removes its own new
/// file and leaves the partial file for the caller.
#[cfg_attr(windows, allow(dead_code))]
pub(crate) fn copy_new(from: &Path, to: &Path) -> io::Result<()> {
    let mut target = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(to)?;
    let copied = fs::File::open(from)
        .and_then(|mut source| io::copy(&mut source, &mut target))
        .and_then(|_| target.sync_all());
    drop(target);
    if let Err(err) = copied {
        // The file this copy made, never one that was there.
        let _ = fs::remove_file(to);
        return Err(err);
    }
    if let Err(err) = fs::remove_file(from) {
        log::warn!(
            "the partial name of a landed download could not be removed ({err}); the next launch removes it"
        );
    }
    Ok(())
}

/// Move the package at `entry`, an archive or a folder, whole into `into` (made when missing),
/// under its own name or ` (2)` … when that one is taken there (any case). The move never lands
/// on an existing name and leaves no copy behind ([`move_new`]); a folder is never copied.
/// Returns the name it took.
pub(crate) fn move_aside(entry: &Path, into: &Path) -> io::Result<String> {
    let wanted = entry
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?
        .to_string();
    let is_folder = fs::symlink_metadata(entry)?.is_dir();
    fs::create_dir_all(into)?;
    let (stem, extension) = match wanted.rfind('.') {
        Some(at) if at > 0 && !is_folder => (&wanted[..at], &wanted[at..]),
        _ => (wanted.as_str(), ""),
    };
    for n in 1..=MOST_NAMES {
        let name = if n == 1 {
            wanted.clone()
        } else {
            format!("{stem} ({n}){extension}")
        };
        if occupant(into, &name).is_some() {
            continue;
        }
        match move_new(entry, &into.join(&name)) {
            Ok(()) => return Ok(name),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!("{MOST_NAMES} names like this one are taken there"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_move_where_hard_links_fail_never_writes_over_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("m-1.part");
        let to = dir.path().join("Lantern Glow Recolor.zip");
        fs::write(&from, b"the new archive").unwrap();
        // A file that appeared after any look: the move itself must refuse it.
        fs::write(&to, b"the player's own").unwrap();
        let err = move_new(&from, &to).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists, "{err}");
        assert_eq!(fs::read(&to).unwrap(), b"the player's own");
        assert_eq!(fs::read(&from).unwrap(), b"the new archive");
        // A free name: moved whole, the partial name gone.
        let free = dir.path().join("Lantern Glow Recolor (2).zip");
        move_new(&from, &free).unwrap();
        assert_eq!(fs::read(&free).unwrap(), b"the new archive");
        assert!(!from.exists());
    }

    #[test]
    fn the_last_resort_copy_never_writes_over_a_file_either() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("m-3.part");
        let to = dir.path().join("Lantern Glow Recolor.zip");
        fs::write(&from, b"the new archive").unwrap();
        fs::write(&to, b"the player's own").unwrap();
        let err = copy_new(&from, &to).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&to).unwrap(), b"the player's own");
        assert_eq!(fs::read(&from).unwrap(), b"the new archive");
        let free = dir.path().join("Lantern Glow Recolor (2).zip");
        copy_new(&from, &free).unwrap();
        assert_eq!(fs::read(&free).unwrap(), b"the new archive");
        assert!(!from.exists());
    }

    #[test]
    fn where_hard_links_fail_the_landing_moves_and_still_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let no_links = |_: &Path, _: &Path| -> io::Result<()> {
            Err(io::Error::from(io::ErrorKind::Unsupported))
        };
        let from = dir.path().join("m-2.part");
        let to = dir.path().join("Vernworth Cloaks Collection.zip");
        fs::write(&from, b"new").unwrap();
        fs::write(&to, b"old").unwrap();
        let err = rename_new_by(&from, &to, no_links).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&to).unwrap(), b"old");
        fs::remove_file(&to).unwrap();
        rename_new_by(&from, &to, no_links).unwrap();
        assert_eq!(fs::read(&to).unwrap(), b"new");
        assert!(!from.exists());
    }

    #[test]
    fn rename_new_refuses_an_existing_name() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("from");
        let to = dir.path().join("to");
        fs::write(&from, b"new").unwrap();
        fs::write(&to, b"old").unwrap();
        let err = rename_new(&from, &to).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&to).unwrap(), b"old");
        assert_eq!(fs::read(&from).unwrap(), b"new");
    }

    #[test]
    fn an_older_version_moves_whole_under_its_name_or_the_next_free_one() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        let older = dir.path().join("mods (older versions)");
        fs::create_dir_all(&mods).unwrap();
        let archive = mods.join("Lantern Glow 1.0.zip");
        fs::write(&archive, b"version one").unwrap();
        assert_eq!(
            move_aside(&archive, &older).unwrap(),
            "Lantern Glow 1.0.zip",
            "the folder is made at the first move"
        );
        assert!(!archive.exists());
        assert_eq!(
            fs::read(older.join("Lantern Glow 1.0.zip")).unwrap(),
            b"version one"
        );

        // The name is taken there, in another case: the next free name, never over it.
        fs::write(&archive, b"another one").unwrap();
        fs::rename(
            older.join("Lantern Glow 1.0.zip"),
            older.join("LANTERN GLOW 1.0.ZIP"),
        )
        .unwrap();
        assert_eq!(
            move_aside(&archive, &older).unwrap(),
            "Lantern Glow 1.0 (2).zip"
        );
        assert_eq!(
            fs::read(older.join("LANTERN GLOW 1.0.ZIP")).unwrap(),
            b"version one"
        );
        assert_eq!(
            fs::read(older.join("Lantern Glow 1.0 (2).zip")).unwrap(),
            b"another one"
        );
    }

    #[test]
    fn a_folder_package_moves_as_a_whole_folder() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("mods").join("Ermine Cape v2");
        fs::create_dir_all(folder.join("natives/stm")).unwrap();
        fs::write(folder.join("natives/stm/cape.mesh"), b"cape").unwrap();
        fs::write(folder.join("modinfo.ini"), b"name=Ermine Cape").unwrap();
        let older = dir.path().join("mods (older versions)");
        assert_eq!(
            move_aside(&folder, &older).unwrap(),
            "Ermine Cape v2",
            "a folder keeps its dots in its name"
        );
        assert!(!folder.exists());
        assert_eq!(
            fs::read(older.join("Ermine Cape v2/natives/stm/cape.mesh")).unwrap(),
            b"cape"
        );
        // And back again.
        let mods = dir.path().join("mods");
        assert_eq!(
            move_aside(&older.join("Ermine Cape v2"), &mods).unwrap(),
            "Ermine Cape v2"
        );
        assert_eq!(
            fs::read(mods.join("Ermine Cape v2/modinfo.ini")).unwrap(),
            b"name=Ermine Cape"
        );
    }
}
