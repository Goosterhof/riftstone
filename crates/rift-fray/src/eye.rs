//! The eye's folder, read as it grows: `<game>\reframework\data\riftstone\probe\`.
//!
//! The eye writes `session-<ses>.json` once at its start and appends its lines to
//! `events-<ses>-<seg>.ndjson`, opening a new segment after 16 MiB or a failed write. Appends never
//! tear a line the eye has flushed whole, but a read can land between the bytes of one it is still
//! writing, so a tail keeps the bytes after the last newline and reads them again next time. A
//! fragment that ends a segment the eye moved past is left unread (contract §3.1). Nothing here
//! writes.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::fray::SessionClock;
use crate::record::{parse_line, Line};

/// The newest session in the folder: the one whose id (`<unix seconds>_<hex>`) sorts last by its
/// number, then its text. None when the folder holds none, or cannot be read.
pub fn newest_session(dir: &Path) -> Option<String> {
    let entries = fs::read_dir(dir).ok()?;
    entries
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|name| session_id(&name).map(str::to_owned))
        .max_by(|a, b| id_number(a).cmp(&id_number(b)).then_with(|| a.cmp(b)))
}

fn session_id(file: &str) -> Option<&str> {
    let id = file.strip_prefix("session-")?.strip_suffix(".json")?;
    valid_id(id).then_some(id)
}

fn valid_id(id: &str) -> bool {
    let Some((num, hex)) = id.split_once('_') else {
        return false;
    };
    !num.is_empty()
        && num.len() <= 20
        && num.bytes().all(|b| b.is_ascii_digit())
        && hex.len() == 6
        && hex.bytes().all(|b| b.is_ascii_hexdigit())
}

fn id_number(id: &str) -> u128 {
    id.split_once('_')
        .and_then(|(n, _)| n.parse().ok())
        .unwrap_or(0)
}

/// The session's start, from its session file: os.clock `started.clock` was wall second `started.time`.
pub fn session_clock(dir: &Path, ses: &str) -> Option<SessionClock> {
    let text = fs::read_to_string(dir.join(format!("session-{ses}.json"))).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let started = v.get("started")?;
    Some(SessionClock {
        clock: started.get("clock")?.as_f64()?,
        unix: started.get("time")?.as_i64()?,
    })
}

#[derive(Debug, Default)]
struct Segment {
    /// Bytes consumed: everything before the last newline read.
    offset: u64,
}

/// One session's segments, read from where the last poll stopped.
#[derive(Debug)]
pub struct Tail {
    dir: PathBuf,
    ses: String,
    segments: BTreeMap<u32, Segment>,
    /// Lines that did not parse: counted, never guessed at.
    pub bad_lines: u64,
}

impl Tail {
    pub fn new(dir: impl Into<PathBuf>, ses: impl Into<String>) -> Self {
        Tail {
            dir: dir.into(),
            ses: ses.into(),
            segments: BTreeMap::new(),
            bad_lines: 0,
        }
    }

    pub fn session(&self) -> &str {
        &self.ses
    }

    /// Every whole line written since the last poll, segments in order.
    pub fn poll(&mut self) -> io::Result<Vec<Line>> {
        let prefix = format!("events-{}-", self.ses);
        for entry in fs::read_dir(&self.dir)? {
            let Ok(name) = entry?.file_name().into_string() else {
                continue;
            };
            let Some(seg) = name
                .strip_prefix(&prefix)
                .and_then(|s| s.strip_suffix(".ndjson"))
            else {
                continue;
            };
            if let Ok(seg) = seg.parse::<u32>() {
                self.segments.entry(seg).or_default();
            }
        }
        let mut out = Vec::new();
        let segs: Vec<u32> = self.segments.keys().copied().collect();
        for seg in segs {
            let path = self.dir.join(format!("{prefix}{seg}.ndjson"));
            let Some(state) = self.segments.get_mut(&seg) else {
                continue;
            };
            let mut file = match fs::File::open(&path) {
                Ok(f) => f,
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e),
            };
            file.seek(SeekFrom::Start(state.offset))?;
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            let Some(end) = buf.iter().rposition(|b| *b == b'\n') else {
                continue;
            };
            state.offset += end as u64 + 1;
            for raw in buf[..end].split(|b| *b == b'\n') {
                let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
                if raw.is_empty() {
                    continue;
                }
                match std::str::from_utf8(raw).ok().map(parse_line) {
                    Some(Ok(line)) => out.push(line),
                    _ => self.bad_lines += 1,
                }
            }
        }
        Ok(out)
    }
}
