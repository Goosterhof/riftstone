//! MurmurHash3 x86_32 and the engine's path hash. The crate docs carry the
//! scheme and the sources it was checked against.

use crate::{PathHash, PATH_HASH_SEED};

const C1: u32 = 0xcc9e_2d51;
const C2: u32 = 0x1b87_3593;

const BACKSLASH: u16 = b'\\' as u16;
const SLASH: u16 = b'/' as u16;

/// MurmurHash3 x86_32: Austin Appleby's reference `MurmurHash3_x86_32`
/// (SMHasher), the variant the RE Engine hashes paths with.
///
/// ```
/// assert_eq!(rift_pak::murmur3_32(b"hello", 0), 0x248B_FA47);
/// ```
#[must_use]
pub fn murmur3_32(data: &[u8], seed: u32) -> u32 {
    let mut h = seed;
    let mut blocks = data.chunks_exact(4);
    for block in &mut blocks {
        let k = u32::from_le_bytes([block[0], block[1], block[2], block[3]]);
        h ^= scramble(k);
        h = h.rotate_left(13).wrapping_mul(5).wrapping_add(0xe654_6b64);
    }
    let tail = blocks.remainder();
    if !tail.is_empty() {
        let k = tail
            .iter()
            .rev()
            .fold(0u32, |k, &byte| (k << 8) | u32::from(byte));
        h ^= scramble(k);
    }
    // The reference mixes in the length modulo 2^32 (`h1 ^= len` on an int).
    h ^= data.len() as u32;
    fmix32(h)
}

fn scramble(k: u32) -> u32 {
    k.wrapping_mul(C1).rotate_left(15).wrapping_mul(C2)
}

fn fmix32(mut h: u32) -> u32 {
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    h
}

/// Hash a game path (`natives/stm/character/...`) the way the engine does.
///
/// Pass the whole game path, `natives/stm/` prefix and version suffix
/// included. Either slash works, and case does not matter for ASCII letters.
/// The crate docs explain the folding rule and cite the references.
///
/// ```
/// let hash = rift_pak::path_hash("natives/stm/camera/collisionfilter/defaultcamera.cfil.7");
/// assert_eq!((hash.lower, hash.upper), (0x65B4_86A1, 0x958E_DD0C));
/// assert_eq!(hash, rift_pak::path_hash(r"NATIVES\STM\Camera\CollisionFilter\DefaultCamera.cfil.7"));
/// ```
#[must_use]
pub fn path_hash(game_path: &str) -> PathHash {
    // Every UTF-8 byte yields at most one UTF-16 unit of two bytes.
    let mut lower = Vec::with_capacity(game_path.len() * 2);
    let mut upper = Vec::with_capacity(game_path.len() * 2);
    for unit in game_path.encode_utf16() {
        let unit = if unit == BACKSLASH { SLASH } else { unit };
        lower.extend_from_slice(&ascii_lower(unit).to_le_bytes());
        upper.extend_from_slice(&ascii_upper(unit).to_le_bytes());
    }
    PathHash {
        lower: murmur3_32(&lower, PATH_HASH_SEED),
        upper: murmur3_32(&upper, PATH_HASH_SEED),
    }
}

/// `A`–`Z` to `a`–`z`; every other UTF-16 unit unchanged. ASCII letters are
/// never surrogate halves, so folding unit by unit is folding char by char.
fn ascii_lower(unit: u16) -> u16 {
    if (u16::from(b'A')..=u16::from(b'Z')).contains(&unit) {
        unit + 32
    } else {
        unit
    }
}

