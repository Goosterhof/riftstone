//! RAR packages (RAR4 and RAR5), through the official UnRAR sources.
//!
//! RAR has no random access: every read walks the archive from its first
//! header. In a non-solid archive skipping an entry is a seek; in a solid one
//! it means decompressing it. So:
//!
//! - `entries` comes from a list-mode walk at open (headers only, cheap even
//!   when solid);
//! - `read_entry` walks to its entry and streams it, cancelling UnRAR the
//!   moment `limit` bytes have arrived; a small entry brings its same-named
//!   small siblings along in the same walk (see [`crate::cache`]);
//! - `extract` is ONE walk in archive order, writing each requested entry as
//!   it passes and stopping after the last.
//!
//! **Siblings are a favour.** A walk skips the entries it does not want. In
//! a non-solid archive a skip is a seek and tests nothing; in a solid one it
//! is an unpack that UnRAR's DLL tests like any other (the DLL matches every
//! file, dll.cpp `FileArgs "*"`, so extract.cpp's untested `SkipSolid` path
//! never runs: a solid skip is a test without the data callback). Streaming
//! a sibling tests its checksum either way, so a damaged sibling could fail a
//! read that never needed it. It never does: whatever fails after the entry
//! asked for is in hand ends the walk and the entry is returned; a sibling
//! that fails *ahead* of it is refused and the entry gets a walk of its own,
//! which skips that sibling the plain way. Only a failure on the way to the
//! entry itself is the read's error: the entry's own data, or a skip it
//! cannot do without (in a solid archive, a damaged or encrypted entry ahead
//! of it). The error names where the walk broke.
//!
//! Entries are located by header ordinal — the position of their header in
//! the walk, not counting the continuation headers of files split across
//! volumes, which list mode never shows.
//!
//! **Links.** A symlink or junction is dropped from the listing. A hard link
//! or a stored copy (`rar -oi`) has no bytes of its own — UnRAR's test mode
//! streams nothing for it — so it is listed under its own name and served
//! from the earlier entry it points at.

use std::collections::BTreeMap;
use std::os::raw::c_int;
use std::path::{Path, PathBuf};

use rift_core::{
    ArchiveEntry, ExtractItem, ExtractProgress, PackageFormat, PackageSource, RiftError,
};
use unrar_sys as sys;

use crate::cache::{SiblingCache, SMALL_ENTRY_MAX};
use crate::fail;
use crate::index::{EntryIndex, IndexBuilder};
use crate::unrar_dll::{self, Failure, Header, Link, Mode, RarArchive, Streamed};
use crate::write::{duplicate, prefix, DestWriter, Tally};

/// Where an entry's bytes live.
#[derive(Debug, Clone, Copy)]
struct Slot {
    /// Header ordinal of the entry that holds the bytes (for a link: of the
    /// entry it points at).
    header: usize,
    /// False when the archive does not record the unpacked size.
    size_known: bool,
}

/// A RAR archive. No UnRAR handle stays open between calls.
pub struct RarSource {
    path: PathBuf,
    index: EntryIndex<Slot>,
    solid: bool,
    cache: SiblingCache,
    passes: usize,
}

impl std::fmt::Debug for RarSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RarSource")
            .field("path", &self.path)
            .field("entries", &self.index.entries().len())
            .field("solid", &self.solid)
            .field("passes", &self.passes)
            .finish()
    }
}

impl RarSource {
    /// Open the RAR at `path` and index its files (headers only).
    pub fn open(path: &Path) -> Result<Self, RiftError> {
        // UnRAR's listing cannot tell a whole archive from a cut-off one;
        // the frame walk can (see `rar_frame`).
        crate::rar_frame::check(path)?;
        let mut archive =
            RarArchive::open(path, Mode::List).map_err(|failure| rar_failure(path, failure))?;
        let solid = archive.is_solid();
        let mut builder = IndexBuilder::new(path);
        let mut ordinal = 0usize;
        while let Some(header) = archive
            .read_header()
            .map_err(|failure| rar_failure(path, failure))?
        {
            if header.split_before {
                // A continuation from an earlier volume: not an entry.
                archive
                    .skip()
                    .map_err(|failure| rar_failure(path, failure))?;
                continue;
            }
            index_header(path, &mut builder, &header, ordinal);
            archive
                .skip()
                .map_err(|failure| rar_failure(path, failure))?;
            ordinal += 1;
        }
        drop(archive);
        let index = builder.finish();
        if index.fully_encrypted() {
            return Err(fail::encrypted(path));
        }
        Ok(RarSource {
            path: path.to_path_buf(),
            index,
            solid,
            cache: SiblingCache::default(),
            passes: 0,
        })
    }

