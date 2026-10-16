//! Zip packages, through the pure-Rust `zip` crate (deflate, deflate64,
//! bzip2, zstd, lzma — what Nexus uploads use).
//!
//! **Names.** The EFS flag (general-purpose bit 11) promises UTF-8. Without
//! it a name is whatever the zipping tool wrote: the `zip` crate keeps a name
//! that decodes as strict UTF-8 (the many tools that write UTF-8 and forget
//! the flag) and decodes anything else as IBM code page 437, the zip
//! default — `Caf\x82.txt` becomes `Café.txt`. With the flag set the bytes
//! are UTF-8 by promise, so one rule covers both cases: the crate's
//! `ZipFileEntry::name`.
//!
//! **Random access.** Zip keeps a central directory, so every entry is read
//! on its own and `extract` simply goes in item order. A read with a `limit`
//! pulls no more than the limit through the inflater.
//!
//! **Opening reads the end, not the file.** Left to find the archive's start
//! itself, `zip` 9 scans every byte before the end record for directory
//! signatures, through a buffer that each seek empties: 172 s to open a
//! 256 MB collection on the investor's library over WSL's `/mnt/c`
//! (measured 2026-10-16). Nearly every zip starts at byte 0, so the
//! directory is first read where that start puts it; only a zip behind a
//! program stub (a self-extracting download) fails that check and pays for
//! the search. And the directory walk seeks between every two records, so
//! the file sits behind [`SeekBuffered`], whose seeks keep the buffer.

use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use rift_core::{
    ArchiveEntry, ExtractItem, ExtractProgress, PackageFormat, PackageSource, RiftError,
};
use zip::read::{ArchiveOffset, Config, ZipFileEntry};
use zip::result::ZipError;
use zip::ZipArchive;

use crate::fail;
use crate::index::{EntryIndex, IndexBuilder};
use crate::write::{read_capped, write_from_reader, Tally};

const READ_BUFFER: usize = 64 * 1024;

/// Unix file-type bits, as zip stores them in the high half of the external
/// attributes (and as the `zip` crate maps the DOS directory attribute).
const S_IFMT: u32 = 0o170_000;
const S_IFDIR: u32 = 0o040_000;
const S_IFLNK: u32 = 0o120_000;

/// A zip archive, opened once and read entry by entry.
pub struct ZipSource {
    path: PathBuf,
    archive: ZipArchive<SeekBuffered<File>>,
    index: EntryIndex<usize>,
}

impl std::fmt::Debug for ZipSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ZipSource")
            .field("path", &self.path)
            .field("entries", &self.index.entries().len())
            .finish()
    }
}

impl ZipSource {
    /// Open the zip at `path` and index its files.
    pub fn open(path: &Path) -> Result<Self, RiftError> {
        let archive = read_directory(path, || {
            File::open(path)
                .map(SeekBuffered::new)
                .map_err(|err| RiftError::unreadable(path, err))
        })?;

        let mut builder = IndexBuilder::new(path);
        for number in 0..archive.len() {
            let entry = archive
                .by_index_data(number)
                .map_err(|err| zip_failure(path, err))?;
            if is_directory(&entry) {
                continue;
            }
            let name = match entry.name() {
                Ok(name) => name.into_owned(),
                Err(err) => {
                    log::warn!(
                        "{}: skipped entry {number}: its name could not be decoded ({err})",
                        path.display()
                    );
                    continue;
                }
            };
            if is_link(&entry) {
                log::warn!(
                    "{}: skipped {name:?}: it is a symbolic link, and links are never installed",
                    path.display()
                );
                continue;
            }
            builder.push(&name, entry.size(), entry.encrypted(), number);
        }
        let index = builder.finish();
        if index.fully_encrypted() {
            return Err(fail::encrypted(path));
        }
        Ok(ZipSource {
            path: path.to_path_buf(),
            archive,
            index,
        })
    }
}

