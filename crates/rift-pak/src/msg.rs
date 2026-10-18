//! The engine's text files (`.msg`, GMSG), read-only: entry names and their text per language.
//!
//! Dragon's Dogma 2's are version `0x18` (`.msg.24`):
//!
//! ```text
//! u32 version, "GMSG", u64 header offset, u32 entry count, u32 attribute count,
//! u32 language count, u32 0, u64 data offset, u64 p offset, u64 language offset,
//! u64 attribute-type offset, u64 attribute-name offset, entry count × u64 entry offset
//! entry := 16-byte GUID, u32, u32 name hash, u64 name pointer, u64 attributes pointer,
//!          language count × u64 text pointer
//! ```
//!
//! The pointers point into the data block, which runs to the end of the file and is
//! XOR-chained with a 16-byte key (`plain[i] = prev ^ cur ^ key[i & 15]`, `prev` the previous
//! cipher byte); its strings are UTF-16LE and end with a 0 unit. Language 1 is English.

use std::collections::HashMap;

const KEY: [u8; 16] = [
    207, 206, 251, 248, 236, 10, 51, 102, 147, 169, 29, 147, 80, 57, 95, 9,
];
const VERSION_DD2: u32 = 0x18;
/// The language the stone reads.
pub const ENGLISH: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MsgError {
    #[error("not an engine text file (no GMSG magic)")]
    NotMsg,
    #[error("an engine text version the stone does not read ({0:#x})")]
    Version(u32),
    #[error("the engine text file is damaged")]
    Damaged,
}

/// One text file: each entry's name with its text in every language the file carries.
#[derive(Debug, Clone, Default)]
pub struct Msg {
    languages: Vec<u32>,
    entries: HashMap<String, Vec<String>>,
    order: Vec<String>,
}

impl Msg {
    pub fn parse(bytes: &[u8]) -> Result<Msg, MsgError> {
        let u32_at = |at: usize| -> Result<u32, MsgError> {
            bytes
                .get(at..at + 4)
                .map(|b| u32::from_le_bytes(b.try_into().expect("4")))
                .ok_or(MsgError::Damaged)
        };
        let u64_at = |at: usize| -> Result<usize, MsgError> {
            bytes
                .get(at..at + 8)
                .map(|b| u64::from_le_bytes(b.try_into().expect("8")))
                .and_then(|v| usize::try_from(v).ok())
                .ok_or(MsgError::Damaged)
        };
        if bytes.get(4..8) != Some(b"GMSG") {
            return Err(MsgError::NotMsg);
        }
        let version = u32_at(0)?;
        if version != VERSION_DD2 {
            return Err(MsgError::Version(version));
        }
        let count = u32_at(16)? as usize;
        let languages_count = u32_at(24)? as usize;
        let data_offset = u64_at(32)?;
        let languages_offset = u64_at(48)?;
        if data_offset > bytes.len() || count > bytes.len() / 8 || languages_count > 64 {
            return Err(MsgError::Damaged);
        }
        let languages = (0..languages_count)
            .map(|i| u32_at(languages_offset + i * 4))
            .collect::<Result<Vec<_>, _>>()?;
        let mut data = bytes[data_offset..].to_vec();
        let mut prev = 0u8;
        for (i, b) in data.iter_mut().enumerate() {
            let cur = *b;
            *b = prev ^ cur ^ KEY[i & 15];
            prev = cur;
        }
        let text_at = |pointer: usize| -> Result<String, MsgError> {
            let start = pointer
                .checked_sub(data_offset)
                .filter(|&s| s <= data.len())
                .ok_or(MsgError::Damaged)?;
            let units: Vec<u16> = data[start..]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .take_while(|&u| u != 0)
                .collect();
            Ok(String::from_utf16_lossy(&units))
        };
        let mut entries = HashMap::with_capacity(count);
        let mut order = Vec::with_capacity(count);
        for i in 0..count {
            let entry = u64_at(72 + i * 8)?;
            let name = text_at(u64_at(entry + 24)?)?;
            let texts = (0..languages_count)
                .map(|l| u64_at(entry + 40 + l * 8).and_then(&text_at))
                .collect::<Result<Vec<_>, _>>()?;
            order.push(name.clone());
            entries.insert(name, texts);
        }
        Ok(Msg {
            languages,
            entries,
            order,
        })
    }

    /// The entry's text in `language`, or `None` when the file has no such entry or language.
    pub fn text(&self, name: &str, language: u32) -> Option<&str> {
        let index = self.languages.iter().position(|&l| l == language)?;
        self.entries.get(name)?.get(index).map(String::as_str)
    }

