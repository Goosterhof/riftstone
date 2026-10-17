//! Fixture builders shared by the scan and catalog tests. Every package a
//! test opens is built here at run time, in a temporary directory; every name
//! is invented.

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::time::{Duration, SystemTime};

use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// What a trouble card's window line already says (src/services/voice.ts:
/// `OPENS_WITH` and the unreadable line). The window owns the verdict and the
/// spine sends only the why, so a card states its verdict once (Chaos #00129 D3).
pub(crate) const WINDOW_VERDICTS: &[&str] = &[
    "will not be installed",
    "will not load",
    "won't load",
    "can be installed",
    "cannot be installed",
    "nothing the game loads",
    "could not open",
    "couldn't open",
    "could not be opened",
    "cannot open",
    "cannot look inside",
    "could not read",
];

/// A trouble message continues the window's sentence after its colon: it
/// starts with a lower-case word or a count, and it repeats no verdict.
pub(crate) fn assert_is_a_why(message: &str) {
    let first = message.chars().next().expect("a why is never empty");
    assert!(
        first.is_lowercase() || first.is_ascii_digit(),
        "a why continues the window's sentence: {message}"
    );
    let lower = message.to_lowercase();
    for verdict in WINDOW_VERDICTS {
        assert!(
            !lower.contains(verdict),
            "{verdict:?} is the window's to say: {message}"
        );
    }
}

/// A zip at `path` holding `files` (deflated), in the given order.
pub(crate) fn zip_at(path: &Path, files: &[(&str, &[u8])]) {
    let mut writer = ZipWriter::new(File::create(path).unwrap());
    for (name, bytes) in files {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap();
}

/// A solid 7z at `path`: every file in one LZMA2 block.
pub(crate) fn solid_7z_at(path: &Path, files: &[(&str, &[u8])]) {
    use sevenz_rust2::{
        ArchiveEntry, ArchiveWriter, EncoderConfiguration, EncoderMethod, SourceReader,
    };
    let mut writer = ArchiveWriter::create(path).unwrap();
    writer.set_content_methods(vec![EncoderConfiguration::new(EncoderMethod::LZMA2)]);
    let entries = files
        .iter()
        .map(|(name, _)| ArchiveEntry::new_file(name))
        .collect();
    let readers = files
        .iter()
        .map(|(_, bytes)| SourceReader::new(*bytes))
        .collect();
    writer.push_archive_entries(entries, readers).unwrap();
    writer.finish().unwrap();
}

/// A folder package at `path` holding `files`.
pub(crate) fn folder_at(path: &Path, files: &[(&str, &[u8])]) {
    for (name, bytes) in files {
        let file = path.join(name);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, bytes).unwrap();
    }
}

/// modinfo.ini text, with the CRLF line ends Windows tools write.
pub(crate) fn modinfo(lines: &[(&str, &str)]) -> Vec<u8> {
    lines
        .iter()
        .map(|(key, value)| format!("{key}={value}\r\n"))
        .collect::<String>()
        .into_bytes()
}

pub(crate) fn mtime(path: &Path) -> SystemTime {
    fs::metadata(path).unwrap().modified().unwrap()
}

pub(crate) fn set_mtime(path: &Path, time: SystemTime) {
    File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(time)
        .unwrap();
}

/// Overwrite a file with zeros of the same length and give it back its old
/// modification time: same stamp, unreadable content. A scan that reopens it
/// can only fail.
pub(crate) fn wreck_keeping_stamp(path: &Path) {
    let before = mtime(path);
    let len = fs::metadata(path).unwrap().len();
    fs::write(path, vec![0u8; usize::try_from(len).unwrap()]).unwrap();
    set_mtime(path, before);
    assert_eq!(mtime(path), before);
}

/// Move a file's modification time on by a minute.
pub(crate) fn touch_later(path: &Path) {
    set_mtime(path, mtime(path) + Duration::from_secs(60));
}

/// Set the "encrypted" bit of the entry named `name` in both its local and
/// its central header: the archive lists, but that entry cannot be read.
pub(crate) fn mark_zip_entry_encrypted(path: &Path, name: &str) {
    let mut bytes = fs::read(path).unwrap();
    let mut marked = 0;
    let mut i = 0;
    while i + 4 <= bytes.len() {
        let signature = &bytes[i..i + 4];
        let (flags_at, name_len_at, name_at) = if signature == b"PK\x03\x04" {
            (i + 6, i + 26, i + 30)
        } else if signature == b"PK\x01\x02" {
            (i + 8, i + 28, i + 46)
        } else {
            i += 1;
            continue;
        };
        let name_len = usize::from(u16::from_le_bytes([
            bytes[name_len_at],
            bytes[name_len_at + 1],
        ]));
        if bytes.get(name_at..name_at + name_len) == Some(name.as_bytes()) {
            bytes[flags_at] |= 1;
            marked += 1;
        }
        i += 4;
    }
    assert_eq!(marked, 2, "{name} should have a local and a central header");
    fs::write(path, bytes).unwrap();
}
