//! 7z packages, through the pure-Rust `sevenz-rust2` (LZMA, LZMA2, the BCJ
//! filters, delta, copy — the workspace builds it without AES, PPMd or BZip2).
//!
//! **Blocks.** A 7z archive is a list of blocks ("folders"); a solid block
//! packs many files into one compressed stream, a non-solid archive has one
//! block per file. Blocks decode independently, so reading one entry decodes
//! only its own block, from the block's start up to the entry, and stops.
//! `extract` decodes each needed block once, in archive order, writing the
//! requested files as they pass and stopping after the last one.
//!
//! **Three traps in the decoder, handled here.**
//! - Every file of a block reads from one shared stream: a file the pass does
//!   not want must still be drained, or the next file's bytes would start
//!   inside it.
//! - `BlockDecoder` walks a block by counting *files* from the block's first
//!   one, while 7-Zip counts *streams*. An empty file or folder listed inside
//!   a block's run would push that block's last files out of reach. 7-Zip
//!   never writes that layout; another tool could. Such files are flagged at
//!   open and reported as damaged rather than silently skipped.
//! - A file is bounded to its promised size from above, not from below: the
//!   LZMA2 decoder ends the stream at an end marker whatever the archive
//!   promised, and the checksum is tested only once every promised byte has
//!   arrived. A damaged stream would read as a short or empty file without a
//!   word, so every file is read through [`Promised`], which calls an early
//!   end what it is: cut off.
//!
//! **Where it broke.** A decode failure names the file whose bytes were being
//! read when it happened — as precise as a shared stream allows. A read that
//! never reached its entry says both: where the stream broke, and that the
//! entry lies past that point. Anything that breaks *after* the entry asked
//! for is in hand belongs to the siblings the pass went on for, never to the
//! read (see [`crate::cache`]).
//!
//! **Encryption** is visible without a password: a block whose coders include
//! AES-256 holds encrypted files. An encrypted header (`7z -mhe`) cannot even
//! be listed, so that package is `Encrypted` outright.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};

use rift_core::{
    ArchiveEntry, ExtractItem, ExtractProgress, PackageFormat, PackageSource, RiftError,
};
use sevenz_rust2::{Archive, ArchiveEntry as SevenZipEntry, BlockDecoder, EncoderMethod, Password};

use crate::cache::{SiblingCache, SMALL_ENTRY_MAX};
use crate::fail;
use crate::index::{EntryIndex, IndexBuilder};
use crate::write::{duplicate, prefix, read_capped, DestWriter, Tally};

const READ_BUFFER: usize = 64 * 1024;

/// Decoder threads, always one. `sevenz-rust2` switches LZMA2 to its
/// multi-threaded reader for two or more, and that reader gathers compressed
/// work units and decodes them ahead on worker threads: a stream written by a
/// single-threaded encoder is ONE unit, so the whole block would be read and
/// decoded before the first byte came back. That would defeat `limit` (a
/// pak's 16-byte head would cost its whole block) and hold a multi-gigabyte
/// solid block in memory. One thread decodes as it is read, and stops when
/// reading stops.
const DECODER_THREADS: u32 = 1;

/// Windows attribute bits 7-Zip stores per file.
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
/// p7zip's marker: the high 16 bits hold a Unix mode.
const FILE_ATTRIBUTE_UNIX_EXTENSION: u32 = 0x8000;
const S_IFMT: u32 = 0o170_000;
const S_IFLNK: u32 = 0o120_000;

/// Where a file's bytes live.
#[derive(Debug, Clone, Copy)]
struct Slot {
    /// Index into `Archive::files`.
    file: usize,
    /// The block holding its data; `None` for an empty file.
    block: Option<usize>,
    /// False when `BlockDecoder` cannot reach it (see the module docs).
    reachable: bool,
}

/// One block's run of files, as `BlockDecoder` walks it.
#[derive(Debug, Clone, Copy)]
struct BlockRun {
    first: usize,
    len: usize,
    encrypted: bool,
}

