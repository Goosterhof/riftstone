//! The KPKA header and the v4 entry table. The crate docs carry the layout
//! and the sources it was checked against.

use std::io::{Read, Seek, SeekFrom};

use crate::{
    PakEntry, PakError, PakReadError, PakToc, PathHash, ENTRY_LEN, FLAG_ENCRYPTED_TABLE,
    HEADER_LEN, MAGIC, MAX_ENTRIES,
};

/// Entries [`read_toc_from`] pulls per read: 48 KiB at a time, so the raw
/// table never sits in memory beside the entries parsed from it.
const ENTRIES_PER_READ: usize = 1024;

/// The 16-byte header, decoded. The fingerprint (bytes 12..16) is not kept.
struct Header {
    major: u8,
    minor: u8,
    feature_flags: u16,
    file_count: u32,
}

impl Header {
    fn encrypted(&self) -> bool {
        self.feature_flags & FLAG_ENCRYPTED_TABLE != 0
    }

    /// Refuse a plain table that claims more than [`MAX_ENTRIES`] files: the
    /// header is damaged. An encrypted table is never read, so its count is
    /// not judged.
    fn check_count(&self) -> Result<(), PakError> {
        if !self.encrypted() && self.file_count > MAX_ENTRIES {
            return Err(PakError::TooManyEntries {
                claimed: self.file_count,
                limit: MAX_ENTRIES,
            });
        }
        Ok(())
    }

    /// Bytes from the start of the pak to the end of the table of contents.
    /// At most `16 + 48 × u32::MAX`, so it cannot overflow a u64.
    fn toc_len(&self) -> u64 {
        if self.encrypted() {
            HEADER_LEN as u64
        } else {
            HEADER_LEN as u64 + ENTRY_LEN as u64 * u64::from(self.file_count)
        }
    }

    fn into_toc(self, entries: Vec<PakEntry>) -> PakToc {
        PakToc {
            encrypted: self.encrypted(),
            major: self.major,
            minor: self.minor,
            feature_flags: self.feature_flags,
            file_count: self.file_count,
            entries,
        }
    }
}

/// Check the magic first (whatever part of it is there), then the length,
/// then the version, so a short file that is not a pak still says so.
fn parse_header(bytes: &[u8]) -> Result<Header, PakError> {
    let seen = &bytes[..bytes.len().min(MAGIC.len())];
    if seen != &MAGIC[..seen.len()] {
        let mut magic = [0u8; 4];
        magic[..seen.len()].copy_from_slice(seen);
        return Err(PakError::NotAPak(magic));
    }
    if bytes.len() < HEADER_LEN {
        return Err(PakError::Truncated {
            needed: HEADER_LEN as u64,
            have: bytes.len() as u64,
        });
    }
    let (major, minor) = (bytes[4], bytes[5]);
    if major != 4 || minor > 2 {
        return Err(PakError::UnsupportedVersion(major, minor));
    }
    Ok(Header {
        major,
        minor,
        feature_flags: u16::from_le_bytes([bytes[6], bytes[7]]),
        file_count: u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
    })
}

/// How many bytes from the start of the pak are needed to read the whole
/// table of contents, given its 16-byte header.
///
/// `16 + 48 × file_count` for a plain table; 16 when the table is encrypted
/// (there is nothing the Riftstone can read past the header). A plain table
/// that claims more than [`MAX_ENTRIES`] files is refused as
/// [`PakError::TooManyEntries`], so a damaged header never asks a caller to
/// read hundreds of megabytes. Only the first [`HEADER_LEN`] bytes of `header`
/// are looked at.
pub fn toc_len(header: &[u8]) -> Result<u64, PakError> {
    let header = parse_header(header)?;
    header.check_count()?;
    Ok(header.toc_len())
}

