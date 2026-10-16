//! rift-pak — the RE Engine `.pak` table of contents, and the engine's path hash.
//!
//! A Dragon's Dogma 2 mod reaches the game in one of two ways: as loose files
//! under `natives/` (served by REFramework's LooseFileLoader) or as `.pak`
//! archives in `pak_mods/`. A pak stores no file names, only a pair of murmur3
//! hashes per file. This crate reads a pak's table of contents (which hashes it
//! carries and how big each file is) without touching the payload, and hashes a
//! game path the same way. That is how the catalog notices that a loose mesh and
//! a pak replace the same game file.
//!
//! # The table of contents (KPKA)
//!
//! Little-endian throughout. The 16-byte header:
//!
//! | bytes  | field |
//! |--------|-------|
//! | 0..4   | magic `KPKA` |
//! | 4      | major version: 4 for every Dragon's Dogma 2 pak |
//! | 5      | minor version: 0 (mod paks), 1 (the `patch_NNN` stubs), 2 (game paks) |
//! | 6..8   | feature flags |
//! | 8..12  | file count |
//! | 12..16 | fingerprint (REE.Packer stamps `0xDEC0ADDE`, ree-pak-rs writes 0) |
//!
//! The entry table follows the header directly: `file count` entries of
//! [`ENTRY_LEN`] (48) bytes, each `hash_lower: u32`, `hash_upper: u32`,
//! `offset: u64`, `compressed_size: u64`, `decompressed_size: u64`,
//! `attributes: u64`, `checksum: u64`. So the table of contents is
//! `16 + 48 × file count` bytes from the start of the file ([`toc_len`]).
//! Versions other than 4.0–4.2 are refused (the v2 paks with 24-byte entries
//! exist only in older RE Engine titles).
//!
//! ## Feature flags
//!
//! Feature flag [`FLAG_ENCRYPTED_TABLE`] (`0x0008`) means the entry table is
//! encrypted with a 128-byte key that lies behind it, though not always
//! directly: a `0x0010`, `0x0004` or `0x0040` block comes first, and a
//! `0x0020` chunk table comes after the key. Encryption covers the game's own
//! paks (flags `0x0068` / `0x0028`) and the 144-byte `patch_NNN` stubs (flags
//! `0x0008`, zero files). [`read_toc`] reports such a pak as `encrypted` with
//! no entries, never as an error.
//!
//! Every other flag the references know adds a block *after* the entry table:
//!
//! | flag     | block behind the entry table | described by |
//! |----------|------------------------------|--------------|
//! | `0x0004` | 9 bytes (ree-pak-rs: first seen in RE9) | both |
//! | `0x0010` | one u32 | both |
//! | `0x0020` | a chunk table, 8 bytes per chunk | both |
//! | `0x0040` | DD2's extra chunk table, 16 bytes per chunk | REE.PAK.Tool only, and only inside DD2's `0x0068` |
//!
//! Both references refuse every flag they do not list. ree-pak-rs calls
//! `0x0040` the undocumented bit `BIT06`, and its default reader
//! (`strict_feature_flags: true`) answers `UnsupportedFeature` to any bit
//! outside `0x003C`, so it refuses DD2's own `0x0068` paks. REE.PAK.Tool
//! accepts only the exact flag values 0, 8, 12, 24, 40, 44 and 104, all of
//! them encrypted but 0, so it never reads a plain table with a flag set.
//!
//! The Riftstone deliberately reads a plain table at byte 16 whatever the
//! flags say. Every block either reference describes sits behind the table,
//! mod paks carry flags 0 (REE.Packer writes v4.0 with flags 0; so do the mod
//! paks measured on the bench), and a table of contents only feeds conflict
//! hints, so refusing would lose more than it protects. What no reference can
//! vouch for is reported instead: [`PakToc::unknown_flags`] returns the bits
//! outside [`KNOWN_FLAGS`], and a caller that finds any should treat that
//! pak's hashes as unconfirmed.
//!
//! ## Damaged counts
//!
//! A plain table that claims more than [`MAX_ENTRIES`] files is refused as
//! [`PakError::TooManyEntries`], by [`toc_len`], [`read_toc`] and
//! [`read_toc_from`] alike, before anything behind the header is read or
//! allocated. Below that cap, [`read_toc_from`] parses the table 1,024 entries
//! (48 KiB) at a time, so the raw table never sits in memory beside the parsed
//! entries: a damaged count costs at most 192 MiB of reading and 160 MiB of
//! entries, never a multiple of the pak's size.
//!
//! # The path hash
//!
//! [`path_hash`] files a game path under two MurmurHash3 x86_32 values
//! ([`murmur3_32`]) of its UTF-16LE bytes, both with seed [`PATH_HASH_SEED`]
//! (`0xFFFF_FFFF`): `lower` hashes the lower-cased path, `upper` the upper-cased
//! one. The table stores `lower` first, so on disk the pair reads as the 64-bit
//! key `upper << 32 | lower` ([`PathHash::combined`]) that both reference tools
//! use. The published reference pair (from ree-pak-rs's test suite; an
//! independent port of REE.PAK.Tool's C# gives the same pair):
//! `natives/stm/camera/collisionfilter/defaultcamera.cfil.7` →
//! lower `0x65B486A1`, upper `0x958EDD0C`.
//!
//! **The hashed string is the whole game path, `natives/stm/` included**, with
//! the file's numeric version suffix (`….cfil.7`, `….pfb.18`, `….tex.251211553`).
//! Callers pass the full path from `natives/`, never a path relative to
//! `natives/stm/`. The evidence:
//!
//! - All 366,525 lines of REE.PAK.Tool's `Projects/DD2_STM_Release.list` start
//!   with `natives/stm/`, and `PakList.iLoadProject` hashes each line verbatim.
//! - ree-pak-cli's packer strips everything *before* `natives/` and warns about
//!   any input path without it. REE.Packer hashes the path relative to the
//!   folder being packed, whose top level is `natives/`.
//! - The published pair above is a `natives/stm/…` path.
//!
//! **Separators:** backslashes become forward slashes before hashing. REE.Packer
//! and ree-pak-rs both do this, and REFramework identifies the engine's own
//! path-to-hash routine partly by its reference to the UTF-16 backslash.
//! Nothing else is normalised: no prefix is stripped, no `./` removed.
//!
//! **Case folding:** the Riftstone folds ASCII letters only (`A`–`Z` ↔ `a`–`z`)
//! and hashes every other UTF-16 unit unchanged. Every reference agrees on
//! every ASCII path, and all 366,525 Dragon's Dogma 2 paths in REE.PAK.Tool's
//! list are pure ASCII (and lower-case, with forward slashes), so in practice
//! the rule changes no hash. Outside ASCII the references disagree, and none
//! of them documents what the engine itself does:
//!
//! - REE.PAK.Tool hashes with .NET `ToLower()` / `ToUpper()`, both when it
//!   names files (`PakList.cs`) and when it packs them (`PakPack.cs`). Those
//!   follow the machine's culture: they fold `É`/`é` and `П`/`п`, and on a
//!   Turkish-locale machine they even change the ASCII `i`, so that rule has
//!   no single answer.
//! - ree-pak-rs folds ASCII only. Its own module doc gives the reason, speed
//!   and no `unsafe`, and accepts that a name with Latin-extended letters then
//!   hashes differently from standard Unicode case conversion. Its full
//!   Unicode version survives behind the `legacy-utf16-hash` feature, "for
//!   testing, benchmark and backward compatibility only".
//! - Rust's full `to_lowercase()` / `to_uppercase()`, the formula
//!   `docs/BUILD-SPEC.md` v0.1 writes, agrees with .NET on letters such as
//!   `É`/`é`, but also turns `ß` into `SS` and lower-cases a word-final `Σ` to
//!   `ς`; neither tool changes a path's length or reads context like that. On
//!   ASCII input it is exactly this crate's rule (a test checks every ASCII
//!   character).
//!
//! ASCII folding is locale-free, length-preserving and context-free, and it
//! matches ree-pak-rs on every path and REE.PAK.Tool on every ASCII one. The
//! price is the non-ASCII letters .NET folds: `natives/stm/É.tex` hashes to
//! `5A794FDF` / `F650655C` here, where the BUILD-SPEC formula (and .NET) give
//! `E3063A26` / `F650655C`.
//!
//! REFramework corroborates the mechanism. Its LooseFileLoader hooks the
//! engine's path-to-hash routine (a `const wchar_t*` path in, the 64-bit pair
//! out, two murmur3 calls recognised by the `0x85EBCA6B` finaliser constant).
//! When a loose file exists on disk, the hook makes that lookup miss, so the
//! loose file wins over any pak carrying the same hash. Its source does not
//! show how the routine folds case.
//!
//! # Sources checked (2026-10-16)
//!
//! - REE.PAK.Tool (Ekey) at `3626a12`:
//!   `REE.Unpacker/REE.Unpacker/FileSystem/Package/PakHash.cs` (UTF-16LE, seed
//!   `0xFFFFFFFF`), `…/Package/PakList.cs` (`ToLower` / `ToUpper`,
//!   `upper << 32 | lower`), `…/Hashing/Murmur3.cs` (`HashCore32`),
//!   `…/Package/PakFlags.cs` (the seven `Features` values it accepts, with the
//!   block order behind the table for each), `…/Package/PakUnpack.cs` (the
//!   header, the refusal of any other flags, the v4 entry order),
//!   `REE.Packer/REE.Packer/FileSystem/Package/PakPack.cs` (`ToLower` /
//!   `ToUpper` when packing; writes v4.0, flags 0) and
//!   `Projects/DD2_STM_Release.list`.
//!   <https://github.com/Ekey/REE.PAK.Tool/tree/3626a12a82c5fad753abd0f4fb33f90405c59a04>
//! - ree-pak-rs (eigeen) at `25562e6`: `ree-pak-core/src/utf16_hash.rs`
//!   (ASCII folding and why; the known pair in
//!   `test_compatibility_with_known_values`), `ree-pak-core/src/pak/flag.rs`
//!   (`FeatureFlags`, `SUPPORTED_BITS` = `0x003C`),
//!   `ree-pak-core/src/pak/header.rs` (the strict flag check),
//!   `ree-pak-core/src/read/mod.rs` (`strict_feature_flags: true` by default;
//!   the u32, the 9 bytes and the key behind the table, in that order),
//!   `ree-pak-core/src/read/chunk_table.rs` (read after the key),
//!   `ree-pak-core/src/spec/entry.rs` (the 48-byte entry, with a published
//!   byte vector reused in this crate's tests) and `ree-pak-cli/src/pack.rs`
//!   (the `natives/` root).
//!   <https://github.com/eigeen/ree-pak-rs/tree/25562e6a6f9a52a43b80d54be91e4feb7ac7668b>
//! - REFramework (praydog) at `d052e8f`: `src/mods/LooseFileLoader.cpp`.
//!   <https://github.com/praydog/REFramework/blob/d052e8fd3fd41d0a1a33394fcf6e6c19be39b1ee/src/mods/LooseFileLoader.cpp>
//! - MurmurHash3 x86_32 test vectors: the SMHasher reference values (seed 0, 1,
//!   `0xFFFFFFFF`, `0x9747B28C`) as collected in Ian Boyd's published vector table.
//!
//! Cross-checked at scale: all 366,525 paths of the DD2 list, a third of them
//! re-cased and a fifth with backslashes, hash identically here and in an
//! independent port of REE.PAK.Tool's C#. `tests/bench_reference_hashes.rs`
//! (ignored, env-gated) re-runs such a check against any
//! `<lower> <upper> <path>` file.
//!
//! FluffyQuack's REtool also packs paks, but its source ships as a download
//! archive rather than a browsable repository, and was not inspected.