/// A block decode that stopped short.
#[derive(Debug)]
struct Interrupted {
    /// The file (an index into `Archive::files`) whose bytes were being read
    /// when the decoder failed; `None` when the block could not be decoded at
    /// all.
    file: Option<usize>,
    /// The failure in the Riftstone's words, naming that file.
    error: RiftError,
}

/// A 7z archive, opened once.
pub struct SevenZipSource {
    path: PathBuf,
    reader: BufReader<File>,
    archive: Archive,
    password: Password,
    runs: Vec<BlockRun>,
    index: EntryIndex<Slot>,
    cache: SiblingCache,
    passes: usize,
}

impl std::fmt::Debug for SevenZipSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SevenZipSource")
            .field("path", &self.path)
            .field("entries", &self.index.entries().len())
            .field("blocks", &self.runs.len())
            .field("passes", &self.passes)
            .finish()
    }
}

impl SevenZipSource {
    /// Open the 7z at `path` and index its files.
    pub fn open(path: &Path) -> Result<Self, RiftError> {
        let file = File::open(path).map_err(|err| RiftError::unreadable(path, err))?;
        let mut reader = BufReader::with_capacity(READ_BUFFER, file);
        let password = Password::empty();
        let archive =
            Archive::read(&mut reader, &password).map_err(|err| sevenz_failure(path, err))?;

        let runs: Vec<BlockRun> = (0..archive.blocks.len())
            .map(|block| BlockRun {
                first: archive.stream_map.block_first_file_index[block],
                len: BlockDecoder::new(DECODER_THREADS, block, &archive, &password, &mut reader)
                    .entry_count(),
                encrypted: archive.blocks[block]
                    .coders
                    .iter()
                    .any(|coder| coder.encoder_method_id() == EncoderMethod::ID_AES256_SHA256),
            })
            .collect();

        let mut builder = IndexBuilder::new(path);
        for (number, file) in archive.files.iter().enumerate() {
            if file.is_directory() || file.is_anti_item() {
                continue;
            }
            if is_link(file) {
                log::warn!(
                    "{}: skipped {:?}: it is a link, and links are never installed",
                    path.display(),
                    file.name()
                );
                continue;
            }
            let block = if file.has_stream() && file.size() > 0 {
                archive
                    .stream_map
                    .file_block_index
                    .get(number)
                    .copied()
                    .flatten()
            } else {
                None
            };
            let (encrypted, reachable) = match block {
                Some(block) => {
                    let run = runs[block];
                    (
                        run.encrypted,
                        (run.first..run.first + run.len).contains(&number),
                    )
                }
                None => (false, true),
            };
            if !reachable {
                log::warn!(
                    "{}: {:?} sits outside the run of its block (an empty entry is listed \
                     inside the block); it will be reported as damaged if read",
                    path.display(),
                    file.name()
                );
            }
            builder.push(
                file.name(),
                file.size(),
                encrypted,
                Slot {
                    file: number,
                    block,
                    reachable,
                },
            );
        }
        let index = builder.finish();
        if index.fully_encrypted() {
            return Err(fail::encrypted(path));
        }
        Ok(SevenZipSource {
            path: path.to_path_buf(),
            reader,
            archive,
            password,
            runs,
            index,
            cache: SiblingCache::default(),
            passes: 0,
        })
    }

    /// How many block decodes this source has run so far. A diagnostic: one
    /// `extract` costs one decode per block it touches, and a scan that
    /// reads every `modinfo.ini` of a solid block costs one decode in all.
    pub fn decode_passes(&self) -> usize {
        self.passes
    }

