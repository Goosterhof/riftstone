//! The class stream inside a save's payload. It describes itself, so it parses with no type
//! information.
//!
//! ```text
//! stream  := (u32 field hash, Class)*
//! Class   := u32 field count, u32 type hash, Field*
//! Field   := u32 field hash, i32 kind, value            (then align 4)
//! value   := kind -1 array | 15 string | 17 class | otherwise a sized scalar
//! scalar  := align 4, u32 size, align size, size bytes
//! string  := align 4, u32 length, length × u16
//! array   := align 4, i32 member kind, u32 member size, u32 count, i32 array kind
//!            (0 values, 1 classes), [u32 0xFFEEFFEE, count × u32 type hash],
//!            count items, align 4
//! ```
//!
//! A field hash is MurmurHash3 x86_32 (seed `0xFFFFFFFF`) of the field's UTF-8 name, so a
//! caller asks for fields by name ([`Class::get`]). Alignment counts from the start of the
//! FILE, and the payload starts at file offset 16: a struct aligns to its own size, so a 24-byte
//! struct at payload offset 8 lands differently from one at file offset 8.

use thiserror::Error;

/// Where the payload sits in the file: the alignment base.
const BASE: usize = 16;
const NAME_HASH_SEED: u32 = 0xFFFF_FFFF;
const CLASS_TYPES_MARKER: u32 = 0xFFEE_FFEE;
/// Deeper nesting than any save holds (the investor's goes 12 deep); refuses a hostile file.
const MAX_DEPTH: usize = 96;

/// The engine's alignment: `(pos + to - 1) & !(to - 1)`, the bitmask form even for sizes that
/// are not powers of two (a 12-byte vector). Rounding up to a true multiple instead parses
/// the first few hundred bytes of a real save and then loses its place.
fn align_up(pos: usize, to: usize) -> usize {
    (pos + to - 1) & !(to - 1)
}

/// The hash a field's name is filed under.
pub fn field_hash(name: &str) -> u32 {
    rift_pak::murmur3_32(name.as_bytes(), NAME_HASH_SEED)
}

/// A value in the stream.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    /// Signed integers and enums.
    Int(i64),
    /// Unsigned integers and chars.
    UInt(u64),
    Float(f64),
    Str(String),
    /// A value type stored as raw bytes (GUIDs, vectors, ids).
    Struct(Vec<u8>),
    Class(Class),
    Array(Vec<Value>),
}

/// A class instance: its type hash and its fields by name hash.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Class {
    pub type_hash: u32,
    pub fields: Vec<(u32, Value)>,
}

/// One top-level entry: the field hash it is filed under and the class.
pub type Root = (u32, Class);