    /// Entries share one compressed stream (`rar -s`).
    pub fn is_solid(&self) -> bool {
        self.solid
    }

    /// How many data walks this source has made so far (each opens the
    /// archive once and walks it in order). A diagnostic: one `extract` is
    /// one walk, and a scan that reads every `modinfo.ini` is one walk.
    pub fn decode_passes(&self) -> usize {
        self.passes
    }

    /// Walk the archive once, offering each wanted header ordinal to `take`
    /// and skipping the rest. Stops after `last`. Returns the ordinals that
    /// were never reached (an archive cut short).
    fn walk(
        &mut self,
        wanted: &BTreeMap<usize, Vec<usize>>,
        take: &mut TakeEntry<'_>,
    ) -> Result<Vec<usize>, RiftError> {
        let path = self.path.clone();
        let Some(&last) = wanted.keys().next_back() else {
            return Ok(Vec::new());
        };
        self.passes += 1;
        let mut archive = RarArchive::open(&path, Mode::Extract)
            .map_err(|failure| rar_failure(&path, failure))?;
        let mut ordinal = 0usize;
        let mut reached = 0usize;
        while ordinal <= last {
            let Some(header) = archive
                .read_header()
                .map_err(|failure| rar_failure(&path, failure))?
            else {
                break;
            };
            if header.split_before {
                archive
                    .skip()
                    .map_err(|failure| rar_skip_failure(&path, &header.name, failure))?;
                continue;
            }
            match wanted.get(&ordinal) {
                Some(positions) => {
                    reached += 1;
                    if take(&mut archive, ordinal, positions)? == Flow::Stop {
                        return Ok(Vec::new());
                    }
                }
                None => archive
                    .skip()
                    .map_err(|failure| rar_skip_failure(&path, &header.name, failure))?,
            }
            ordinal += 1;
        }
        if reached == wanted.len() {
            Ok(Vec::new())
        } else {
            Ok(wanted.keys().copied().filter(|&o| o >= ordinal).collect())
        }
    }

    fn cut_short(&self, entry: &str) -> RiftError {
        RiftError::corrupt(
            &self.path,
            format!("the archive ends before {entry}, which its own listing promised"),
        )
    }

    /// One walk for the entry at `position` (named `name`, located by
    /// `slot`). A `small` entry is streamed whole and kept, with its
    /// same-named small siblings; any other is streamed no further than
    /// `limit`. Returns its bytes, [`Attempt::Again`] when a sibling ahead of
    /// it failed and stopped the walk short of it (that sibling is refused
    /// now), or the failure that kept the entry itself out of reach.
    fn attempt(
        &mut self,
        position: usize,
        slot: Slot,
        small: bool,
        limit: Option<u64>,
        name: &str,
    ) -> Result<Attempt, RiftError> {
        let mut wanted: BTreeMap<usize, Vec<usize>> =
            BTreeMap::from([(slot.header, vec![position])]);
        if small {
            for sibling in self
                .cache
                .siblings(&self.index, position, |entry| entry.locator.size_known)
            {
                wanted
                    .entry(self.index.get(sibling).locator.header)
                    .or_default()
                    .push(sibling);
            }
        }
        let path = self.path.clone();
        // The entry asked for, and every position that shares its bytes.
        let mut answer: Option<(Vec<usize>, Vec<u8>)> = None;
        let mut captured: Vec<(Vec<usize>, Vec<u8>)> = Vec::new();
        let mut lost: Vec<usize> = Vec::new();
        let mut target_failed = false;
        let walked = self.walk(&wanted, &mut |archive, ordinal, positions| {
            if ordinal != slot.header {
                // A sibling. Its failure spends the handle, so the walk ends
                // here; it is not this read's failure.
                return Ok(match stream_whole(archive) {
                    Ok(bytes) => {
                        captured.push((positions.to_vec(), bytes));
                        Flow::Continue
                    }
                    Err(_) => {
                        lost.extend_from_slice(positions);
                        Flow::Stop
                    }
                });
            }
            let read = if small {
                stream_whole(archive)
            } else {
                stream_head(archive, limit)
            };
            match read {
                Ok(bytes) => {
                    answer = Some((positions.to_vec(), bytes));
                    // Siblings may follow a small entry; a large one is the
                    // only entry its walk wants.
                    Ok(if small { Flow::Continue } else { Flow::Stop })
                }
                Err(failure) => {
                    target_failed = true;
                    Err(failure.into_error(&path, name))
                }
            }
        });

        // Every entry streamed whole passed UnRAR's checksum test: keep it,
        // even when the walk failed further on. The entry asked for goes
        // first; the sibling budget left room for it.
        if small {
            if let Some((positions, bytes)) = &answer {
                for &kept in positions {
                    self.cache.keep(kept, bytes.clone());
                }
            }
        }
        for (positions, bytes) in captured {
            for kept in positions {
                self.cache.keep(kept, bytes.clone());
            }
        }
        self.cache.refuse(&lost);
        if let Some((_, bytes)) = answer {
            // Whatever failed after it failed on the way to a sibling: the
            // sibling's loss, never this read's.
            return Ok(Attempt::Read(if small {
                prefix(&bytes, limit)
            } else {
                bytes
            }));
        }
        if !lost.is_empty() {
            return Ok(Attempt::Again);
        }
        match walked {
            Err(err) if target_failed => Err(err),
            Err(err) => Err(fail::behind(err, name)),
            Ok(_) => Err(self.cut_short(name)),
        }
    }
}