    /// Decode one block from its start, offering each file to `visit` (its
    /// file index and a reader positioned at its bytes, which ends exactly
    /// where the file does or fails). `visit` answers whether to go on;
    /// whatever it does not read is drained so the next file starts in the
    /// right place. A failure names the file being read when it happened.
    fn decode_block(
        &mut self,
        block: usize,
        visit: &mut dyn FnMut(usize, &mut dyn Read) -> io::Result<bool>,
    ) -> Result<(), Interrupted> {
        self.passes += 1;
        let first = self.runs[block].first;
        let mut offset = 0usize;
        let mut failure: Option<(usize, io::Error)> = None;
        let decoded = BlockDecoder::new(
            DECODER_THREADS,
            block,
            &self.archive,
            &self.password,
            &mut self.reader,
        )
        .for_each_entries(&mut |entry: &SevenZipEntry, reader: &mut dyn Read| {
            let file = first + offset;
            offset += 1;
            let mut reader = Promised::new(reader, entry);
            let keep_going = match visit(file, &mut reader) {
                Ok(keep_going) => keep_going,
                Err(err) => {
                    failure = Some((file, err));
                    return Ok(false);
                }
            };
            if keep_going {
                // Drain what `visit` left: the next file starts after it.
                if let Err(err) = io::copy(&mut reader, &mut io::sink()) {
                    failure = Some((file, err));
                    return Ok(false);
                }
            }
            Ok(keep_going)
        });
        if let Some((file, err)) = failure {
            return Err(Interrupted {
                file: Some(file),
                error: stream_failure(&self.path, &self.file_label(file), &err),
            });
        }
        decoded.map(|_| ()).map_err(|err| Interrupted {
            file: None,
            error: sevenz_failure(&self.path, err),
        })
    }

    /// The name a file of the archive goes by in messages: its listed path,
    /// or, for a file the listing dropped (a link, an unsafe name, a losing
    /// duplicate), the name the archive gives it.
    fn file_label(&self, file: usize) -> String {
        match self
            .index
            .entries()
            .iter()
            .find(|entry| entry.locator.file == file)
        {
            Some(entry) => entry.path.clone(),
            None => self.archive.files.get(file).map_or_else(
                || format!("file {file}"),
                |raw| fail::entry_label(raw.name()),
            ),
        }
    }

    fn unreachable(&self, entry: &str) -> RiftError {
        RiftError::corrupt(
            &self.path,
            format!("{entry} sits outside its block's run of files and cannot be unpacked"),
        )
    }
}

impl PackageSource for SevenZipSource {
    fn format(&self) -> PackageFormat {
        PackageFormat::SevenZip
    }

    fn entries(&mut self) -> Result<Vec<ArchiveEntry>, RiftError> {
        Ok(self.index.listing())
    }