impl Value {
    pub fn as_i64(&self) -> Option<i64> {
        match *self {
            Value::Int(v) => Some(v),
            Value::UInt(v) => i64::try_from(v).ok(),
            Value::Bool(v) => Some(i64::from(v)),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match *self {
            Value::UInt(v) => Some(v),
            Value::Int(v) => u64::try_from(v).ok(),
            Value::Bool(v) => Some(u64::from(v)),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match *self {
            Value::Bool(v) => Some(v),
            Value::Int(v) => Some(v != 0),
            Value::UInt(v) => Some(v != 0),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_class(&self) -> Option<&Class> {
        match self {
            Value::Class(c) => Some(c),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }
}

impl Class {
    /// The field filed under `name`.
    pub fn get(&self, name: &str) -> Option<&Value> {
        let hash = field_hash(name);
        self.fields.iter().find(|(h, _)| *h == hash).map(|(_, v)| v)
    }

    /// Visit this class and every class below it, depth first.
    pub fn walk<'a>(&'a self, visit: &mut impl FnMut(&'a Class)) {
        visit(self);
        for (_, value) in &self.fields {
            walk_value(value, visit);
        }
    }
}

fn walk_value<'a>(value: &'a Value, visit: &mut impl FnMut(&'a Class)) {
    match value {
        Value::Class(c) => c.walk(visit),
        Value::Array(items) => {
            for item in items {
                walk_value(item, visit);
            }
        }
        _ => {}
    }
}

/// Visit every class in a parsed stream, depth first.
pub fn walk<'a>(roots: &'a [Root], visit: &mut impl FnMut(&'a Class)) {
    for (_, class) in roots {
        class.walk(visit);
    }
}

/// Why a stream would not parse. Names the byte offset (in the payload), never a byte.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StreamError {
    #[error("the save's stream ends early (at byte {at})")]
    Truncated { at: usize },
    #[error("the save's stream holds an unknown field kind {kind} (at byte {at})")]
    UnknownKind { kind: i32, at: usize },
    #[error("the save's stream holds an impossible size (at byte {at})")]
    BadSize { at: usize },
    #[error("the save's stream nests deeper than any save (at byte {at})")]
    TooDeep { at: usize },
}

struct Reader<'a> {
    payload: &'a [u8],
    /// Position in the FILE (payload offset + 16).
    pos: usize,
}

impl Reader<'_> {
    fn at(&self) -> usize {
        self.pos - BASE
    }

    fn remaining(&self) -> usize {
        self.payload.len() + BASE - self.pos
    }

    fn align(&mut self, to: usize) -> Result<(), StreamError> {
        if to > 1 {
            let aligned = align_up(self.pos, to);
            if aligned > self.payload.len() + BASE {
                return Err(StreamError::Truncated { at: self.at() });
            }
            self.pos = aligned;
        }
        Ok(())
    }

    fn take(&mut self, n: usize) -> Result<&[u8], StreamError> {
        if n > self.remaining() {
            return Err(StreamError::Truncated { at: self.at() });
        }
        let start = self.pos - BASE;
        self.pos += n;
        Ok(&self.payload[start..start + n])
    }

    fn u32(&mut self) -> Result<u32, StreamError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().expect("4")))
    }

    fn i32(&mut self) -> Result<i32, StreamError> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().expect("4")))
    }

    fn class(&mut self, depth: usize) -> Result<Class, StreamError> {
        if depth > MAX_DEPTH {
            return Err(StreamError::TooDeep { at: self.at() });
        }
        let count = self.u32()? as usize;
        let type_hash = self.u32()?;
        // A field takes at least 8 bytes: a count past the stream's end is a lie.
        if count > self.remaining() / 8 {
            return Err(StreamError::BadSize { at: self.at() });
        }
        let mut fields = Vec::with_capacity(count);
        for _ in 0..count {
            let hash = self.u32()?;
            let kind = self.i32()?;
            let value = self.value(kind, depth)?;
            self.align(4)?;
            fields.push((hash, value));
        }
        Ok(Class { type_hash, fields })
    }

    fn value(&mut self, kind: i32, depth: usize) -> Result<Value, StreamError> {
        match kind {
            -1 => self.array(depth),
            17 => Ok(Value::Class(self.class(depth + 1)?)),
            15 => self.string(),
            1..=14 | 16 => {
                self.align(4)?;
                let size = self.u32()? as usize;
                self.scalar(kind, size)
            }
            _ => Err(StreamError::UnknownKind {
                kind,
                at: self.at(),
            }),
        }
    }

    fn string(&mut self) -> Result<Value, StreamError> {
        self.align(4)?;
        let units = self.u32()? as usize;
        let bytes = self.take(
            units
                .checked_mul(2)
                .ok_or(StreamError::BadSize { at: self.at() })?,
        )?;
        let utf16: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        Ok(Value::Str(String::from_utf16_lossy(&utf16)))
    }

    fn scalar(&mut self, kind: i32, size: usize) -> Result<Value, StreamError> {
        if size == 0 || size > self.remaining() {
            return Err(StreamError::BadSize { at: self.at() });
        }
        self.align(size)?;
        let at = self.at();
        let bytes = self.take(size)?;
        let signed = |b: &[u8]| -> Result<i64, StreamError> {
            Ok(match b.len() {
                1 => i64::from(b[0] as i8),
                2 => i64::from(i16::from_le_bytes([b[0], b[1]])),
                4 => i64::from(i32::from_le_bytes(b.try_into().expect("4"))),
                8 => i64::from_le_bytes(b.try_into().expect("8")),
                _ => return Err(StreamError::BadSize { at }),
            })
        };
        let unsigned = |b: &[u8]| -> Result<u64, StreamError> {
            Ok(match b.len() {
                1 => u64::from(b[0]),
                2 => u64::from(u16::from_le_bytes([b[0], b[1]])),
                4 => u64::from(u32::from_le_bytes(b.try_into().expect("4"))),
                8 => u64::from_le_bytes(b.try_into().expect("8")),
                _ => return Err(StreamError::BadSize { at }),
            })
        };
        Ok(match kind {
            1 | 3 | 5 | 7 | 9 => Value::Int(signed(bytes)?),
            2 => Value::Bool(bytes[0] != 0),
            4 | 6 | 8 | 10 | 13 | 14 => Value::UInt(unsigned(bytes)?),
            11 if size == 4 => {
                Value::Float(f64::from(f32::from_le_bytes(bytes.try_into().expect("4"))))
            }
            12 if size == 8 => Value::Float(f64::from_le_bytes(bytes.try_into().expect("8"))),
            11 | 12 => return Err(StreamError::BadSize { at }),
            _ => Value::Struct(bytes.to_vec()),
        })
    }

    fn array(&mut self, depth: usize) -> Result<Value, StreamError> {
        if depth > MAX_DEPTH {
            return Err(StreamError::TooDeep { at: self.at() });
        }
        self.align(4)?;
        let member = self.i32()?;
        let member_size = self.u32()? as usize;
        let count = self.u32()? as usize;
        let of_classes = self.i32()? == 1;
        if count > self.remaining() {
            return Err(StreamError::BadSize { at: self.at() });
        }
        if of_classes && self.remaining() >= 4 {
            let marker = u32::from_le_bytes(
                self.payload[self.pos - BASE..self.pos - BASE + 4]
                    .try_into()
                    .expect("4"),
            );
            if marker == CLASS_TYPES_MARKER {
                self.take(4)?;
                self.take(
                    count
                        .checked_mul(4)
                        .ok_or(StreamError::BadSize { at: self.at() })?,
                )?;
            }
        }
        let mut items = Vec::with_capacity(count.min(1 << 16));
        for _ in 0..count {
            items.push(if of_classes {
                Value::Class(self.class(depth + 1)?)
            } else if member == 15 {
                self.string()?
            } else if (1..=14).contains(&member) || member == 16 {
                self.scalar(member, member_size)?
            } else {
                return Err(StreamError::UnknownKind {
                    kind: member,
                    at: self.at(),
                });
            });
        }
        self.align(4)?;
        Ok(Value::Array(items))
    }
}