/// How one walk for an entry ended.
enum Attempt {
    /// The entry's bytes.
    Read(Vec<u8>),
    /// A sibling ahead of the entry failed, which spent the walk before it
    /// reached the entry. The sibling is refused; walk again.
    Again,
}

/// Why streaming an entry failed.
enum StreamError {
    /// UnRAR failed the call.
    Dll(Failure),
    /// The entry delivered far more bytes than its header declares.
    Overlong,
}

impl StreamError {
    fn into_error(self, path: &Path, entry: &str) -> RiftError {
        match self {
            StreamError::Dll(failure) => rar_entry_failure(path, entry, failure),
            StreamError::Overlong => RiftError::corrupt(
                path,
                format!("{entry} delivered far more bytes than its header declares"),
            ),
        }
    }
}

/// Stream the entry the archive stands at, whole. UnRAR tests its checksum
/// at the end: `Ok` means these are the entry's bytes.
fn stream_whole(archive: &mut RarArchive) -> Result<Vec<u8>, StreamError> {
    let mut bytes = Vec::new();
    let streamed = archive
        .stream(&mut |chunk| {
            bytes.extend_from_slice(chunk);
            // UnRAR never delivers more than the declared size; this only
            // guards the cache against a header that lies.
            bytes.len() as u64 <= 2 * SMALL_ENTRY_MAX
        })
        .map_err(StreamError::Dll)?;
    match streamed {
        Streamed::Complete => Ok(bytes),
        Streamed::Stopped => Err(StreamError::Overlong),
    }
}

/// Stream the entry the archive stands at until `limit` bytes are in hand
/// (all of it for `None`), and cancel UnRAR there: decompression stops.
fn stream_head(archive: &mut RarArchive, limit: Option<u64>) -> Result<Vec<u8>, StreamError> {
    let mut collected = Vec::new();
    let mut remaining = limit;
    archive
        .stream(&mut |chunk| {
            let take = remaining.map_or(chunk.len(), |left| {
                usize::try_from(left).map_or(chunk.len(), |left| left.min(chunk.len()))
            });
            collected.extend_from_slice(&chunk[..take]);
            match remaining.as_mut() {
                Some(left) => {
                    *left -= take as u64;
                    *left > 0
                }
                None => true,
            }
        })
        .map_err(StreamError::Dll)?;
    Ok(collected)
}

/// What a walk does with a wanted entry: the open archive (positioned at
/// its header), its header ordinal, and the index positions it feeds. It must
/// consume the entry (stream it) before answering.
type TakeEntry<'t> = dyn FnMut(&mut RarArchive, usize, &[usize]) -> Result<Flow, RiftError> + 't;

/// Whether a walk goes on after an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flow {
    Continue,
    Stop,
}