    fn read_entry(&mut self, entry: &str, limit: Option<u64>) -> Result<Vec<u8>, RiftError> {
        let (position, found) = self.index.lookup(entry)?;
        let (slot, size, name) = (found.locator, found.size, found.path.clone());
        if found.encrypted {
            return Err(fail::encrypted(&self.path));
        }
        if !slot.reachable {
            return Err(self.unreachable(&name));
        }
        let Some(block) = slot.block else {
            return Ok(Vec::new()); // an empty file has no stream to decode
        };
        if limit == Some(0) {
            return Ok(Vec::new());
        }
        if let Some(bytes) = self.cache.get(position) {
            return Ok(prefix(bytes, limit));
        }

        // A small entry is read whole (and kept), and its same-named small
        // siblings in this block come along in the same pass: every
        // modinfo.ini. A large entry is read no further than `limit`, and the
        // block decoder stops right there.
        let small = size <= SMALL_ENTRY_MAX;
        let mut siblings: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        if small {
            for sibling in self.cache.siblings(&self.index, position, |entry| {
                entry.locator.block == Some(block) && entry.locator.reachable
            }) {
                siblings
                    .entry(self.index.get(sibling).locator.file)
                    .or_default()
                    .push(sibling);
            }
        }
        let last = siblings
            .keys()
            .next_back()
            .map_or(slot.file, |&file| file.max(slot.file));
        let read_limit = if small { None } else { limit };
        let mut answer: Option<Vec<u8>> = None;
        let mut captured: Vec<(Vec<usize>, Vec<u8>)> = Vec::new();
        let pass = self.decode_block(block, &mut |file, reader| {
            if file == slot.file {
                answer = Some(read_capped(reader, read_limit, size)?);
            } else if let Some(positions) = siblings.remove(&file) {
                let mut bytes = Vec::new();
                reader.read_to_end(&mut bytes)?;
                captured.push((positions, bytes));
            }
            Ok(file < last)
        });

        // Every file read whole arrived in full and passed its checksum (when
        // the archive records one) as its last byte came in: keep it, even
        // when the pass broke further on. The entry asked for goes first;
        // the sibling budget left room for it.
        let answer = answer.map(|bytes| {
            if small {
                let asked = prefix(&bytes, limit);
                self.cache.keep(position, bytes);
                asked
            } else {
                bytes
            }
        });
        for (positions, bytes) in captured {
            for kept in positions {
                self.cache.keep(kept, bytes.clone());
            }
        }
        match (answer, pass) {
            // Whatever broke past the entry asked for broke on the way to a
            // sibling: the sibling's loss, never this read's.
            (Some(bytes), _) => Ok(bytes),
            (
                None,
                Err(Interrupted {
                    file: Some(file),
                    error,
                }),
            ) if file != slot.file => Err(fail::behind(error, &name)),
            (None, Err(interrupted)) => Err(interrupted.error),
            (None, Ok(())) => Err(self.unreachable(&name)),
        }
    }

    fn extract(
        &mut self,
        items: &[ExtractItem],
        progress: &mut dyn FnMut(ExtractProgress),
    ) -> Result<(), RiftError> {
        let positions = self.index.resolve(&self.path, items)?;
        for &position in &positions {
            let found = self.index.get(position);
            if !found.locator.reachable {
                return Err(self.unreachable(&found.path));
            }
        }
        let mut tally = Tally::new(progress, items.len(), self.index.total_size(&positions));

        // Empty files have no stream: write them first, nothing to decode.
        // Then every block that holds a requested file, in archive order.
        let mut by_block: BTreeMap<usize, BTreeMap<usize, Vec<usize>>> = BTreeMap::new();
        for (item, &position) in positions.iter().enumerate() {
            let found = self.index.get(position);
            match found.locator.block {
                None => {
                    let empty = DestWriter::create(&items[item].dest, &found.path)?;
                    tally.file_done(empty.finish()?);
                }
                Some(block) => by_block
                    .entry(block)
                    .or_default()
                    .entry(found.locator.file)
                    .or_default()
                    .push(item),
            }
        }

        for (block, mut files) in by_block {
            let last = files.keys().next_back().copied().unwrap_or(0);
            let mut outcome: Result<(), RiftError> = Ok(());
            let package = self.path.clone();
            let names: BTreeMap<usize, String> = files
                .keys()
                .map(|&file| (file, self.file_label(file)))
                .collect();
            self.decode_block(block, &mut |file, reader| {
                let Some(requested) = files.remove(&file) else {
                    return Ok(file < last);
                };
                let entry = names.get(&file).map_or("", String::as_str);
                let first = &items[requested[0]].dest;
                match write_block_file(&package, first, entry, reader) {
                    Ok(written) => tally.file_done(written),
                    Err(err) => {
                        outcome = Err(err);
                        return Ok(false);
                    }
                }
                for &item in &requested[1..] {
                    match duplicate(first, &items[item].dest, entry) {
                        Ok(written) => tally.file_done(written),
                        Err(err) => {
                            outcome = Err(err);
                            return Ok(false);
                        }
                    }
                }
                Ok(file < last)
            })
            .map_err(|interrupted| interrupted.error)?;
            outcome?;
            if let Some((&file, _)) = files.iter().next() {
                return Err(self.unreachable(&self.file_label(file)));
            }
        }
        Ok(())
    }
}