impl PackageSource for ZipSource {
    fn format(&self) -> PackageFormat {
        PackageFormat::Zip
    }

    fn entries(&mut self) -> Result<Vec<ArchiveEntry>, RiftError> {
        Ok(self.index.listing())
    }

    fn read_entry(&mut self, entry: &str, limit: Option<u64>) -> Result<Vec<u8>, RiftError> {
        let (_, found) = self.index.lookup(entry)?;
        if found.encrypted {
            return Err(fail::encrypted(&self.path));
        }
        if limit == Some(0) {
            return Ok(Vec::new());
        }
        let (number, size, name) = (found.locator, found.size, found.path.clone());
        let mut file = self
            .archive
            .by_index(number)
            .map_err(|err| zip_failure(&self.path, err))?;
        read_capped(&mut file, limit, size).map_err(|err| fail::entry_read(&self.path, &name, &err))
    }

    fn extract(
        &mut self,
        items: &[ExtractItem],
        progress: &mut dyn FnMut(ExtractProgress),
    ) -> Result<(), RiftError> {
        let positions = self.index.resolve(&self.path, items)?;
        let mut tally = Tally::new(progress, items.len(), self.index.total_size(&positions));
        for (at, (item, position)) in items.iter().zip(positions).enumerate() {
            let found = self.index.get(position);
            let (number, name) = (found.locator, found.path.as_str());
            let mut file = self
                .archive
                .by_index(number)
                .map_err(|err| zip_failure(&self.path, err))?;
            let package = self.path.as_path();
            let written = write_from_reader(&item.dest, name, &mut file, &|err| {
                fail::entry_read(package, name, err)
            })?;
            tally.file_done(at, written);
        }
        Ok(())
    }
}

/// Read the zip's central directory, trying first where an archive that
/// starts at byte 0 keeps it, and searching the whole file only when that
/// fails (a zip behind a program stub). `open` hands out a fresh reader for
/// each attempt. When both fail, the search's error is the one reported.
fn read_directory<R: Read + Seek>(
    path: &Path,
    mut open: impl FnMut() -> Result<R, RiftError>,
) -> Result<ZipArchive<R>, RiftError> {
    let at_byte_zero = Config {
        archive_offset: ArchiveOffset::Known(0),
    };
    match ZipArchive::with_config(at_byte_zero, open()?) {
        Ok(archive) => Ok(archive),
        Err(err) => {
            log::debug!(
                "{}: no zip directory where an archive starting at byte 0 keeps it ({err}); searching the whole file",
                path.display()
            );
            ZipArchive::new(open()?).map_err(|err| zip_failure(path, err))
        }
    }
}

/// A buffered reader whose seeks keep the buffer when the target lies inside
/// it. `BufReader::seek` always empties its buffer, and the `zip` crate
/// seeks between every two central-directory records, so behind a plain
/// `BufReader` a 3,000-entry collection re-reads 64 KiB per record — about
/// 400 MB to list one archive.
struct SeekBuffered<R> {
    inner: BufReader<R>,
    /// The logical position: where the next byte handed out comes from.
    position: u64,
}

impl<R: Read + Seek> SeekBuffered<R> {
    /// Wrap a reader standing at position 0 (a freshly opened file).
    fn new(inner: R) -> Self {
        SeekBuffered {
            inner: BufReader::with_capacity(READ_BUFFER, inner),
            position: 0,
        }
    }
}

impl<R: Read> Read for SeekBuffered<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        self.position += read as u64;
        Ok(read)
    }
}

impl<R: Read + Seek> Seek for SeekBuffered<R> {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let target = match to {
            SeekFrom::Start(target) => target,
            SeekFrom::Current(delta) => {
                self.position.checked_add_signed(delta).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "seek to a negative or overflowing position",
                    )
                })?
            }
            SeekFrom::End(_) => {
                self.position = self.inner.seek(to)?;
                return Ok(self.position);
            }
        };
        let delta = i64::try_from(i128::from(target) - i128::from(self.position))
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "seek distance overflows"))?;
        self.inner.seek_relative(delta)?;
        self.position = target;
        Ok(target)
    }

    fn stream_position(&mut self) -> io::Result<u64> {
        Ok(self.position)
    }
}