use serde::Serialize;

mod hash;
mod toc;

pub use hash::{murmur3_32, path_hash};
pub use toc::{read_toc, read_toc_from, toc_len};

/// The pair of murmur3 hashes the engine files a path under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathHash {
    pub lower: u32,
    pub upper: u32,
}

impl PathHash {
    /// The 64-bit key both reference tools file a path under:
    /// `upper << 32 | lower`. It is also the little-endian reading of the
    /// entry's first eight bytes.
    #[must_use]
    pub const fn combined(self) -> u64 {
        ((self.upper as u64) << 32) | (self.lower as u64)
    }

    /// Split a 64-bit `upper << 32 | lower` key back into its halves.
    #[must_use]
    pub const fn from_combined(combined: u64) -> Self {
        PathHash {
            lower: combined as u32,
            upper: (combined >> 32) as u32,
        }
    }
}

/// Sixteen upper-case hex digits of [`PathHash::combined`]. REE.PAK.Tool names
/// a file it cannot identify the same way (`__Unknown/958EDD0C65B486A1`), so a
/// log line and an unpacked folder can be matched by eye.
impl std::fmt::Display for PathHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:016X}", self.combined())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PakEntry {
    pub hash: PathHash,
    pub offset: u64,
    pub compressed_size: u64,
    pub size: u64,
    pub attributes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PakToc {
    pub major: u8,
    pub minor: u8,
    pub feature_flags: u16,
    pub file_count: u32,
    /// The entry table is encrypted (game paks; mod paks are plain).
    pub encrypted: bool,
    /// Empty when `encrypted`.
    pub entries: Vec<PakEntry>,
}