/// Parse the table of contents from the head of a pak (at least `toc_len` bytes).
///
/// Entries come back in table order (REE.Packer writes them sorted by
/// [`PathHash::combined`]; nothing here relies on that). An encrypted table is
/// not an error: it comes back with `encrypted: true` and no entries, even when
/// `head` holds nothing but the header. A plain table that claims more than
/// [`MAX_ENTRIES`] files is refused as [`PakError::TooManyEntries`].
pub fn read_toc(head: &[u8]) -> Result<PakToc, PakError> {
    let header = parse_header(head)?;
    if header.encrypted() {
        return Ok(header.into_toc(Vec::new()));
    }
    header.check_count()?;
    let needed = header.toc_len();
    let have = head.len() as u64;
    if have < needed {
        return Err(PakError::Truncated { needed, have });
    }
    // `needed <= head.len()`, so the cast cannot truncate.
    let table = &head[HEADER_LEN..needed as usize];
    let entries = table.chunks_exact(ENTRY_LEN).map(parse_entry).collect();
    Ok(header.into_toc(entries))
}

/// One 48-byte v4 entry. The trailing checksum (bytes 40..48) is not kept.
fn parse_entry(raw: &[u8]) -> PakEntry {
    let u32_at = |at: usize| u32::from_le_bytes([raw[at], raw[at + 1], raw[at + 2], raw[at + 3]]);
    let u64_at = |at: usize| {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&raw[at..at + 8]);
        u64::from_le_bytes(bytes)
    };
    PakEntry {
        hash: PathHash {
            lower: u32_at(0),
            upper: u32_at(4),
        },
        offset: u64_at(8),
        compressed_size: u64_at(16),
        size: u64_at(24),
        attributes: u64_at(32),
    }
}

/// Read just the table of contents of a pak, starting where `reader` stands:
/// a `.pak` in the game's `pak_mods/`, or a pak inside any seekable stream.
///
/// Reads the header, then exactly the rest of the table of contents, never
/// the payload behind it, so a 2 GB pak costs `16 + 48 × file_count` bytes.
/// The table is parsed 1,024 entries (48 KiB) at a time, so the raw table
/// never sits in memory beside the entries parsed from it. A header that
/// claims more than [`MAX_ENTRIES`] files ([`PakError::TooManyEntries`]) or
/// more table than the stream holds ([`PakError::Truncated`]) is refused
/// before anything past the header is read or allocated. On success the
/// reader is left just past the table of contents.
pub fn read_toc_from<R: Read + Seek>(reader: &mut R) -> Result<PakToc, PakReadError> {
    let start = reader.stream_position()?;
    let end = reader.seek(SeekFrom::End(0))?;
    reader.seek(SeekFrom::Start(start))?;
    let available = end.saturating_sub(start);

    // `available < HEADER_LEN` here means it is below 16, so the cast is exact.
    let head_len = if available < HEADER_LEN as u64 {
        available as usize
    } else {
        HEADER_LEN
    };
    let mut head = [0u8; HEADER_LEN];
    reader.read_exact(&mut head[..head_len])?;

    let header = parse_header(&head[..head_len])?;
    if header.encrypted() {
        return Ok(header.into_toc(Vec::new()));
    }
    header.check_count()?;
    let needed = header.toc_len();
    if needed > available {
        return Err(PakError::Truncated {
            needed,
            have: available,
        }
        .into());
    }

    // `check_count` held the count to MAX_ENTRIES (2^22), so it fits a usize.
    let count = header.file_count as usize;
    let mut entries = Vec::with_capacity(count);
    let mut chunk = vec![0u8; ENTRY_LEN * count.min(ENTRIES_PER_READ)];
    let mut left = count;
    while left > 0 {
        let take = left.min(ENTRIES_PER_READ);
        let raw = &mut chunk[..take * ENTRY_LEN];
        reader.read_exact(raw)?;
        entries.extend(raw.chunks_exact(ENTRY_LEN).map(parse_entry));
        left -= take;
    }
    Ok(header.into_toc(entries))
}

