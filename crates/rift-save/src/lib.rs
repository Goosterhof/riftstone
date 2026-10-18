//! rift-save — a Dragon's Dogma 2 save file, read-only.
//!
//! Steam keeps the saves in `<Steam>\userdata\<account id>\2054970\remote\win64_save\`. Each
//! file is a DSSS container whose payload is encrypted with the cipher the community calls
//! Mandarin, keyed by that account id, and (since 2026) deflated. The payload is a stream of
//! classes that describes itself: every field carries the MurmurHash3 of its name and its own
//! kind and size, so it parses with no type information at all (BUILD-SPEC § v0.4, The ground
//! truth).
//!
//! The crate reads. It has no way to write a save: the `fixtures` feature, which only tests
//! switch on, seals invented payloads so every test can build its saves at run time (RS-8).
//!
//! ```text
//! open(bytes, account id) ──► payload ──► stream::parse ──► [(field hash, Class)]
//! ```
//!
//! No error carries a byte of the payload, a name or a path: a save is the player's own.

mod container;
mod mandarin;
pub mod stream;
#[cfg(test)]
mod sweep_fixture;

pub use container::{open, SaveError, MAX_FILE};
pub use stream::{field_hash, Class, Root, StreamError, Value};

#[cfg(any(test, feature = "fixtures"))]
pub use container::seal;
