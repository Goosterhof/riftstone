//! rift-fray — THE FRAY's stone: the party screen's numbers (#00015 §19).
//!
//! ```text
//! the eye's folder ─ eye::Tail::poll ─► record::Line ─► fray::Fray::ingest ─► Fray::frame ─► the window
//! ```
//!
//! The eye is the REFramework script inside Dragon's Dogma 2: it writes every hit, death, foe
//! sample and heartbeat as one JSON line. This crate reads those lines, credits each hit to a
//! party member by the reviewed analyzer's rule, cuts the session into fights, and hands the
//! window one frame. It reads only the eye's own folder, and nothing here writes.
//!
//! Names are not this crate's: a member, a foe, a skill and a vocation travel as ids, and the
//! spine joins the game's own text to them at display (THE MUSTER's Codex).

pub mod eye;
pub mod frame;
pub mod fray;
pub mod record;

pub use eye::{newest_session, session_clock, Tail};
pub use frame::Frame;
pub use fray::{Credit, Fray, SessionClock, Settings};

#[cfg(test)]
mod tests;
