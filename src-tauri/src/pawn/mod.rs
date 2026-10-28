//! THE WORKSHOP (v0.7, BUILD-SPEC § v0.7): Tamsin, a Rift pawn the stone
//! summons in WSL to build mods with. Compiled only with the `pawn` feature
//! (RS-10): the public build carries none of it.
//!
//! - [`bring`]: RS-2's second exception, one built artifact into the mods
//!   folder by the digest the player was shown.
//! - [`fence`]: the fence's argv, filter, settings and ground, and the mount
//!   validators (RS-9). Pure.
//! - [`mailbox`]: what the stone does with a request: validation, the tool
//!   table, the run's idempotent ledger and its retirement.
//! - [`folder`]: the run's two mailbox folders, by handle: no link, reparse
//!   point or non-regular file is ever read, and no answer replaces a file.

pub mod bring;
pub mod fence;
pub mod folder;
pub mod mailbox;
