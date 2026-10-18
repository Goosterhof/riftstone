//! MD5 of a package on disk — how Nexus recognises an archive it served.

use std::fs::File;
use std::io::{ErrorKind, Read};
use std::path::Path;

use md5::{Digest, Md5};

/// Read size per step. Packages reach several GB; memory stays flat.
const CHUNK: usize = crate::DOWNLOAD_CHUNK_BYTES;

/// The MD5 of a file, streamed in 1 MiB steps. `progress` hears the running
/// byte count after every step. Lower-case hex.
pub fn md5_file(path: &Path, progress: &mut dyn FnMut(u64)) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Md5::new();
    let mut buffer = vec![0u8; CHUNK];
    let mut done: u64 = 0;
    loop {
        let read = match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(err) if err.kind() == ErrorKind::Interrupted => continue,
            Err(err) => return Err(err),
        };
        hasher.update(&buffer[..read]);
        done += read as u64;
        progress(done);
    }
    Ok(hex(&hasher.finalize()))
}

/// Is this 32 hexadecimal digits (either case)?
pub fn is_md5(text: &str) -> bool {
    text.len() == 32 && text.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Lower-case hex of a digest (shared with the downloader's running MD5).
pub(crate) fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(DIGITS[usize::from(byte >> 4)]));
        out.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn md5_of(bytes: &[u8]) -> (String, Vec<u64>) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Lantern Glow Recolor.zip");
        std::fs::File::create(&path)
            .unwrap()
            .write_all(bytes)
            .unwrap();
        let mut seen = Vec::new();
        let digest = md5_file(&path, &mut |done| seen.push(done)).unwrap();
        (digest, seen)
    }

    #[test]
    fn matches_the_known_digests() {
        assert_eq!(md5_of(b"").0, "d41d8cd98f00b204e9800998ecf8427e");
        let (digest, seen) = md5_of(b"The quick brown fox jumps over the lazy dog");
        assert_eq!(digest, "9e107d9d372bb6826bd81d3542a419d6");
        assert_eq!(seen, vec![43]);
    }

    #[test]
    fn streams_across_chunk_boundaries() {
        // 2.5 chunks of a repeating pattern: the digest must not depend on how
        // the reads were cut, and progress climbs to the full size.
        let bytes: Vec<u8> = (0..(CHUNK * 5 / 2)).map(|i| (i % 251) as u8).collect();
        let (digest, seen) = md5_of(&bytes);
        assert_eq!(digest, hex(&Md5::digest(&bytes)));
        assert_eq!(seen.last().copied(), Some(bytes.len() as u64));
        assert!(seen.windows(2).all(|pair| pair[0] < pair[1]), "{seen:?}");
        assert!(seen.len() >= 3);
    }

    #[test]
    fn a_missing_file_is_an_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("gone.zip");
        assert!(md5_file(&missing, &mut |_| {}).is_err());
    }

    #[test]
    fn recognises_md5_strings() {
        assert!(is_md5("d41d8cd98f00b204e9800998ecf8427e"));
        assert!(is_md5("D41D8CD98F00B204E9800998ECF8427E"));
        assert!(!is_md5("d41d8cd98f00b204e9800998ecf8427"));
        assert!(!is_md5("not-a-hash"));
        assert!(!is_md5("g41d8cd98f00b204e9800998ecf8427e"));
    }
}