/// Parse a payload ([`crate::open`]'s output) into its top-level classes.
pub fn parse(payload: &[u8]) -> Result<Vec<Root>, StreamError> {
    let mut reader = Reader { payload, pos: BASE };
    let mut roots = Vec::new();
    // The stream ends within 7 bytes of padding.
    while reader.remaining() > 8 {
        let hash = reader.u32()?;
        roots.push((hash, reader.class(0)?));
    }
    Ok(roots)
}

/// Builds streams from invented classes — tests only.
#[cfg(any(test, feature = "fixtures"))]
pub mod write {
    use super::{field_hash, BASE, CLASS_TYPES_MARKER};

    /// A value to write, with the kind it is written as.
    #[derive(Debug, Clone)]
    pub enum W {
        Bool(bool),
        I32(i32),
        U32(u32),
        I64(i64),
        U8(u8),
        F32(f32),
        F64(f64),
        Str(String),
        Struct(Vec<u8>),
        Class(WClass),
        ClassArray(Vec<WClass>, bool),
        I32Array(Vec<i32>),
    }

    #[derive(Debug, Clone, Default)]
    pub struct WClass {
        pub type_hash: u32,
        pub fields: Vec<(u32, W)>,
    }

    impl WClass {
        pub fn new(type_name: &str) -> Self {
            WClass {
                type_hash: field_hash(type_name),
                fields: Vec::new(),
            }
        }

        pub fn with(mut self, name: &str, value: W) -> Self {
            self.fields.push((field_hash(name), value));
            self
        }
    }

    struct Out {
        bytes: Vec<u8>,
    }