/// A directory entry: a trailing slash, or (for tools that leave the slash
/// off) an empty entry whose attributes say directory.
fn is_directory(entry: &ZipFileEntry<'_>) -> bool {
    entry.is_dir()
        || (entry.size() == 0
            && entry
                .unix_mode()
                .is_some_and(|mode| mode & S_IFMT == S_IFDIR))
}

fn is_link(entry: &ZipFileEntry<'_>) -> bool {
    entry
        .unix_mode()
        .is_some_and(|mode| mode & S_IFMT == S_IFLNK)
}

fn zip_failure(path: &Path, err: ZipError) -> RiftError {
    match err {
        ZipError::Io(err) => fail::structure_read(path, &err),
        ZipError::InvalidArchive(detail) => RiftError::corrupt(path, detail),
        ZipError::UnsupportedArchive(detail) if detail == ZipError::PASSWORD_REQUIRED => {
            fail::encrypted(path)
        }
        ZipError::InvalidPassword => fail::encrypted(path),
        ZipError::UnsupportedArchive(detail) => {
            log::warn!("{}: unsupported zip feature: {detail}", path.display());
            fail::unsupported(path)
        }
        ZipError::CompressionMethodNotSupported(method) => {
            log::warn!(
                "{}: uses zip compression method {method}, which the Riftstone cannot unpack",
                path.display()
            );
            fail::unsupported(path)
        }
        ZipError::FileNotFound => {
            RiftError::corrupt(path, "an entry listed in the zip directory is missing")
        }
        other => RiftError::corrupt(path, other),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::io::{Cursor, Write};
    use std::rc::Rc;

    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    use super::*;

    /// A reader that counts the bytes it hands out.
    struct Counted<R> {
        inner: R,
        read: Rc<Cell<u64>>,
    }

    impl<R: Read> Read for Counted<R> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let read = self.inner.read(buf)?;
            self.read.set(self.read.get() + read as u64);
            Ok(read)
        }
    }

    impl<R: Seek> Seek for Counted<R> {
        fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
            self.inner.seek(to)
        }
    }

    fn zip_bytes(stub: &[u8], files: &[(String, Vec<u8>)]) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        for (name, bytes) in files {
            writer.start_file(name.as_str(), stored).unwrap();
            writer.write_all(bytes).unwrap();
        }
        let archive = writer.finish().unwrap().into_inner();
        [stub, archive.as_slice()].concat()
    }

    fn counted(bytes: &[u8], read: &Rc<Cell<u64>>) -> SeekBuffered<Counted<Cursor<Vec<u8>>>> {
        SeekBuffered::new(Counted {
            inner: Cursor::new(bytes.to_vec()),
            read: Rc::clone(read),
        })
    }

    #[test]
    fn a_zip_starting_at_byte_zero_is_opened_from_its_end_alone() {
        let files = vec![
            (
                "Lantern Glow/modinfo.ini".to_string(),
                b"name=Lantern Glow".to_vec(),
            ),
            (
                "Lantern Glow/natives/stm/glow.tex".to_string(),
                vec![0x5A; 8 << 20],
            ),
        ];
        let bytes = zip_bytes(b"", &files);
        let read = Rc::new(Cell::new(0));
        let archive =
            read_directory(Path::new("Lantern Glow.zip"), || Ok(counted(&bytes, &read))).unwrap();
        assert_eq!(archive.len(), 2);
        assert_eq!(archive.offset(), 0);
        assert!(
            read.get() < 256 * 1024,
            "opening read {} of the archive's {} bytes",
            read.get(),
            bytes.len()
        );
    }

    #[test]
    fn listing_a_large_collection_reads_its_directory_about_once() {
        let files: Vec<(String, Vec<u8>)> = (0..3_000)
            .map(|n| {
                let name =
                    format!("Vernworth Wardrobe Collection/Option {n:04}/natives/stm/cloak.mesh");
                (name, vec![b'm'; 16])
            })
            .collect();
        let bytes = zip_bytes(b"", &files);
        let read = Rc::new(Cell::new(0));
        let archive = read_directory(Path::new("Vernworth Wardrobe Collection.zip"), || {
            Ok(counted(&bytes, &read))
        })
        .unwrap();
        assert_eq!(archive.len(), 3_000);
        let directory = bytes.len() as u64 - archive.central_directory_start();
        assert!(
            read.get() < 4 * directory + 4 * READ_BUFFER as u64,
            "listing read {} bytes for a {directory}-byte directory",
            read.get()
        );
    }

    #[test]
    fn a_zip_behind_a_program_stub_is_found_by_searching() {
        let stub = [b"MZ".as_slice(), &[0_u8; 70_000]].concat();
        let files = vec![(
            "Lantern Glow/modinfo.ini".to_string(),
            b"name=Lantern Glow".to_vec(),
        )];
        let bytes = zip_bytes(&stub, &files);
        let mut attempts = 0;
        let mut archive = read_directory(Path::new("Lantern Glow Setup.exe"), || {
            attempts += 1;
            Ok(SeekBuffered::new(Cursor::new(bytes.clone())))
        })
        .unwrap();
        assert_eq!(attempts, 2, "byte 0 first, then the search");
        assert_eq!(archive.offset(), stub.len() as u64);
        let mut text = String::new();
        archive
            .by_index(0)
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert_eq!(text, "name=Lantern Glow");
    }

    #[test]
    fn something_that_is_no_zip_at_all_reports_the_searchs_verdict() {
        let bytes = vec![0x11_u8; 4_096];
        let err = read_directory(Path::new("Lantern Glow.zip"), || {
            Ok(SeekBuffered::new(Cursor::new(bytes.clone())))
        })
        .err()
        .expect("bytes that hold no zip");
        assert!(matches!(err, RiftError::CorruptArchive { .. }), "{err:?}");
    }

    #[test]
    fn seeks_inside_and_outside_the_buffer_land_where_they_say() {
        let bytes: Vec<u8> = (0..300_000_u32).map(|n| (n % 251) as u8).collect();
        let mut reader = SeekBuffered::new(Cursor::new(bytes.clone()));
        let mut chunk = [0_u8; 10];
        let mut check = |reader: &mut SeekBuffered<Cursor<Vec<u8>>>, at: usize| {
            assert_eq!(reader.stream_position().unwrap(), at as u64);
            reader.read_exact(&mut chunk).unwrap();
            assert_eq!(chunk, bytes[at..at + 10], "bytes at {at}");
        };
        check(&mut reader, 0);
        assert_eq!(reader.seek(SeekFrom::Start(5)).unwrap(), 5); // back, inside the buffer
        check(&mut reader, 5);
        assert_eq!(reader.seek(SeekFrom::Current(100)).unwrap(), 115); // ahead, inside
        check(&mut reader, 115);
        assert_eq!(reader.seek(SeekFrom::Start(200_000)).unwrap(), 200_000); // far ahead
        check(&mut reader, 200_000);
        assert_eq!(reader.seek(SeekFrom::Current(-150_000)).unwrap(), 50_010); // far back
        check(&mut reader, 50_010);
        assert_eq!(reader.seek(SeekFrom::End(-10)).unwrap(), 299_990);
        check(&mut reader, 299_990);
        assert_eq!(reader.seek(SeekFrom::Current(-20)).unwrap(), 299_980); // after End
        check(&mut reader, 299_980);
        assert!(reader.seek(SeekFrom::Current(-400_000)).is_err());
        assert_eq!(
            reader.stream_position().unwrap(),
            299_990,
            "a refused seek moves nothing"
        );
    }
}