/// Write one file from a block's stream. A read failure inside the stream is
/// the package's fault; a write failure is the destination's.
fn write_block_file(
    package: &Path,
    dest: &Path,
    entry: &str,
    reader: &mut dyn Read,
) -> Result<u64, RiftError> {
    crate::write::write_from_reader(dest, entry, reader, &|err| {
        stream_failure(package, entry, err)
    })
}

/// A read failure inside a block's stream, naming the entry being read.
fn stream_failure(package: &Path, entry: &str, err: &io::Error) -> RiftError {
    if checksum_failed(err) {
        RiftError::corrupt(package, format!("{entry} failed its checksum test"))
    } else {
        fail::entry_read(package, entry, err)
    }
}

/// One file's reader inside a block, held to the size the archive promised:
/// an end before the last promised byte is `UnexpectedEof` (the file is cut
/// off), never a short read that looks like the whole file. See the module
/// docs for why the decoder underneath does not notice by itself.
struct Promised<'r> {
    inner: &'r mut dyn Read,
    remaining: u64,
}

impl<'r> Promised<'r> {
    fn new(inner: &'r mut dyn Read, file: &SevenZipEntry) -> Self {
        // `BlockDecoder` hands a file without a stream an empty reader.
        let promised = if file.has_stream() { file.size() } else { 0 };
        Promised {
            inner,
            remaining: promised,
        }
    }
}

impl Read for Promised<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        if read == 0 && !buf.is_empty() && self.remaining > 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!(
                    "the stream ended {} bytes before the file did",
                    self.remaining
                ),
            ));
        }
        self.remaining = self.remaining.saturating_sub(read as u64);
        Ok(read)
    }
}

/// A 7z symlink: a Unix mode marked as a link, or a Windows reparse point.
fn is_link(file: &SevenZipEntry) -> bool {
    if !file.has_windows_attributes {
        return false;
    }
    let attributes = file.windows_attributes();
    let unix_link =
        attributes & FILE_ATTRIBUTE_UNIX_EXTENSION != 0 && (attributes >> 16) & S_IFMT == S_IFLNK;
    unix_link || attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

/// Did the decoder's CRC check fail? (`sevenz-rust2` wraps its own error in
/// an `io::Error`.)
fn checksum_failed(err: &io::Error) -> bool {
    err.get_ref()
        .and_then(|inner| inner.downcast_ref::<sevenz_rust2::Error>())
        .is_some_and(|inner| matches!(inner, sevenz_rust2::Error::ChecksumVerificationFailed))
}

fn sevenz_failure(path: &Path, err: sevenz_rust2::Error) -> RiftError {
    use sevenz_rust2::Error;
    match err {
        Error::PasswordRequired | Error::MaybeBadPassword(_) => fail::encrypted(path),
        Error::UnsupportedCompressionMethod(method) if method == "AES256_SHA256" => {
            fail::encrypted(path)
        }
        Error::UnsupportedCompressionMethod(method) => {
            log::warn!(
                "{}: uses the 7z method {method}, which this build of the Riftstone cannot unpack",
                path.display()
            );
            fail::unsupported(path)
        }
        Error::UnsupportedVersion { .. }
        | Error::ExternalUnsupported
        | Error::Unsupported(_)
        | Error::MaxMemLimited { .. } => {
            log::warn!("{}: unsupported 7z feature: {err}", path.display());
            fail::unsupported(path)
        }
        Error::Io(err, _) | Error::FileOpen(err, _) => {
            if checksum_failed(&err) {
                RiftError::corrupt(path, "a block failed its checksum test")
            } else {
                fail::structure_read(path, &err)
            }
        }
        Error::ChecksumVerificationFailed | Error::NextHeaderCrcMismatch => {
            RiftError::corrupt(path, "the archive failed its checksum test")
        }
        Error::FileNotFound => {
            RiftError::corrupt(path, "an entry listed in the archive is missing")
        }
        other => RiftError::corrupt(path, other),
    }
}