    impl Out {
        fn pos(&self) -> usize {
            self.bytes.len() + BASE
        }
        fn align(&mut self, to: usize) {
            if to > 1 {
                let target = super::align_up(self.pos(), to);
                self.bytes.resize(target - BASE, 0);
            }
        }
        fn u32(&mut self, v: u32) {
            self.bytes.extend_from_slice(&v.to_le_bytes());
        }
        fn i32(&mut self, v: i32) {
            self.bytes.extend_from_slice(&v.to_le_bytes());
        }
        fn scalar(&mut self, kind: i32, bytes: &[u8]) {
            self.i32(kind);
            self.align(4);
            self.u32(bytes.len() as u32);
            self.align(bytes.len());
            self.bytes.extend_from_slice(bytes);
        }
        fn string_body(&mut self, s: &str) {
            self.align(4);
            let units: Vec<u16> = s.encode_utf16().collect();
            self.u32(units.len() as u32);
            for u in units {
                self.bytes.extend_from_slice(&u.to_le_bytes());
            }
        }
        fn class(&mut self, c: &WClass) {
            self.u32(c.fields.len() as u32);
            self.u32(c.type_hash);
            for (hash, value) in &c.fields {
                self.u32(*hash);
                self.value(value);
                self.align(4);
            }
        }
        fn value(&mut self, v: &W) {
            match v {
                W::Bool(b) => self.scalar(2, &[u8::from(*b)]),
                W::I32(x) => self.scalar(7, &x.to_le_bytes()),
                W::U32(x) => self.scalar(8, &x.to_le_bytes()),
                W::I64(x) => self.scalar(9, &x.to_le_bytes()),
                W::U8(x) => self.scalar(4, &[*x]),
                W::F32(x) => self.scalar(11, &x.to_le_bytes()),
                W::F64(x) => self.scalar(12, &x.to_le_bytes()),
                W::Struct(b) => self.scalar(16, b),
                W::Str(s) => {
                    self.i32(15);
                    self.string_body(s);
                }
                W::Class(c) => {
                    self.i32(17);
                    self.class(c);
                }
                W::ClassArray(items, marker) => {
                    self.i32(-1);
                    self.align(4);
                    self.i32(17);
                    self.u32(8);
                    self.u32(items.len() as u32);
                    self.i32(1);
                    if *marker {
                        self.u32(CLASS_TYPES_MARKER);
                        for item in items {
                            self.u32(item.type_hash);
                        }
                    }
                    for item in items {
                        self.class(item);
                    }
                    self.align(4);
                }
                W::I32Array(items) => {
                    self.i32(-1);
                    self.align(4);
                    self.i32(7);
                    self.u32(4);
                    self.u32(items.len() as u32);
                    self.i32(0);
                    for x in items {
                        self.align(4);
                        self.bytes.extend_from_slice(&x.to_le_bytes());
                    }
                    self.align(4);
                }
            }
        }
    }

