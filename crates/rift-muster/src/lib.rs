//! rift-muster — the Muster's model (v0.4): the party in a Dragon's Dogma 2 save slot, and the
//! game's own names for what it carries.
//!
//! ```text
//! a slot file ─ rift_save::open ─ rift_save::stream::parse ─► party::read ─► Muster (ids)
//! the game's pak ─ rift_pak::game ─ rift_pak::msg ─────────► Codex (names by id)
//! ```
//!
//! The spine joins the two into the window's view. Names that are the player's own (the
//! Arisen's, the pawns') travel as [`Private`], which never prints. Nothing here writes.

pub mod codex;
pub mod party;
mod private;
pub mod saves;

pub use codex::{Codex, CodexError};
pub use party::{Creator, Gains, Inclination, Member, Muster, MusterError, PawnTraits, Role};
pub use private::Private;

/// Why a slot file gave no party.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SlotError {
    #[error(transparent)]
    Save(#[from] rift_save::SaveError),
    #[error(transparent)]
    Stream(#[from] rift_save::StreamError),
    #[error(transparent)]
    Muster(#[from] MusterError),
}

/// Open a slot file's bytes with the Steam account id it was found under, and read its party.
pub fn read_slot(bytes: &[u8], account_id: u32) -> Result<Muster, SlotError> {
    let payload = rift_save::open(bytes, account_id)?;
    let roots = rift_save::stream::parse(&payload)?;
    Ok(party::read(&roots)?)
}

/// Invented saves and an invented game pak, for tests here and in the spine.
#[cfg(any(test, feature = "fixtures"))]
pub mod fixtures;

#[cfg(test)]
mod tests;
