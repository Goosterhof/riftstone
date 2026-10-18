//! The Mandarin cipher, as Dragon's Dogma 2 uses it.
//!
//! The math follows MandarinJuice (MIT License, © 2026 Michał Gębicki,
//! <https://github.com/mi5hmash/MandarinJuice>), `MandarinDeencryptor.DecryptData` and
//! `EncryptData`, written again for Rust with `num-bigint` doing the 256-bit arithmetic that
//! MandarinJuice spells out limb by limb. Constants are MandarinJuice's `PrivateKey1..3`
//! (the prime `p`, the group order `q`, the generator `g`); DD2's seed comes from the
//! community's per-game table.
//!
//! The payload is cut into slices of 16–128 KiB. Each slice carries a 528-byte header, XOR-ed
//! with a SplitMix64 stream: four ElGamal pairs `(c1, c2)` of 64 bytes, then a u64 checksum and
//! a u64 length. Every `c1` is the constant `g^20 mod p`, and `c2 = s × part` where `s` is
//! `c1^uid mod p`, so `part = c2 / s`. The four parts are an AES-128 key and an OFB IV for the
//! slice's bytes. `uid` is the bitwise NOT of the Steam account id.

use aes::cipher::{BlockCipherEncrypt, KeyInit};
use aes::Aes128;
use num_bigint::BigUint;

/// Dragon's Dogma 2's Mandarin seed.
pub(crate) const DD2_SEED: u64 = 0x5EC6_4699_7D69_AE1B;

/// One slice header: four `(c1, c2)` pairs of 64 bytes, a checksum and a length.
pub(crate) const SLICE_HEADER: usize = 2 * 4 * 8 * 8 + 16;

const SLICE_UNIT_SHIFT: u32 = 14;

const P_LE: [u8; 32] = [
    0xF3, 0x3B, 0x6F, 0xB9, 0x72, 0xA0, 0xB7, 0x25, 0x15, 0xE4, 0x5C, 0x39, 0x18, 0x29, 0xE1, 0x82,
    0xAD, 0x8A, 0x9B, 0xDC, 0x0A, 0x64, 0xD3, 0x44, 0x4D, 0x79, 0xC8, 0x10, 0xAB, 0x86, 0x37, 0x17,
];
#[cfg(any(test, feature = "fixtures"))]
const Q_LE: [u8; 32] = [
    0xF9, 0x9D, 0xB7, 0x5C, 0x39, 0xD0, 0xDB, 0x92, 0x0A, 0x72, 0xAE, 0x1C, 0x8C, 0x94, 0x70, 0xC1,
    0x56, 0xC5, 0x4D, 0x6E, 0x05, 0xB2, 0x69, 0xA2, 0xA6, 0x3C, 0x64, 0x88, 0x55, 0xC3, 0x9B, 0x0B,
];
#[cfg(any(test, feature = "fixtures"))]
const G_LE: [u8; 32] = [
    0xE6, 0x6F, 0x54, 0x4A, 0xFC, 0xCE, 0x68, 0xC5, 0xEF, 0x07, 0xB9, 0xA0, 0x7B, 0x27, 0x75, 0x85,
    0x34, 0x4A, 0x1D, 0xB6, 0x13, 0x76, 0xE8, 0x31, 0xF7, 0x3B, 0x9F, 0xBD, 0x5F, 0x44, 0xF7, 0x15,
];
/// `g^20 mod p`: the `c1` of every pair in every slice header.
const HEADER_KEY_LE: [u8; 32] = [
    0xBB, 0x39, 0xC8, 0x17, 0x02, 0xD5, 0x49, 0x5B, 0x67, 0x18, 0x44, 0xD2, 0xF5, 0xEE, 0x2B, 0x77,
    0xB8, 0x11, 0xEB, 0x6E, 0x07, 0x6B, 0x23, 0x6E, 0x41, 0xEE, 0x7C, 0xE3, 0x42, 0xF5, 0x16, 0x12,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MandarinError {
    /// The encrypted bytes do not have the length the slice layout asks for.
    SliceLayout,
    /// The first slice header does not open: the key is another account's.
    WrongAccount,
}

/// SplitMix64, one step.
fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    *state = z ^ (z >> 31);
    *state
}

/// The slice sizes (in 16 KiB units) for a payload of `len` bytes, and the state after them.
/// MandarinJuice's `CalculateSlicesQueue`: `(len >> 14) + 1` laps, each lap that still has
/// bytes to cover takes `(state & 7) + 1` units; every lap advances the state.
fn slice_units(seed: u64, mut len: u64) -> (Vec<u64>, u64) {
    let mut state = seed;
    let mut units = Vec::new();
    for _ in 0..(len >> SLICE_UNIT_SHIFT) + 1 {
        if len > 0 {
            let unit = (state & 7) + 1;
            units.push(unit);
            len = len.saturating_sub(unit << SLICE_UNIT_SHIFT);
        }
        splitmix(&mut state);
    }
    (units, state)
}