impl PakToc {
    /// The feature-flag bits no reference reader describes: those outside
    /// [`KNOWN_FLAGS`].
    ///
    /// Zero for every pak the references can read. Non-zero means the entry
    /// table was still read from byte 16, but no reference vouches that it
    /// sits there: both refuse such a pak outright. A caller that finds any
    /// should treat the pak's hashes (and so its conflicts) as unconfirmed.
    #[must_use]
    pub const fn unknown_flags(&self) -> u16 {
        self.feature_flags & !KNOWN_FLAGS
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PakError {
    #[error("not an RE Engine pak (magic {0:?})")]
    NotAPak([u8; 4]),
    #[error("pak version {0}.{1} is not one the Riftstone reads")]
    UnsupportedVersion(u8, u8),
    #[error("pak is truncated: need {needed} bytes, have {have}")]
    Truncated { needed: u64, have: u64 },
    /// A plain table claims more files than [`MAX_ENTRIES`]: the header is
    /// damaged, and its table is never read.
    #[error(
        "pak header claims {claimed} files, more than the {limit} the Riftstone accepts: \
         the header is damaged"
    )]
    TooManyEntries { claimed: u32, limit: u32 },
}

/// Why [`read_toc_from`] could not read a pak's table of contents.
#[derive(Debug, thiserror::Error)]
pub enum PakReadError {
    /// The reader failed: the file vanished, is locked, or the disk failed.
    #[error("could not read the pak: {0}")]
    Io(#[from] std::io::Error),
    /// The bytes arrived but are not a table of contents the Riftstone reads.
    #[error(transparent)]
    Pak(#[from] PakError),
}

/// Header size in bytes.
pub const HEADER_LEN: usize = 16;

/// Size in bytes of one entry in a v4 table of contents.
pub const ENTRY_LEN: usize = 48;

/// The four bytes every RE Engine pak starts with.
pub const MAGIC: [u8; 4] = *b"KPKA";

/// Feature flag: the entry table is encrypted (the game's own paks).
pub const FLAG_ENCRYPTED_TABLE: u16 = 0x0008;

/// The feature-flag bits at least one reference reader describes. Each adds a
/// block behind the entry table (the crate docs carry the table and sources):
/// `0x0004` nine bytes, [`FLAG_ENCRYPTED_TABLE`] the key of an encrypted
/// table, `0x0010` one u32, `0x0020` a chunk table, and `0x0040` DD2's extra
/// chunk table (described by REE.PAK.Tool only). See [`PakToc::unknown_flags`].
pub const KNOWN_FLAGS: u16 = 0x0004 | FLAG_ENCRYPTED_TABLE | 0x0010 | 0x0020 | 0x0040;

/// The most files a plain table may claim: 4,194,304, more than eleven times
/// the 366,525 paths of the whole game in REE.PAK.Tool's
/// `Projects/DD2_STM_Release.list` (a mod pak on the bench holds 177). A
/// larger count is a damaged header and is refused as
/// [`PakError::TooManyEntries`] before its table is read or allocated; at the
/// cap the table is 192 MiB and its parsed entries 160 MiB.
pub const MAX_ENTRIES: u32 = 1 << 22;

/// The murmur3 seed the engine hashes paths with.
pub const PATH_HASH_SEED: u32 = 0xFFFF_FFFF;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combined_key_puts_upper_in_the_high_half() {
        let hash = PathHash {
            lower: 0x65B4_86A1,
            upper: 0x958E_DD0C,
        };
        assert_eq!(hash.combined(), 0x958E_DD0C_65B4_86A1);
        assert_eq!(PathHash::from_combined(0x958E_DD0C_65B4_86A1), hash);
        assert_eq!(hash.to_string(), "958EDD0C65B486A1");
        assert_eq!(PathHash::from_combined(0x1).to_string(), "0000000000000001");
    }

    #[test]
    fn read_errors_keep_their_cause() {
        let pak: PakReadError = PakError::Truncated {
            needed: 16,
            have: 3,
        }
        .into();
        assert_eq!(pak.to_string(), "pak is truncated: need 16 bytes, have 3");

        let io: PakReadError = std::io::Error::other("the disk sighed").into();
        assert_eq!(io.to_string(), "could not read the pak: the disk sighed");

        let damaged: PakReadError = PakError::TooManyEntries {
            claimed: 4_194_305,
            limit: MAX_ENTRIES,
        }
        .into();
        assert_eq!(
            damaged.to_string(),
            "pak header claims 4194305 files, more than the 4194304 the Riftstone accepts: \
             the header is damaged"
        );
    }

    #[test]
    fn the_documented_cap_costs_what_the_docs_say() {
        // MAX_ENTRIES, the crate docs and the PakError message all promise
        // these numbers: 192 MiB of table, 160 MiB of parsed entries.
        assert_eq!(MAX_ENTRIES, 4_194_304);
        assert_eq!(
            HEADER_LEN as u64 + ENTRY_LEN as u64 * u64::from(MAX_ENTRIES),
            192 * 1024 * 1024 + 16
        );
        assert_eq!(
            std::mem::size_of::<PakEntry>() as u64 * u64::from(MAX_ENTRIES),
            160 * 1024 * 1024
        );
        assert_eq!(KNOWN_FLAGS, 0x007C);
    }

    #[test]
    fn hashes_and_tables_cross_the_bridge_in_camel_case() {
        let toc = PakToc {
            major: 4,
            minor: 0,
            feature_flags: 0,
            file_count: 1,
            encrypted: false,
            entries: vec![PakEntry {
                hash: PathHash { lower: 1, upper: 2 },
                offset: 64,
                compressed_size: 10,
                size: 20,
                attributes: 1,
            }],
        };
        let json = serde_json::to_value(&toc).unwrap();
        assert_eq!(json["featureFlags"], 0);
        assert_eq!(json["fileCount"], 1);
        assert_eq!(json["encrypted"], false);
        assert_eq!(json["entries"][0]["hash"]["lower"], 1);
        assert_eq!(json["entries"][0]["hash"]["upper"], 2);
        assert_eq!(json["entries"][0]["compressedSize"], 10);
        assert_eq!(json["entries"][0]["size"], 20);
    }
}
