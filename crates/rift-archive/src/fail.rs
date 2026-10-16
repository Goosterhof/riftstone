//! How reading failures become [`RiftError`]s. The backend states facts; the
//! window gives them a voice. Every error built here names the package.

use std::io;
use std::path::Path;

use rift_core::RiftError;

pub(crate) fn encrypted(package: &Path) -> RiftError {
    RiftError::Encrypted {
        path: package.display().to_string(),
    }
}

pub(crate) fn unsupported(package: &Path) -> RiftError {
    RiftError::UnsupportedFormat {
        path: package.display().to_string(),
    }
}

/// A failure while pulling the bytes of `entry` out of `package`. An error
/// the operating system raised (a read error, a vanished or locked file)
/// means the package could not be read; anything a decoder raised (bad
/// compressed data, a failed checksum, a cut-off stream) means it is damaged.
pub(crate) fn entry_read(package: &Path, entry: &str, err: &io::Error) -> RiftError {
    if err.raw_os_error().is_some() {
        RiftError::unreadable(package, format!("reading {entry} failed: {err}"))
    } else if err.kind() == io::ErrorKind::UnexpectedEof {
        RiftError::corrupt(package, format!("{entry} is cut off before its end"))
    } else {
        RiftError::corrupt(package, format!("{entry} could not be unpacked: {err}"))
    }
}

/// `error` happened at an earlier point of a sequential read (another entry
/// of a solid stream, or a header on the way) and cost `entry`, which lies
/// past it: say so, so the message names both where the read broke and what
/// it could not reach. Kinds without a detail (`Encrypted`) stay as they are.
pub(crate) fn behind(error: RiftError, entry: &str) -> RiftError {
    let reach = format!("; {entry} lies past that point and could not be reached");
    match error {
        RiftError::CorruptArchive { path, detail } => RiftError::CorruptArchive {
            path,
            detail: detail + &reach,
        },
        RiftError::Unreadable { path, detail } => RiftError::Unreadable {
            path,
            detail: detail + &reach,
        },
        other => other,
    }
}

/// How an entry the listing may not hold is named in a message: its
/// normalized path when it has one, else its raw name (a dropped `../` name
/// is still the honest label for where a read broke).
pub(crate) fn entry_label(raw: &str) -> String {
    match rift_core::normalize_entry_path(raw) {
        Ok(path) if !path.is_empty() => path,
        _ => raw.to_string(),
    }
}

/// A failure while reading the package's own structure (its directory, its
/// headers) rather than one entry.
pub(crate) fn structure_read(package: &Path, err: &io::Error) -> RiftError {
    if err.raw_os_error().is_some() {
        RiftError::unreadable(package, err)
    } else if err.kind() == io::ErrorKind::UnexpectedEof {
        RiftError::corrupt(
            package,
            "the archive ends before its own directory says it should",
        )
    } else {
        RiftError::corrupt(package, err)
    }
}

/// A failure while writing an extracted file.
pub(crate) fn write(dest: &Path, entry: &str, err: &io::Error) -> RiftError {
    RiftError::io(
        format!("could not write {} (unpacking {entry})", dest.display()),
        err,
    )
}

/// Something already sits at `dest` (a file, a folder, a link): extraction
/// only ever creates new files, so it refuses rather than write over it or
/// through it. It is a refusal, the kind the deploy engine gives when its
/// own check finds the name taken: the same fact, caught at the last
/// moment. The reason is a full sentence naming the destination.
pub(crate) fn taken(dest: &Path, entry: &str) -> RiftError {
    RiftError::refused(format!(
        "\"{}\" already exists, so {entry} was not unpacked to it: unpacking only ever creates new \
         files, and never writes over or through one it did not put there. Move it out of the \
         way, then try again.",
        dest.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operating_system_errors_mean_unreadable_and_decoder_errors_mean_damaged() {
        let package = Path::new("Vernworth Cloaks Collection.7z");
        let os = io::Error::from_raw_os_error(5);
        assert!(matches!(
            entry_read(package, "a.mesh", &os),
            RiftError::Unreadable { .. }
        ));
        let decoder = io::Error::new(io::ErrorKind::InvalidData, "corrupt deflate stream");
        assert!(matches!(
            entry_read(package, "a.mesh", &decoder),
            RiftError::CorruptArchive { detail, .. } if detail.contains("a.mesh")
        ));
        let cut = io::Error::from(io::ErrorKind::UnexpectedEof);
        assert!(matches!(
            entry_read(package, "a.mesh", &cut),
            RiftError::CorruptArchive { detail, .. } if detail.contains("cut off")
        ));
        assert!(matches!(
            structure_read(package, &cut),
            RiftError::CorruptArchive { .. }
        ));
    }

    #[test]
    fn a_failure_further_back_names_both_places() {
        let package = Path::new("Vernworth Cloaks Collection.7z");
        let broke = entry_read(
            package,
            "Cloaks/Ermine Cape/natives/stm/cape.mesh",
            &io::Error::new(io::ErrorKind::InvalidData, "dist overflow"),
        );
        let err = behind(broke, "Cloaks/Wool Cape/modinfo.ini");
        assert!(
            matches!(&err, RiftError::CorruptArchive { detail, .. }
                if detail.starts_with("Cloaks/Ermine Cape/natives/stm/cape.mesh could not be unpacked")
                    && detail.ends_with("Cloaks/Wool Cape/modinfo.ini lies past that point and could not be reached")),
            "{err:?}"
        );
        assert!(err.to_string().contains("Vernworth Cloaks Collection.7z"));
        let os = behind(
            entry_read(package, "a.mesh", &io::Error::from_raw_os_error(5)),
            "b.ini",
        );
        assert!(matches!(&os, RiftError::Unreadable { detail, .. } if detail.contains("b.ini")));
        assert_eq!(behind(encrypted(package), "b.ini"), encrypted(package));
    }

    #[test]
    fn labels_prefer_the_normalized_path() {
        assert_eq!(
            entry_label(r"Cloaks\Ermine Cape\modinfo.ini"),
            "Cloaks/Ermine Cape/modinfo.ini"
        );
        assert_eq!(entry_label("../evil.dll"), "../evil.dll");
        assert_eq!(entry_label("./"), "./");
    }

    #[test]
    fn a_taken_destination_is_a_refusal_that_names_it() {
        let dest = Path::new("/game/natives/stm/cape.mesh.riftstone-part");
        let err = taken(dest, "Cloaks/Ermine Cape/natives/stm/cape.mesh");
        assert!(
            matches!(&err, RiftError::Refused { reason }
                if reason.starts_with("\"/game/natives/stm/cape.mesh.riftstone-part\" already exists")
                    && reason.contains("Cloaks/Ermine Cape/natives/stm/cape.mesh was not unpacked")),
            "{err:?}"
        );
    }

    #[test]
    fn every_reading_error_names_the_package() {
        let package = Path::new("Vernworth Cloaks Collection.7z");
        let err = io::Error::new(io::ErrorKind::InvalidData, "bad");
        for rift in [
            entry_read(package, "a", &err),
            structure_read(package, &err),
            encrypted(package),
            unsupported(package),
        ] {
            assert!(
                rift.to_string().contains("Vernworth Cloaks Collection.7z"),
                "{rift}"
            );
        }
    }
}