    /// The entry's English text; `None` also when it is empty.
    pub fn english(&self, name: &str) -> Option<&str> {
        self.text(name, ENGLISH).filter(|t| !t.is_empty())
    }

    /// Entry names in file order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.order.iter().map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }
}

/// Builds an invented GMSG file — tests only.
#[cfg(any(test, feature = "fixtures"))]
pub fn fixture(languages: &[u32], entries: &[(&str, &[&str])]) -> Vec<u8> {
    let header = 72;
    let entry_table = header + entries.len() * 8;
    let entry_len = 40 + languages.len() * 8;
    let languages_offset = entry_table + entries.len() * entry_len;
    let mut data_offset = languages_offset + languages.len() * 4;
    data_offset = data_offset.div_ceil(8) * 8;
    let mut data: Vec<u8> = Vec::new();
    let mut put = |s: &str| -> u64 {
        let at = (data_offset + data.len()) as u64;
        for u in s.encode_utf16() {
            data.extend_from_slice(&u.to_le_bytes());
        }
        data.extend_from_slice(&[0, 0]);
        at
    };
    let mut table = Vec::new();
    for (name, texts) in entries {
        let mut e = vec![0u8; entry_len];
        e[16..20].copy_from_slice(&0u32.to_le_bytes());
        e[20..24].copy_from_slice(&crate::murmur3_32(name.as_bytes(), 0xFFFF_FFFF).to_le_bytes());
        e[24..32].copy_from_slice(&put(name).to_le_bytes());
        for (l, text) in texts.iter().enumerate().take(languages.len()) {
            e[40 + l * 8..48 + l * 8].copy_from_slice(&put(text).to_le_bytes());
        }
        table.push(e);
    }
    let mut out = vec![0u8; data_offset];
    out[0..4].copy_from_slice(&VERSION_DD2.to_le_bytes());
    out[4..8].copy_from_slice(b"GMSG");
    out[8..16].copy_from_slice(&16u64.to_le_bytes());
    out[16..20].copy_from_slice(&(entries.len() as u32).to_le_bytes());
    out[24..28].copy_from_slice(&(languages.len() as u32).to_le_bytes());
    out[32..40].copy_from_slice(&(data_offset as u64).to_le_bytes());
    out[48..56].copy_from_slice(&(languages_offset as u64).to_le_bytes());
    for (i, e) in table.iter().enumerate() {
        let at = entry_table + i * entry_len;
        out[header + i * 8..header + i * 8 + 8].copy_from_slice(&(at as u64).to_le_bytes());
        out[at..at + entry_len].copy_from_slice(e);
    }
    for (i, l) in languages.iter().enumerate() {
        out[languages_offset + i * 4..languages_offset + i * 4 + 4]
            .copy_from_slice(&l.to_le_bytes());
    }
    // Encrypt the data block: cipher[i] = plain[i] ^ prev_cipher ^ key[i & 15].
    let mut prev = 0u8;
    for (i, b) in data.iter().enumerate() {
        let c = b ^ prev ^ KEY[i & 15];
        out.push(c);
        prev = c;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_invented_text_file_reads_back() {
        let bytes = fixture(
            &[0, 1, 2],
            &[
                ("item_name_1", &["", "Ashen Greatsword", "Épée cendrée"]),
                ("item_name_2", &["", "", ""]),
                ("job_name_05", &["", "Warrior", "Guerrier"]),
            ],
        );
        let msg = Msg::parse(&bytes).unwrap();
        assert_eq!(msg.len(), 3);
        assert_eq!(msg.english("item_name_1"), Some("Ashen Greatsword"));
        assert_eq!(msg.text("item_name_1", 2), Some("Épée cendrée"));
        assert_eq!(msg.english("item_name_2"), None, "an empty text is no name");
        assert_eq!(msg.english("item_name_3"), None);
        assert_eq!(
            msg.text("job_name_05", 9),
            None,
            "a language the file lacks"
        );
        assert_eq!(
            msg.names().collect::<Vec<_>>(),
            ["item_name_1", "item_name_2", "job_name_05"]
        );
    }

    #[test]
    fn other_files_and_damage_are_refused() {
        assert_eq!(
            Msg::parse(b"not a text file at all").unwrap_err(),
            MsgError::NotMsg
        );
        let mut bytes = fixture(&[1], &[("a", &["b"])]);
        bytes[0] = 0x16;
        assert_eq!(Msg::parse(&bytes).unwrap_err(), MsgError::Version(0x16));
        let bytes = fixture(&[1], &[("a", &["b"])]);
        for cut in [10, 60, 80, bytes.len() - 3] {
            let _ = Msg::parse(&bytes[..cut]);
        }
    }
}