#[cfg(test)]
mod tests {
    use std::io::{self, Cursor};

    use super::*;

    /// A forged header, laid out as REE.Packer writes one.
    fn header(major: u8, minor: u8, flags: u16, file_count: u32) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(HEADER_LEN);
        bytes.extend_from_slice(&MAGIC);
        bytes.push(major);
        bytes.push(minor);
        bytes.extend_from_slice(&flags.to_le_bytes());
        bytes.extend_from_slice(&file_count.to_le_bytes());
        bytes.extend_from_slice(&0xDEC0_ADDE_u32.to_le_bytes());
        bytes
    }

    fn entry_bytes(entry: &PakEntry, checksum: u64) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(ENTRY_LEN);
        bytes.extend_from_slice(&entry.hash.lower.to_le_bytes());
        bytes.extend_from_slice(&entry.hash.upper.to_le_bytes());
        bytes.extend_from_slice(&entry.offset.to_le_bytes());
        bytes.extend_from_slice(&entry.compressed_size.to_le_bytes());
        bytes.extend_from_slice(&entry.size.to_le_bytes());
        bytes.extend_from_slice(&entry.attributes.to_le_bytes());
        bytes.extend_from_slice(&checksum.to_le_bytes());
        bytes
    }

    fn two_entries() -> Vec<PakEntry> {
        vec![
            PakEntry {
                hash: crate::path_hash("natives/stm/gui/lantern_glow.tex.251211553"),
                offset: 112,
                compressed_size: 300,
                size: 1_024,
                attributes: 1,
            },
            PakEntry {
                hash: crate::path_hash("natives/stm/character/vernworth_cloak.mesh.260421070"),
                offset: 412,
                compressed_size: 5_000,
                size: 5_000,
                attributes: 0,
            },
        ]
    }

    /// A plain v4.0 pak: header, table, then some payload bytes.
    fn plain_pak(flags: u16, entries: &[PakEntry]) -> Vec<u8> {
        let count = u32::try_from(entries.len()).unwrap();
        let mut bytes = header(4, 0, flags, count);
        for (i, entry) in entries.iter().enumerate() {
            bytes.extend_from_slice(&entry_bytes(entry, 0xC0FF_EE00 + i as u64));
        }
        bytes.extend_from_slice(&[0xAB; 64]);
        bytes
    }

    #[test]
    fn reads_a_forged_plain_pak_with_two_entries() {
        let entries = two_entries();
        let pak = plain_pak(0, &entries);
        assert_eq!(toc_len(&pak[..HEADER_LEN]), Ok(112));

        let toc = read_toc(&pak).unwrap();
        assert_eq!(
            toc,
            PakToc {
                major: 4,
                minor: 0,
                feature_flags: 0,
                file_count: 2,
                encrypted: false,
                entries: entries.clone(),
            }
        );
        // Exactly `toc_len` bytes are enough.
        assert_eq!(read_toc(&pak[..112]).unwrap(), toc);
    }

    #[test]
    fn the_published_reference_entry_parses_field_by_field() {
        // ree-pak-rs 25562e6, spec/entry.rs `test_read_write`: one real v4 entry.
        let reference: [u8; ENTRY_LEN] = [
            0x34, 0x2F, 0x6E, 0xC2, 0xEB, 0xBE, 0xE6, 0x80, 0x95, 0xFA, 0x06, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x40, 0x8A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x8A, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let mut pak = header(4, 0, 0, 1);
        pak.extend_from_slice(&reference);

        let toc = read_toc(&pak).unwrap();
        assert_eq!(
            toc.entries,
            vec![PakEntry {
                hash: PathHash {
                    lower: 3_262_000_948,
                    upper: 2_162_605_803,
                },
                offset: 457_365,
                compressed_size: 35_392,
                size: 35_392,
                attributes: 0,
            }]
        );
    }

    #[test]
    fn an_encrypted_patch_stub_reads_as_encrypted_and_empty() {
        // The 144-byte `re_chunk_000.pak.patch_NNN.pak` shape: v4.1, flags
        // 0x0008, zero files, then a 128-byte key.
        let mut stub = header(4, 1, 0x0008, 0);
        stub.extend_from_slice(&[0x5A; 128]);
        assert_eq!(stub.len(), 144);

        assert_eq!(toc_len(&stub), Ok(16));
        let toc = read_toc(&stub).unwrap();
        assert!(toc.encrypted);
        assert!(toc.entries.is_empty());
        assert_eq!((toc.major, toc.minor, toc.feature_flags), (4, 1, 0x0008));
    }

    #[test]
    fn an_encrypted_game_pak_header_alone_is_enough() {
        for flags in [0x0068, 0x0028] {
            let head = header(4, 2, flags, 412_345);
            assert_eq!(toc_len(&head), Ok(16));
            let toc = read_toc(&head).unwrap();
            assert!(toc.encrypted, "flags {flags:#06x}");
            assert!(toc.entries.is_empty());
            assert_eq!(toc.file_count, 412_345);
        }
    }

    #[test]
    fn a_short_header_is_truncated() {
        let pak = plain_pak(0, &two_entries());
        for len in [0, 3, 4, 5, 15] {
            let want = PakError::Truncated {
                needed: 16,
                have: len as u64,
            };
            assert_eq!(toc_len(&pak[..len]), Err(want.clone()), "{len} bytes");
            assert_eq!(read_toc(&pak[..len]), Err(want), "{len} bytes");
        }
    }

    #[test]
    fn a_short_table_is_truncated() {
        let pak = plain_pak(0, &two_entries());
        assert_eq!(
            read_toc(&pak[..16 + 48 + 10]),
            Err(PakError::Truncated {
                needed: 112,
                have: 74,
            })
        );
    }

    #[test]
    fn a_bad_magic_is_not_a_pak() {
        let mut zip = b"PK\x03\x04".to_vec();
        zip.extend_from_slice(&[0; 60]);
        assert_eq!(toc_len(&zip), Err(PakError::NotAPak(*b"PK\x03\x04")));
        assert_eq!(read_toc(&zip), Err(PakError::NotAPak(*b"PK\x03\x04")));

        // Even a file too short for a header says "not a pak" when its first
        // bytes already rule it out.
        assert_eq!(read_toc(b"PK"), Err(PakError::NotAPak([b'P', b'K', 0, 0])));
        assert_eq!(read_toc(b"kpka"), Err(PakError::NotAPak(*b"kpka")));
    }

    #[test]
    fn only_versions_4_0_to_4_2_are_read() {
        for minor in 0..=2 {
            assert!(read_toc(&header(4, minor, 0, 0)).is_ok(), "4.{minor}");
        }
        for (major, minor) in [(2, 0), (4, 3), (5, 0), (0, 0)] {
            assert_eq!(
                read_toc(&header(major, minor, 0, 0)),
                Err(PakError::UnsupportedVersion(major, minor))
            );
            assert_eq!(
                toc_len(&header(major, minor, 0, 0)),
                Err(PakError::UnsupportedVersion(major, minor))
            );
        }
    }

    #[test]
    fn a_huge_file_count_is_refused_without_overflow_or_allocation() {
        let head = header(4, 0, 0, u32::MAX);
        let damaged = PakError::TooManyEntries {
            claimed: u32::MAX,
            limit: MAX_ENTRIES,
        };
        assert_eq!(toc_len(&head), Err(damaged.clone()));
        assert_eq!(read_toc(&head), Err(damaged.clone()));

        // Even a stream long enough to hold the claimed table is not read.
        let mut pak = VirtualPak::new(head, 16 + 48 * u64::from(u32::MAX));
        let err = read_toc_from(&mut pak).unwrap_err();
        assert!(
            matches!(&err, PakReadError::Pak(e) if *e == damaged),
            "{err:?}"
        );
        assert_eq!(pak.bytes_read, 16);
    }

    #[test]
    fn a_count_beyond_any_dd2_pak_is_refused_before_its_table_is_read() {
        // One entry past the cap, in a stream that really holds that table:
        // only the count gives the damage away.
        let claimed = MAX_ENTRIES + 1;
        let head = header(4, 0, 0, claimed);
        let damaged = PakError::TooManyEntries {
            claimed,
            limit: MAX_ENTRIES,
        };
        assert_eq!(toc_len(&head), Err(damaged.clone()));
        assert_eq!(read_toc(&head), Err(damaged.clone()));

        let mut pak = VirtualPak::new(head, 16 + 48 * u64::from(claimed));
        let err = read_toc_from(&mut pak).unwrap_err();
        assert!(
            matches!(&err, PakReadError::Pak(e) if *e == damaged),
            "{err:?}"
        );
        assert_eq!(pak.bytes_read, 16, "nothing past the header may be read");
        assert!(pak.largest_read <= 16);
    }

    #[test]
    fn the_entry_cap_is_inclusive_and_never_judges_an_encrypted_count() {
        let at_cap = header(4, 0, 0, MAX_ENTRIES);
        assert_eq!(
            toc_len(&at_cap),
            Ok(16 + 48 * u64::from(MAX_ENTRIES)),
            "a count of exactly MAX_ENTRIES is accepted"
        );
        // Accepted, so the length check speaks next — and still nothing past
        // the header is read when the stream is one byte short.
        let short = 16 + 48 * u64::from(MAX_ENTRIES) - 1;
        let mut pak = VirtualPak::new(at_cap.clone(), short);
        let err = read_toc_from(&mut pak).unwrap_err();
        assert!(
            matches!(
                err,
                PakReadError::Pak(PakError::Truncated { needed, have })
                    if needed == short + 1 && have == short
            ),
            "{err:?}"
        );
        assert_eq!(pak.bytes_read, 16);
        assert_eq!(
            read_toc(&at_cap),
            Err(PakError::Truncated {
                needed: 16 + 48 * u64::from(MAX_ENTRIES),
                have: 16,
            })
        );

        // An encrypted table is never read, so any count is fine.
        let encrypted = header(4, 2, 0x0068, u32::MAX);
        assert_eq!(toc_len(&encrypted), Ok(16));
        let toc = read_toc(&encrypted).unwrap();
        assert!(toc.encrypted && toc.entries.is_empty());
        assert_eq!(toc.file_count, u32::MAX);
    }

    #[test]
    fn an_empty_plain_pak_has_an_empty_table() {
        let toc = read_toc(&header(4, 0, 0, 0)).unwrap();
        assert!(!toc.encrypted);
        assert!(toc.entries.is_empty());
    }

    #[test]
    fn flag_blocks_after_the_table_do_not_move_a_plain_table() {
        // 0x0004 appends nine bytes, 0x0010 a u32 and 0x0020 a chunk table
        // AFTER the entries, so the entries still start at byte 16. ree-pak-rs
        // reads all three on a plain table; REE.PAK.Tool only ever meets them
        // beside 0x0008.
        let entries = two_entries();
        for flags in [0x0010, 0x0020, 0x0004] {
            let toc = read_toc(&plain_pak(flags, &entries)).unwrap();
            assert!(!toc.encrypted);
            assert_eq!(toc.feature_flags, flags);
            assert_eq!(toc.entries, entries, "flags {flags:#06x}");
        }
    }

    #[test]
    fn every_flag_value_reads_a_plain_table_at_byte_16_and_reports_unknown_bits() {
        // Both references refuse flags they do not list; the Riftstone reads
        // through them on purpose and says which bits nobody describes:
        // everything outside 0x0004 | 0x0008 | 0x0010 | 0x0020 | 0x0040.
        let entries = two_entries();
        for flags in 0..=u16::MAX {
            let toc = read_toc(&plain_pak(flags, &entries)).unwrap();
            assert_eq!(toc.feature_flags, flags);
            assert_eq!(toc.unknown_flags(), flags & !0x007C, "flags {flags:#06x}");
            if flags & 0x0008 == 0 {
                assert!(!toc.encrypted, "flags {flags:#06x}");
                assert_eq!(toc.entries, entries, "flags {flags:#06x}");
            } else {
                assert!(toc.encrypted, "flags {flags:#06x}");
                assert!(toc.entries.is_empty(), "flags {flags:#06x}");
            }
        }

        // The ones that matter by name.
        for (flags, unknown) in [
            (0x0000, 0x0000), // every mod pak measured
            (0x0040, 0x0000), // DD2's 16-byte chunk table (REE.PAK.Tool)
            (0x0068, 0x0000), // the game's own paks
            (0x0001, 0x0001),
            (0x0002, 0x0002),
            (0x0080, 0x0080),
            (0x8000, 0x8000),
            (0x0041, 0x0001),
        ] {
            let toc = read_toc(&plain_pak(flags, &entries)).unwrap();
            assert_eq!(toc.unknown_flags(), unknown, "flags {flags:#06x}");
        }
    }

    #[test]
    fn read_toc_from_reads_the_table_and_not_the_payload() {
        let pak = plain_pak(0, &two_entries());
        let mut reader = Cursor::new(pak.clone());
        let toc = read_toc_from(&mut reader).unwrap();
        assert_eq!(toc, read_toc(&pak).unwrap());
        assert_eq!(reader.position(), 112);
    }

    #[test]
    fn read_toc_from_starts_where_the_reader_stands() {
        let pak = plain_pak(0, &two_entries());
        let mut blob = b"seven b".to_vec();
        blob.extend_from_slice(&pak);
        let mut reader = Cursor::new(blob);
        reader.set_position(7);
        assert_eq!(read_toc_from(&mut reader).unwrap(), read_toc(&pak).unwrap());
        assert_eq!(reader.position(), 7 + 112);
    }

    #[test]
    fn read_toc_from_handles_encrypted_paks() {
        let mut stub = header(4, 1, 0x0008, 0);
        stub.extend_from_slice(&[0x5A; 128]);
        let mut reader = Cursor::new(stub);
        let toc = read_toc_from(&mut reader).unwrap();
        assert!(toc.encrypted);
        assert_eq!(reader.position(), 16);
    }

    #[test]
    fn read_toc_from_refuses_a_header_that_claims_more_than_the_file_holds() {
        // A sane count (1,000 entries, 48,016 bytes of table) in a 200-byte file.
        let mut pak = VirtualPak::new(header(4, 0, 0, 1_000), 200);
        let err = read_toc_from(&mut pak).unwrap_err();
        assert!(
            matches!(
                err,
                PakReadError::Pak(PakError::Truncated {
                    needed: 48_016,
                    have: 200,
                })
            ),
            "{err:?}"
        );
        assert_eq!(pak.bytes_read, 16, "refused before reading past the header");
    }

    #[test]
    fn read_toc_from_reports_short_and_foreign_files() {
        let err = read_toc_from(&mut Cursor::new(Vec::new())).unwrap_err();
        assert!(matches!(
            err,
            PakReadError::Pak(PakError::Truncated {
                needed: 16,
                have: 0
            })
        ));

        let err = read_toc_from(&mut Cursor::new(b"KPKA\x04\x00".to_vec())).unwrap_err();
        assert!(matches!(
            err,
            PakReadError::Pak(PakError::Truncated {
                needed: 16,
                have: 6
            })
        ));

        let err = read_toc_from(&mut Cursor::new(b"Rar!\x1A\x07\x01\x00".to_vec())).unwrap_err();
        assert!(matches!(err, PakReadError::Pak(PakError::NotAPak(magic)) if &magic == b"Rar!"));

        let pak = plain_pak(0, &two_entries());
        let err = read_toc_from(&mut Cursor::new(pak[..100].to_vec())).unwrap_err();
        assert!(matches!(
            err,
            PakReadError::Pak(PakError::Truncated {
                needed: 112,
                have: 100,
            })
        ));
    }

    /// A reader whose disk gives out after `good` bytes.
    struct FailingDisk {
        inner: Cursor<Vec<u8>>,
        good: u64,
    }

    impl Read for FailingDisk {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.inner.position() >= self.good {
                return Err(io::Error::other("the disk gave out"));
            }
            let room = usize::try_from(self.good - self.inner.position()).unwrap();
            let len = buf.len().min(room);
            self.inner.read(&mut buf[..len])
        }
    }

    impl Seek for FailingDisk {
        fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
            self.inner.seek(pos)
        }
    }

    /// A pak of `len` bytes that holds `head` and then zeros, without ever
    /// allocating them: big enough for any table, light enough for a unit
    /// test. It counts what it is asked for.
    struct VirtualPak {
        head: Vec<u8>,
        len: u64,
        pos: u64,
        bytes_read: u64,
        largest_read: usize,
    }

    impl VirtualPak {
        fn new(head: Vec<u8>, len: u64) -> Self {
            VirtualPak {
                head,
                len,
                pos: 0,
                bytes_read: 0,
                largest_read: 0,
            }
        }
    }

    impl Read for VirtualPak {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.largest_read = self.largest_read.max(buf.len());
            let left = self.len.saturating_sub(self.pos);
            let n = usize::try_from(left.min(buf.len() as u64)).unwrap();
            buf[..n].fill(0);
            let start = usize::try_from(self.pos).unwrap();
            if start < self.head.len() {
                let m = (self.head.len() - start).min(n);
                buf[..m].copy_from_slice(&self.head[start..start + m]);
            }
            self.pos += n as u64;
            self.bytes_read += n as u64;
            Ok(n)
        }
    }

    impl Seek for VirtualPak {
        fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
            let to = match pos {
                SeekFrom::Start(at) => Some(at),
                SeekFrom::End(delta) => self.len.checked_add_signed(delta),
                SeekFrom::Current(delta) => self.pos.checked_add_signed(delta),
            };
            self.pos = to.ok_or_else(|| io::Error::other("seek before the start"))?;
            Ok(self.pos)
        }
    }

    #[test]
    fn read_toc_from_parses_the_table_in_bounded_reads() {
        // 10,000 entries: a 480,000-byte table that must never be asked for,
        // or held, in one piece.
        let entries: Vec<PakEntry> = (0..10_000u32)
            .map(|i| PakEntry {
                hash: PathHash {
                    lower: i,
                    upper: !i,
                },
                offset: 480_016 + u64::from(i) * 7,
                compressed_size: 7,
                size: 9,
                attributes: u64::from(i % 2),
            })
            .collect();
        let bytes = plain_pak(0, &entries);
        let len = bytes.len() as u64;
        let mut pak = VirtualPak::new(bytes.clone(), len);

        let toc = read_toc_from(&mut pak).unwrap();
        assert_eq!(toc, read_toc(&bytes).unwrap());
        assert_eq!(toc.entries, entries);
        assert_eq!(pak.pos, 16 + 48 * 10_000, "left just past the table");
        assert_eq!(pak.bytes_read, 16 + 48 * 10_000, "and nothing beyond it");
        assert!(
            pak.largest_read <= 48 * 1024,
            "the table was read {} bytes at a time",
            pak.largest_read
        );
    }

    #[test]
    fn read_toc_from_passes_disk_failures_through() {
        let pak = plain_pak(0, &two_entries());
        let mut disk = FailingDisk {
            inner: Cursor::new(pak),
            good: 40,
        };
        let err = read_toc_from(&mut disk).unwrap_err();
        assert!(matches!(err, PakReadError::Io(_)), "{err:?}");
    }
}