    /// Serialize top-level classes into a payload.
    pub fn stream(roots: &[(&str, WClass)]) -> Vec<u8> {
        let mut out = Out { bytes: Vec::new() };
        for (name, class) in roots {
            out.u32(field_hash(name));
            out.class(class);
        }
        out.align(8);
        out.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::write::{stream, WClass, W};
    use super::*;

    #[test]
    fn field_hashes_are_murmur3_of_the_utf8_name() {
        assert_eq!(field_hash(""), rift_pak::murmur3_32(b"", 0xFFFF_FFFF));
        assert_eq!(
            field_hash("_Level"),
            rift_pak::murmur3_32(b"_Level", 0xFFFF_FFFF)
        );
    }

    #[test]
    fn an_invented_tree_round_trips() {
        let member = WClass::new("app.HumanStatusContext")
            .with("_Level", W::I32(41))
            .with("_Exp", W::I32(123_456))
            .with("_Name", W::Str("Odalric".into()))
            .with("_Gold", W::I64(284_310))
            .with("_IsEquipped", W::Bool(true))
            .with("_Slot", W::U8(3))
            .with("_Speed", W::F32(1.5))
            .with("_Time", W::F64(2.25))
            .with(
                "_Id",
                W::Struct(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]),
            );
        let list = WClass::new("app.Records")
            .with(
                "Contexts",
                W::ClassArray(vec![member.clone(), member.clone()], true),
            )
            .with("Plain", W::ClassArray(vec![member], false))
            .with("Numbers", W::I32Array(vec![-1, 2, 3]));
        let payload = stream(&[("GameSaveDataList", list)]);
        let roots = parse(&payload).unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].0, field_hash("GameSaveDataList"));
        let contexts = roots[0].1.get("Contexts").unwrap().as_array().unwrap();
        assert_eq!(contexts.len(), 2);
        let first = contexts[0].as_class().unwrap();
        assert_eq!(first.get("_Level").and_then(Value::as_i64), Some(41));
        assert_eq!(first.get("_Name").and_then(Value::as_str), Some("Odalric"));
        assert_eq!(first.get("_Gold").and_then(Value::as_i64), Some(284_310));
        assert_eq!(
            first.get("_IsEquipped").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(first.get("_Slot").and_then(Value::as_u64), Some(3));
        assert_eq!(first.get("_Speed"), Some(&Value::Float(1.5)));
        assert_eq!(
            first
                .get("_Id")
                .map(|v| matches!(v, Value::Struct(b) if b.len() == 16)),
            Some(true)
        );
        assert_eq!(
            roots[0].1.get("Numbers"),
            Some(&Value::Array(vec![
                Value::Int(-1),
                Value::Int(2),
                Value::Int(3)
            ]))
        );
        let mut seen = 0;
        walk(&roots, &mut |c| {
            if c.type_hash == field_hash("app.HumanStatusContext") {
                seen += 1;
            }
        });
        assert_eq!(seen, 3);
    }

    #[test]
    fn the_alignment_is_the_engines_bitmask_form() {
        assert_eq!(align_up(20, 4), 20);
        assert_eq!(align_up(21, 4), 24);
        assert_eq!(align_up(41, 8), 48);
        // Not a power of two: the bitmask form, not a multiple (a multiple would say 24 and 48).
        assert_eq!(align_up(20, 12), 20);
        assert_eq!(align_up(40, 24), 40);
        assert_eq!(align_up(41, 12), 52);
    }

    #[test]
    fn a_struct_aligns_to_its_size_from_the_files_start() {
        // A 12-byte vector and a 24-byte struct round-trip under the engine's rule. That the
        // rule counts from the file's start, not the payload's, and uses the bitmask form, is
        // what the investor's own save showed (rift-save's bench).
        let c = WClass::new("t")
            .with("V", W::Struct(vec![3; 12]))
            .with("A", W::Struct(vec![9; 24]))
            .with("B", W::I32(7));
        let payload = stream(&[("r", c)]);
        let roots = parse(&payload).unwrap();
        assert_eq!(roots[0].1.get("B").and_then(Value::as_i64), Some(7));
        assert_eq!(roots[0].1.get("A"), Some(&Value::Struct(vec![9; 24])));
        assert_eq!(roots[0].1.get("V"), Some(&Value::Struct(vec![3; 12])));
    }

    #[test]
    fn an_empty_string_and_an_empty_array_read_back() {
        let c = WClass::new("t")
            .with("S", W::Str(String::new()))
            .with("L", W::ClassArray(vec![], true));
        let roots = parse(&stream(&[("r", c)])).unwrap();
        assert_eq!(roots[0].1.get("S").and_then(Value::as_str), Some(""));
        assert_eq!(
            roots[0]
                .1
                .get("L")
                .and_then(Value::as_array)
                .map(<[Value]>::len),
            Some(0)
        );
    }

    #[test]
    fn broken_streams_are_errors_never_panics() {
        let c = WClass::new("t")
            .with("S", W::Str("Wren".into()))
            .with("N", W::I32(1));
        let payload = stream(&[("r", c)]);
        for cut in 0..payload.len() {
            let _ = parse(&payload[..cut]);
        }
        let mut lying = payload.clone();
        lying[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(parse(&lying), Err(StreamError::BadSize { .. })));
        let mut odd = payload;
        odd[16..20].copy_from_slice(&99i32.to_le_bytes());
        assert!(parse(&odd).is_err());
    }
}