/// Index one header from the listing walk.
fn index_header(
    path: &Path,
    builder: &mut IndexBuilder<'_, Slot>,
    header: &Header,
    ordinal: usize,
) {
    if header.directory {
        return;
    }
    let size = header.size.unwrap_or(0);
    match &header.link {
        Link::None => {
            builder.push(
                &header.name,
                size,
                header.encrypted,
                Slot {
                    header: ordinal,
                    size_known: header.size.is_some(),
                },
            );
        }
        Link::CopyOf(target) => {
            let resolved = builder
                .find(target)
                .map(|entry| (entry.locator, entry.encrypted));
            match resolved {
                Some((slot, encrypted)) => {
                    builder.push(&header.name, size, encrypted, slot);
                }
                None => log::warn!(
                    "{}: skipped {:?}: it is a link to {target:?}, which is not in the package",
                    path.display(),
                    header.name
                ),
            }
        }
        Link::Symlink => log::warn!(
            "{}: skipped {:?}: it is a symbolic link or junction, and links are never installed",
            path.display(),
            header.name
        ),
        Link::Unknown(kind) => log::warn!(
            "{}: skipped {:?}: it is a file-system redirect of unknown kind {kind}",
            path.display(),
            header.name
        ),
    }
}

impl PackageSource for RarSource {
    fn format(&self) -> PackageFormat {
        PackageFormat::Rar
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
        if limit == Some(0) {
            return Ok(Vec::new());
        }
        if let Some(bytes) = self.cache.get(position) {
            return Ok(prefix(bytes, limit));
        }
        // A small entry is read whole and brings its same-named small
        // siblings along; a large one (or one of unknown size) is streamed
        // until `limit` bytes are in hand, and UnRAR is cancelled there.
        let small = slot.size_known && size <= SMALL_ENTRY_MAX;
        // Each `Again` refused at least one sibling that the next walk no
        // longer asks for, so this ends: at worst in a walk with no siblings.
        loop {
            if let Attempt::Read(bytes) = self.attempt(position, slot, small, limit, &name)? {
                return Ok(bytes);
            }
        }
    }

    fn extract(
        &mut self,
        items: &[ExtractItem],
        progress: &mut dyn FnMut(ExtractProgress),
    ) -> Result<(), RiftError> {
        let positions = self.index.resolve(&self.path, items)?;
        let mut tally = Tally::new(progress, items.len(), self.index.total_size(&positions));

        // Header ordinal → the items it feeds (a link and its target share
        // one ordinal; the same entry may be asked for twice).
        let mut plan: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (item, &position) in positions.iter().enumerate() {
            plan.entry(self.index.get(position).locator.header)
                .or_default()
                .push(item);
        }
        let names: Vec<String> = positions
            .iter()
            .map(|&position| self.index.get(position).path.clone())
            .collect();
        let path = self.path.clone();
        let missing = self.walk(&plan, &mut |archive, _, requested| {
            let first = requested[0];
            let dest = &items[first].dest;
            let entry = names[first].as_str();
            let mut out = DestWriter::create(dest, entry)?;
            let streamed = archive.stream(&mut |chunk| out.push(chunk));
            match streamed {
                Ok(Streamed::Complete) => tally.file_done(first, out.finish()?),
                Ok(Streamed::Stopped) => {
                    // This sink stops only when a write failed: finish()
                    // reports that error and removes the partial file.
                    let written = out.finish();
                    return Err(written.err().unwrap_or_else(|| {
                        RiftError::corrupt(&path, format!("{entry} stopped unpacking"))
                    }));
                }
                Err(failure) => {
                    out.abandon();
                    return Err(rar_entry_failure(&path, entry, failure));
                }
            }
            for &item in &requested[1..] {
                tally.file_done(
                    item,
                    duplicate(dest, &items[item].dest, names[item].as_str())?,
                );
            }
            Ok(Flow::Continue)
        })?;
        if let Some(ordinal) = missing.first() {
            let entry = plan
                .get(ordinal)
                .and_then(|items| items.first())
                .map_or("an entry", |&item| names[item].as_str());
            return Err(self.cut_short(entry));
        }
        Ok(())
    }
}

/// A DLL failure, in the Riftstone's words, naming the package.
fn rar_failure(path: &Path, failure: Failure) -> RiftError {
    describe_failure(path, "", failure)
}

/// As [`rar_failure`], naming the entry being unpacked.
fn rar_entry_failure(path: &Path, entry: &str, failure: Failure) -> RiftError {
    describe_failure(path, &format!(" while unpacking {entry}"), failure)
}

/// As [`rar_failure`], naming the entry a walk was skipping (in a solid
/// archive, skipping an entry means unpacking it without keeping a byte).
fn rar_skip_failure(path: &Path, raw_name: &str, failure: Failure) -> RiftError {
    describe_failure(
        path,
        &format!(" while skipping {}", fail::entry_label(raw_name)),
        failure,
    )
}

