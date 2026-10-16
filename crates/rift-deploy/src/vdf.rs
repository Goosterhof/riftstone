//! A minimal reader for Valve's KeyValues text format — enough for Steam's
//! `libraryfolders.vdf` and an app's `appmanifest_<id>.acf`.
//!
//! ```text
//! "libraryfolders"
//! {
//!     "0"
//!     {
//!         "path"      "C:\\Program Files (x86)\\Steam"
//!         "apps"      { "2054970"  "96651043533" }
//!     }
//! }
//! ```
//!
//! Keys and values are quoted strings (escapes `\\`, `\"`, `\n`, `\t`; any
//! other backslash stays as written, so an unescaped `D:\SteamLibrary` reads
//! correctly) or bare tokens; `{ … }` opens a nested block; `//` starts a
//! comment; a `[$WIN32]`-style condition after a value is ignored. Key lookup
//! is case-insensitive, as in Steam.

use std::fmt;

/// One KeyValues node: a string, or a block of ordered key/value pairs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Vdf {
    Str(String),
    Obj(Vec<(String, Vdf)>),
}

impl Vdf {
    /// The first child named `key` (ASCII case-insensitive).
    pub fn get(&self, key: &str) -> Option<&Vdf> {
        match self {
            Vdf::Obj(pairs) => pairs
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v),
            Vdf::Str(_) => None,
        }
    }

    /// The string value of the child named `key`.
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Vdf::as_str)
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Vdf::Str(s) => Some(s),
            Vdf::Obj(_) => None,
        }
    }

    /// The pairs of a block (empty for a string).
    pub fn entries(&self) -> &[(String, Vdf)] {
        match self {
            Vdf::Obj(pairs) => pairs,
            Vdf::Str(_) => &[],
        }
    }
}

/// Why a KeyValues text could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VdfError {
    /// 1-based line of the problem.
    pub line: usize,
    pub message: String,
}

impl fmt::Display for VdfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for VdfError {}

/// Parse a KeyValues text. The result is a block holding the top-level pairs
/// (`"libraryfolders" { … }` → a block with one pair).
pub fn parse(text: &str) -> Result<Vdf, VdfError> {
    let mut lexer = Lexer::new(text);
    let pairs = parse_block(&mut lexer, false)?;
    Ok(Vdf::Obj(pairs))
}

fn parse_block(lexer: &mut Lexer<'_>, nested: bool) -> Result<Vec<(String, Vdf)>, VdfError> {
    let mut pairs = Vec::new();
    loop {
        let key = match lexer.next_token()? {
            None if nested => return Err(lexer.error("the file ends inside a { block }")),
            None => return Ok(pairs),
            Some(Token::Close) if nested => return Ok(pairs),
            Some(Token::Close) => {
                return Err(lexer.error("a } closes a block that was never opened"))
            }
            Some(Token::Open) => return Err(lexer.error("a { block } has no key in front of it")),
            Some(Token::Condition) => continue,
            Some(Token::Text(text)) => text,
        };
        let value = match lexer.next_token()? {
            Some(Token::Text(text)) => Vdf::Str(text),
            Some(Token::Open) => Vdf::Obj(parse_block(lexer, true)?),
            _ => return Err(lexer.error(format!("the key \"{key}\" has no value"))),
        };
        pairs.push((key, value));
    }
}

#[derive(Debug, PartialEq)]
enum Token {
    Text(String),
    Open,
    Close,
    Condition,
}

struct Lexer<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
    line: usize,
}

