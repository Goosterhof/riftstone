//! rift-core — the shared domain contract of the Riftstone.
//!
//! Every other crate speaks these types. Nothing here touches the disk or the
//! network: this crate is the vocabulary, not the machinery.
//!
//! The words, plainly (the Riftstone keeps its core nouns generic on purpose —
//! "generic gives meaning faster" — and spends its Dragon's Dogma voice on the
//! frame, the empty states and the errors):
//!
//! - The **mods folder** is the one folder the Riftstone reads mods from: the
//!   `mods` folder beside the installed `riftstone.exe`.
//! - A **package** is one archive (zip / 7z / rar) or one plain folder directly
//!   inside the mods folder — one thing you downloaded.
//! - An **option** is one installable unit inside a package. A plain mod is a
//!   package with one option. A collection like a 281-variant armour pack is a
//!   package with 281 options, wired into menus by modinfo.ini's `addonfor` and
//!   `NameAsBundle` keys.
//! - The **payload** of an option is the list of files that land in the game,
//!   each with a [`DeployTarget`].

pub mod dd2;
pub mod error;
pub mod model;
pub mod nexus_name;
pub mod path;
pub mod source;

pub use error::{NexusReason, RiftError, SaveReason};
pub use model::*;
pub use path::{fnv1a64, lower_key, normalize_entry_path};
pub use source::{ExtractItem, ExtractProgress, PackageSource};