/// `a`–`z` to `A`–`Z`; every other UTF-16 unit unchanged.
fn ascii_upper(unit: u16) -> u16 {
    if (u16::from(b'a')..=u16::from(b'z')).contains(&unit) {
        unit - 32
    } else {
        unit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16le(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn murmur3_matches_the_build_spec_vectors() {
        assert_eq!(murmur3_32(b"", 0), 0);
        assert_eq!(murmur3_32(b"", 1), 0x514E_28B7);
        assert_eq!(murmur3_32(b"hello", 0), 0x248B_FA47);
        assert_eq!(murmur3_32(b"Hello, world!", 1234), 0xFAF6_CDB3);
    }

    #[test]
    fn murmur3_matches_the_smhasher_reference_vectors() {
        // Every tail length (0, 1, 2, 3 bytes) and the path seed itself.
        let vectors: [(&[u8], u32, u32); 15] = [
            (b"", 0xFFFF_FFFF, 0x81F1_6F39),
            (b"\0", 0, 0x514E_28B7),
            (b"\0\0", 0, 0x30F4_C306),
            (b"\0\0\0", 0, 0x85F0_B427),
            (b"\0\0\0\0", 0, 0x2362_F9DE),
            (b"a", 0x9747_B28C, 0x7FA0_9EA6),
            (b"aa", 0x9747_B28C, 0x5D21_1726),
            (b"aaa", 0x9747_B28C, 0x283E_0130),
            (b"aaaa", 0x9747_B28C, 0x5A97_808A),
            (b"ab", 0x9747_B28C, 0x7487_5592),
            (b"abc", 0x9747_B28C, 0xC84A_62DD),
            (b"abcd", 0x9747_B28C, 0xF047_8627),
            (b"Hello, world!", 0x9747_B28C, 0x2488_4CBA),
            ("ππππππππ".as_bytes(), 0x9747_B28C, 0xD580_63C1),
            (
                b"The quick brown fox jumps over the lazy dog",
                0x9747_B28C,
                0x2FA8_26CD,
            ),
        ];
        for (data, seed, want) in vectors {
            assert_eq!(
                murmur3_32(data, seed),
                want,
                "murmur3_32({data:?}, {seed:#x})"
            );
        }
    }

    #[test]
    fn path_hash_reproduces_the_published_reference_pair() {
        // ree-pak-rs 25562e6, utf16_hash.rs `test_compatibility_with_known_values`;
        // an independent port of REE.PAK.Tool's C# gives the same pair.
        let hash = path_hash("natives/stm/camera/collisionfilter/defaultcamera.cfil.7");
        assert_eq!(hash.lower, 0x65B4_86A1);
        assert_eq!(hash.upper, 0x958E_DD0C);
        assert_eq!(hash.combined(), 0x958E_DD0C_65B4_86A1);
    }

    #[test]
    fn path_hash_is_murmur3_of_the_folded_utf16le_path() {
        let path = "natives/STM/AppSystem/TopsWb/mesh/lantern_glow_recolor.pfb.18";
        let hash = path_hash(path);
        assert_eq!(
            hash.lower,
            murmur3_32(&utf16le(&path.to_ascii_lowercase()), 0xFFFF_FFFF)
        );
        assert_eq!(
            hash.upper,
            murmur3_32(&utf16le(&path.to_ascii_uppercase()), 0xFFFF_FFFF)
        );
    }

    #[test]
    fn path_hash_halves_differ_and_ignore_input_case() {
        let hash = path_hash("natives/stm/character/vernworth_cloak_cape.mesh.260421070");
        assert_ne!(hash.lower, hash.upper);
        for spelling in [
            "NATIVES/STM/CHARACTER/VERNWORTH_CLOAK_CAPE.MESH.260421070",
            "Natives/Stm/Character/Vernworth_Cloak_Cape.mesh.260421070",
            "nAtIvEs/sTm/cHaRaCtEr/vErNwOrTh_cLoAk_cApE.MeSh.260421070",
        ] {
            assert_eq!(path_hash(spelling), hash, "{spelling}");
        }
    }

    #[test]
    fn backslashes_hash_as_forward_slashes() {
        assert_eq!(
            path_hash(r"natives\stm\gui\lantern_glow.tex.251211553"),
            path_hash("natives/stm/gui/lantern_glow.tex.251211553")
        );
    }

    #[test]
    fn the_natives_prefix_is_part_of_what_is_hashed() {
        // The engine hashes the full path; a path relative to natives/stm/
        // names a different file as far as any pak is concerned.
        assert_ne!(
            path_hash("natives/stm/camera/collisionfilter/defaultcamera.cfil.7"),
            path_hash("camera/collisionfilter/defaultcamera.cfil.7")
        );
    }

    /// `docs/BUILD-SPEC.md` v0.1's formula, verbatim: Rust's full Unicode
    /// `to_lowercase()` / `to_uppercase()` of the forward-slashed path.
    fn build_spec_formula(path: &str) -> PathHash {
        let path = path.replace('\\', "/");
        PathHash {
            lower: murmur3_32(&utf16le(&path.to_lowercase()), 0xFFFF_FFFF),
            upper: murmur3_32(&utf16le(&path.to_uppercase()), 0xFFFF_FFFF),
        }
    }

    #[test]
    fn the_build_spec_formula_is_this_rule_on_every_ascii_path() {
        // Every Dragon's Dogma 2 path is ASCII, so the ASCII rule and the
        // spec's full-Unicode formula must never part there: try each of the
        // 128 ASCII characters inside a path, then real-shaped paths.
        for byte in 0u8..=127 {
            let path = format!("natives/STM/Lantern{}Glow.tex.251211553", char::from(byte));
            assert_eq!(
                path_hash(&path),
                build_spec_formula(&path),
                "byte {byte:#04x}"
            );
        }
        for path in [
            "natives/stm/camera/collisionfilter/defaultcamera.cfil.7",
            r"NATIVES\STM\AppSystem\TopsWb\mesh\Vernworth_Cloak_wb_f.pfb.18",
            "natives/stm/character/vernworth_cloak_cape.mesh.260421070",
            "",
        ] {
            assert_eq!(path_hash(path), build_spec_formula(path), "{path}");
        }
    }

    #[test]
    fn only_ascii_letters_fold_everything_else_is_hashed_as_is() {
        // The ree-pak-rs rule: É/é and ß are left alone (no ß → SS growth),
        // so the hashed strings keep the path's own UTF-16 length.
        let path = "natives/stm/Écharpe_ß.tex.251211553";
        let hash = path_hash(path);
        assert_eq!(
            hash.lower,
            murmur3_32(&utf16le("natives/stm/Écharpe_ß.tex.251211553"), 0xFFFF_FFFF)
        );
        assert_eq!(
            hash.upper,
            murmur3_32(&utf16le("NATIVES/STM/ÉCHARPE_ß.TEX.251211553"), 0xFFFF_FFFF)
        );
        assert_ne!(
            path_hash("natives/stm/É.tex"),
            path_hash("natives/stm/é.tex")
        );

        // Pinned, so that a change of rule is a decision and never a slip.
        // The BUILD-SPEC formula (and .NET's ToLower/ToUpper) fold É and é
        // together, to E3063A26 / F650655C; this rule does not.
        let upper_e = path_hash("natives/stm/É.tex");
        let lower_e = path_hash("natives/stm/é.tex");
        assert_eq!((upper_e.lower, upper_e.upper), (0x5A79_4FDF, 0xF650_655C));
        assert_eq!((lower_e.lower, lower_e.upper), (0xE306_3A26, 0x7580_A04A));
        let spec = build_spec_formula("natives/stm/É.tex");
        assert_eq!((spec.lower, spec.upper), (0xE306_3A26, 0xF650_655C));
        assert_eq!(spec, build_spec_formula("natives/stm/é.tex"));
    }

    #[test]
    fn characters_outside_the_bmp_hash_as_surrogate_pairs() {
        // U+1F409 (a dragon) is two UTF-16 units; neither half is touched.
        let hash = path_hash("natives/stm/\u{1F409}.tex");
        assert_eq!(
            hash.lower,
            murmur3_32(&utf16le("natives/stm/\u{1F409}.tex"), 0xFFFF_FFFF)
        );
        assert_eq!(
            hash.upper,
            murmur3_32(&utf16le("NATIVES/STM/\u{1F409}.TEX"), 0xFFFF_FFFF)
        );
    }

    #[test]
    fn the_empty_path_hashes_like_the_empty_string() {
        let hash = path_hash("");
        assert_eq!(hash.lower, 0x81F1_6F39);
        assert_eq!(hash.upper, 0x81F1_6F39);
    }
}