impl<'a> Lexer<'a> {
    fn new(text: &'a str) -> Self {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        Lexer {
            chars: text.chars().peekable(),
            line: 1,
        }
    }

    fn error(&self, message: impl Into<String>) -> VdfError {
        VdfError {
            line: self.line,
            message: message.into(),
        }
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.chars.next()?;
        if c == '\n' {
            self.line += 1;
        }
        Some(c)
    }

    fn next_token(&mut self) -> Result<Option<Token>, VdfError> {
        loop {
            let Some(&c) = self.chars.peek() else {
                return Ok(None);
            };
            match c {
                c if c.is_whitespace() => {
                    self.bump();
                }
                '/' => {
                    self.bump();
                    if self.chars.peek() == Some(&'/') {
                        while let Some(c) = self.bump() {
                            if c == '\n' {
                                break;
                            }
                        }
                    } else {
                        return Ok(Some(Token::Text(self.bare_token("/"))));
                    }
                }
                '{' => {
                    self.bump();
                    return Ok(Some(Token::Open));
                }
                '}' => {
                    self.bump();
                    return Ok(Some(Token::Close));
                }
                '"' => {
                    self.bump();
                    return self.quoted().map(|s| Some(Token::Text(s)));
                }
                '[' => {
                    while let Some(c) = self.bump() {
                        if c == ']' || c == '\n' {
                            break;
                        }
                    }
                    return Ok(Some(Token::Condition));
                }
                _ => return Ok(Some(Token::Text(self.bare_token("")))),
            }
        }
    }

    fn quoted(&mut self) -> Result<String, VdfError> {
        let start_line = self.line;
        let mut out = String::new();
        loop {
            match self.bump() {
                None => {
                    return Err(VdfError {
                        line: start_line,
                        message: "a quoted string is never closed".into(),
                    })
                }
                Some('"') => return Ok(out),
                Some('\\') => match self.chars.peek().copied() {
                    Some('\\') => {
                        self.bump();
                        out.push('\\');
                    }
                    Some('"') => {
                        self.bump();
                        out.push('"');
                    }
                    Some('n') => {
                        self.bump();
                        out.push('\n');
                    }
                    Some('t') => {
                        self.bump();
                        out.push('\t');
                    }
                    _ => out.push('\\'),
                },
                Some(c) => out.push(c),
            }
        }
    }

    fn bare_token(&mut self, start: &str) -> String {
        let mut out = start.to_string();
        while let Some(&c) = self.chars.peek() {
            if c.is_whitespace() || c == '{' || c == '}' || c == '"' {
                break;
            }
            out.push(c);
            self.bump();
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIBRARYFOLDERS: &str = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"C:\\\\Program Files (x86)\\\\Steam\"\n\t\t\"label\"\t\t\"\"\n\t\t\"contentid\"\t\t\"4242\"\n\t\t\"apps\"\n\t\t{\n\t\t\t\"228980\"\t\t\"277286956\"\n\t\t}\n\t}\n\t\"1\"\n\t{\n\t\t\"path\"\t\t\"D:\\\\SteamLibrary\"\n\t\t\"apps\"\n\t\t{\n\t\t\t\"2054970\"\t\t\"96651043533\"\n\t\t}\n\t}\n}\n";

    #[test]
    fn reads_a_real_libraryfolders_shape() {
        let doc = parse(LIBRARYFOLDERS).unwrap();
        let folders = doc.get("LibraryFolders").unwrap();
        assert_eq!(folders.entries().len(), 2);
        assert_eq!(
            folders.get("0").unwrap().get_str("path"),
            Some(r"C:\Program Files (x86)\Steam")
        );
        let second = folders.get("1").unwrap();
        assert_eq!(second.get_str("path"), Some(r"D:\SteamLibrary"));
        assert!(second.get("apps").unwrap().get("2054970").is_some());
        assert_eq!(folders.get("0").unwrap().get_str("label"), Some(""));
    }

    #[test]
    fn reads_an_app_manifest() {
        let acf = "\"AppState\"\r\n{\r\n\t\"appid\"\t\t\"2054970\"\r\n\t\"name\"\t\t\"Dragon's Dogma 2\"\r\n\t\"installdir\"\t\t\"Dragons Dogma 2\"\r\n\t\"buildid\"\t\t\"25290080\"\r\n\t\"InstalledDepots\"\r\n\t{\r\n\t\t\"2054971\"\r\n\t\t{\r\n\t\t\t\"manifest\"\t\t\"3438547366935427234\"\r\n\t\t}\r\n\t}\r\n}\r\n";
        let doc = parse(acf).unwrap();
        let state = doc.get("appstate").unwrap();
        assert_eq!(state.get_str("installdir"), Some("Dragons Dogma 2"));
        assert_eq!(state.get_str("BuildID"), Some("25290080"));
        assert_eq!(state.get_str("name"), Some("Dragon's Dogma 2"));
    }

    #[test]
    fn handles_comments_bare_tokens_escapes_and_conditions() {
        let text = "// a comment\n\"root\" {\n  bare value // trailing\n  \"q\" \"say \\\"hi\\\"\\n\\ttab\"\n  \"raw\" \"D:\\SteamLibrary\" [$WIN32]\n  \"slash\" a/b\n}\n";
        let doc = parse(text).unwrap();
        let root = doc.get("root").unwrap();
        assert_eq!(root.get_str("bare"), Some("value"));
        assert_eq!(root.get_str("q"), Some("say \"hi\"\n\ttab"));
        assert_eq!(root.get_str("raw"), Some(r"D:\SteamLibrary"));
        assert_eq!(root.get_str("slash"), Some("a/b"));
    }

    #[test]
    fn reports_broken_structure_with_a_line() {
        let unclosed = parse("\"a\"\n{\n\"b\" \"c\"\n").unwrap_err();
        assert!(unclosed.message.contains("ends inside"));
        assert!(parse("}").is_err());
        assert!(parse("\"a\" \"b").is_err());
        let missing = parse("\"root\" { \"key\" }").unwrap_err();
        assert!(missing.message.contains("has no value"), "{missing}");
    }
}