/// The encrypted length a payload of `len` bytes takes, header bytes included.
#[cfg(any(test, feature = "fixtures"))]
pub(crate) fn encrypted_len(len: u64) -> u64 {
    let (units, _) = slice_units(DD2_SEED, len);
    units
        .iter()
        .map(|u| (u << SLICE_UNIT_SHIFT) + SLICE_HEADER as u64)
        .sum()
}

fn uid_of(account_id: u32) -> u64 {
    !u64::from(account_id)
}

/// `c1^uid mod p` with `c1 = g^20 mod p`: one modular power per account and file.
fn slice_secret(uid: u64) -> BigUint {
    let p = BigUint::from_bytes_le(&P_LE);
    BigUint::from_bytes_le(&HEADER_KEY_LE).modpow(&BigUint::from(uid), &p)
}

/// The next 528 SplitMix low bytes, after the 16 steps every slice skips first.
fn header_stream(state: &mut u64) -> [u8; SLICE_HEADER] {
    for _ in 0..16 {
        splitmix(state);
    }
    let mut stream = [0u8; SLICE_HEADER];
    for b in &mut stream {
        *b = splitmix(state) as u8;
    }
    stream
}

/// AES-128-OFB over `data` in place (OFB is its own inverse).
fn ofb(key: &[u8; 16], iv: &[u8; 16], data: &mut [u8]) {
    let cipher = Aes128::new(&(*key).into());
    let mut block: aes::Block = (*iv).into();
    for chunk in data.chunks_mut(16) {
        cipher.encrypt_block(&mut block);
        for (b, k) in chunk.iter_mut().zip(block.iter()) {
            *b ^= k;
        }
    }
}

/// Decrypt `data` (the bytes between the DSSS header and footer) into `len` bytes.
pub(crate) fn decrypt(data: &[u8], len: u64, account_id: u32) -> Result<Vec<u8>, MandarinError> {
    let (units, mut state) = slice_units(DD2_SEED, len);
    let expected: u64 = units
        .iter()
        .map(|u| (u << SLICE_UNIT_SHIFT) + SLICE_HEADER as u64)
        .sum();
    if expected != data.len() as u64 {
        return Err(MandarinError::SliceLayout);
    }
    let uid = uid_of(account_id);
    state = state.wrapping_add(uid);
    let header_key = BigUint::from_bytes_le(&HEADER_KEY_LE);
    let secret = slice_secret(uid);

    let mut out = Vec::with_capacity(len as usize);
    let mut pos = 0usize;
    let mut remaining = len as usize;
    for unit in units {
        let size = (unit << SLICE_UNIT_SHIFT) as usize;
        let mut header = [0u8; SLICE_HEADER];
        header.copy_from_slice(&data[pos..pos + SLICE_HEADER]);
        for (b, s) in header.iter_mut().zip(header_stream(&mut state)) {
            *b ^= s;
        }
        let mut key = [0u8; 32];
        for k in 0..4 {
            let at = k * 128;
            if BigUint::from_bytes_le(&header[at..at + 64]) != header_key {
                return Err(MandarinError::WrongAccount);
            }
            let part = BigUint::from_bytes_le(&header[at + 64..at + 128]) / &secret;
            let low = part.iter_u64_digits().next().unwrap_or(0);
            key[k * 8..k * 8 + 8].copy_from_slice(&low.to_le_bytes());
        }
        let mut body = data[pos + SLICE_HEADER..pos + SLICE_HEADER + size].to_vec();
        let (aes_key, iv) = key.split_at(16);
        ofb(
            aes_key.try_into().expect("16"),
            iv.try_into().expect("16"),
            &mut body,
        );
        let take = size.min(remaining);
        out.extend_from_slice(&body[..take]);
        remaining -= take;
        pos += SLICE_HEADER + size;
    }
    Ok(out)
}