/// The DLL failure, with `about` (empty, or ` while unpacking <entry>`)
/// appended.
fn describe_failure(path: &Path, about: &str, failure: Failure) -> RiftError {
    if failure.password_asked
        || failure.code == sys::ERAR_MISSING_PASSWORD
        || failure.code == sys::ERAR_BAD_PASSWORD
    {
        return fail::encrypted(path);
    }
    if failure.volume_missing {
        return RiftError::unreadable(
            path,
            format!("the next part of this multi-part archive is missing{about}"),
        );
    }
    if failure.panicked {
        return RiftError::corrupt(path, format!("reading stopped unexpectedly{about}"));
    }
    let name = unrar_dll::code_name(failure.code);
    let code: c_int = failure.code;
    match code {
        sys::ERAR_EOPEN | sys::ERAR_EREAD => {
            RiftError::unreadable(path, format!("UnRAR: {name}{about}"))
        }
        sys::ERAR_NO_MEMORY => {
            RiftError::unreadable(path, format!("not enough memory to unpack it{about}"))
        }
        sys::ERAR_UNKNOWN_FORMAT | unrar_dll::ERAR_LARGE_DICT => {
            log::warn!("{}: UnRAR: {name}{about}", path.display());
            fail::unsupported(path)
        }
        sys::ERAR_ECREATE | sys::ERAR_EWRITE | sys::ERAR_ECLOSE => {
            RiftError::io(path.display(), format!("UnRAR: {name}{about}"))
        }
        _ => RiftError::corrupt(path, format!("UnRAR: {name}{about}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failure(code: c_int) -> Failure {
        Failure {
            code,
            volume_missing: false,
            password_asked: false,
            panicked: false,
        }
    }

    #[test]
    fn dll_codes_become_the_riftstone_vocabulary() {
        let path = Path::new("Vernworth Cloaks Collection.rar");
        assert!(matches!(
            rar_failure(path, failure(sys::ERAR_MISSING_PASSWORD)),
            RiftError::Encrypted { .. }
        ));
        assert!(matches!(
            rar_failure(path, failure(sys::ERAR_BAD_PASSWORD)),
            RiftError::Encrypted { .. }
        ));
        assert!(matches!(
            rar_failure(path, failure(sys::ERAR_BAD_DATA)),
            RiftError::CorruptArchive { .. }
        ));
        assert!(matches!(
            rar_failure(path, failure(sys::ERAR_BAD_ARCHIVE)),
            RiftError::CorruptArchive { .. }
        ));
        assert!(matches!(
            rar_failure(path, failure(sys::ERAR_EOPEN)),
            RiftError::Unreadable { .. }
        ));
        assert!(matches!(
            rar_failure(path, failure(sys::ERAR_UNKNOWN_FORMAT)),
            RiftError::UnsupportedFormat { .. }
        ));
        let missing_volume = Failure {
            volume_missing: true,
            ..failure(sys::ERAR_EOPEN)
        };
        assert!(matches!(
            rar_failure(path, missing_volume),
            RiftError::Unreadable { detail, .. } if detail.contains("multi-part")
        ));
        let asked = Failure {
            password_asked: true,
            ..failure(sys::ERAR_UNKNOWN)
        };
        assert!(matches!(
            rar_failure(path, asked),
            RiftError::Encrypted { .. }
        ));
    }

    #[test]
    fn skip_failures_name_the_skipped_entry_normalized() {
        let err = rar_skip_failure(
            Path::new("Vernworth Cloaks Collection.rar"),
            r"Cloaks\Ermine Cape\natives\stm\cape.mesh",
            failure(sys::ERAR_BAD_DATA),
        );
        assert!(
            matches!(&err, RiftError::CorruptArchive { detail, .. }
                if detail.ends_with("while skipping Cloaks/Ermine Cape/natives/stm/cape.mesh")),
            "{err:?}"
        );
    }

    #[test]
    fn entry_failures_name_the_entry() {
        let err = rar_entry_failure(
            Path::new("Vernworth Cloaks Collection.rar"),
            "Cloaks/Ermine Cape/natives/stm/cape.mesh",
            failure(sys::ERAR_BAD_DATA),
        );
        assert!(err.to_string().contains("cape.mesh"), "{err}");
        assert!(
            err.to_string().contains("Vernworth Cloaks Collection.rar"),
            "{err}"
        );
    }
}