/// Encrypt `plain` for `account_id` — tests only (MandarinJuice's `EncryptData`). Each slice's
/// key is the 16 SplitMix steps the slice skips (low byte, then the next byte), as there.
#[cfg(any(test, feature = "fixtures"))]
pub(crate) fn encrypt(plain: &[u8], account_id: u32) -> Vec<u8> {
    let len = plain.len() as u64;
    let (units, mut state) = slice_units(DD2_SEED, len);
    let uid = uid_of(account_id);
    state = state.wrapping_add(uid);
    // The personal seed: (g^(uid mod q))^20 mod p, which equals c1^uid mod p.
    let p = BigUint::from_bytes_le(&P_LE);
    let q = BigUint::from_bytes_le(&Q_LE);
    let g = BigUint::from_bytes_le(&G_LE);
    let personal = g
        .modpow(&(BigUint::from(uid) % &q), &p)
        .modpow(&BigUint::from(20u8), &p);
    debug_assert_eq!(personal, slice_secret(uid));
    let mut header_key = HEADER_KEY_LE.to_vec();
    header_key.resize(64, 0);

    let mut out = Vec::with_capacity(encrypted_len(len) as usize);
    let mut done = 0usize;
    for unit in units {
        let size = (unit << SLICE_UNIT_SHIFT) as usize;
        // The slice key: the 16 skipped steps, as MandarinJuice's CalculateHeaderChecksum lays
        // them out (low bytes in the first half, the next bytes in the second).
        let mut probe = state;
        let mut key = [0u8; 32];
        for i in 0..16 {
            let s = splitmix(&mut probe);
            key[i] = s as u8;
            key[16 + i] = (s >> 8) as u8;
        }
        let mut header = [0u8; SLICE_HEADER];
        for k in 0..4 {
            let at = k * 128;
            header[at..at + 64].copy_from_slice(&header_key);
            let part = u64::from_le_bytes(key[k * 8..k * 8 + 8].try_into().expect("8"));
            let mut c2 = (&personal * BigUint::from(part)).to_bytes_le();
            c2.resize(64, 0);
            header[at + 64..at + 128].copy_from_slice(&c2);
        }
        let take = size.min(plain.len() - done);
        let remaining_before = (plain.len() - done) as u64;
        let mut length_word = remaining_before;
        splitmix(&mut length_word);
        header[520..528].copy_from_slice(&length_word.to_le_bytes());
        for (b, s) in header.iter_mut().zip(header_stream(&mut state)) {
            *b ^= s;
        }
        let mut body = vec![0u8; size];
        body[..take].copy_from_slice(&plain[done..done + take]);
        let (aes_key, iv) = key.split_at(16);
        ofb(
            aes_key.try_into().expect("16"),
            iv.try_into().expect("16"),
            &mut body,
        );
        out.extend_from_slice(&header);
        out.extend_from_slice(&body);
        done += take;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_header_key_is_g_to_the_twentieth() {
        let p = BigUint::from_bytes_le(&P_LE);
        let q = BigUint::from_bytes_le(&Q_LE);
        let g = BigUint::from_bytes_le(&G_LE);
        assert_eq!(
            g.modpow(&(BigUint::from(20u8) % &q), &p),
            BigUint::from_bytes_le(&HEADER_KEY_LE)
        );
    }

    #[test]
    fn the_secret_is_the_personal_seed_for_any_account() {
        let p = BigUint::from_bytes_le(&P_LE);
        let q = BigUint::from_bytes_le(&Q_LE);
        let g = BigUint::from_bytes_le(&G_LE);
        for account in [1u32, 4_242_424, u32::MAX] {
            let uid = uid_of(account);
            let personal = g
                .modpow(&(BigUint::from(uid) % &q), &p)
                .modpow(&BigUint::from(20u8), &p);
            assert_eq!(personal, slice_secret(uid), "account {account}");
        }
    }

    #[test]
    fn slices_cover_the_payload_in_sixteen_kib_units() {
        for len in [1u64, 16_383, 16_384, 16_385, 65_536, 400_000, 3_771_524] {
            let (units, _) = slice_units(DD2_SEED, len);
            let covered: u64 = units.iter().map(|u| u << 14).sum();
            assert!(covered >= len, "len {len}");
            assert!(units.iter().all(|u| (1..=8).contains(u)));
            let without_last: u64 = units[..units.len() - 1].iter().map(|u| u << 14).sum();
            assert!(
                without_last < len,
                "len {len}: the last slice must be needed"
            );
        }
    }

    #[test]
    fn round_trips_at_every_awkward_size() {
        for len in [
            0usize, 1, 15, 16, 17, 16_383, 16_384, 16_385, 131_072, 200_001,
        ] {
            let plain: Vec<u8> = (0..len).map(|i| (i * 31 % 251) as u8).collect();
            let sealed = encrypt(&plain, 31_337);
            assert_eq!(sealed.len() as u64, encrypted_len(len as u64));
            assert_eq!(
                decrypt(&sealed, len as u64, 31_337).unwrap(),
                plain,
                "len {len}"
            );
        }
    }

    #[test]
    fn another_account_does_not_open_the_first_header() {
        let sealed = encrypt(b"an invented party", 31_337);
        assert_eq!(
            decrypt(&sealed, 17, 31_337 + 1),
            Err(MandarinError::WrongAccount)
        );
    }

    #[test]
    fn a_short_body_is_a_layout_error_not_a_panic() {
        let sealed = encrypt(&[7u8; 40_000], 9);
        assert_eq!(
            decrypt(&sealed[..sealed.len() - 1], 40_000, 9),
            Err(MandarinError::SliceLayout)
        );
        assert_eq!(
            decrypt(&sealed[..sealed.len() - 16], 40_000, 9),
            Err(MandarinError::SliceLayout)
        );
    }
}
