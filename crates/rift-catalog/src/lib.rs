//! rift-catalog — the library: every package in every root, every option in
//! every package, the addon tree Fluffy hides behind nested menus, search
//! across all of it (names, descriptions, breadcrumbs AND the file paths
//! inside the archives), and conflicts between what is installed.
//!
//! | module | what it does |
//! |--------|--------------|
//! | [`modinfo`] | Fluffy's `modinfo.ini`, parsed leniently |
//! | [`classify`] | one package's options and the payload of each |
//! | [`tree`] | `addonfor` parents and breadcrumb trails across the library |
//! | [`scan`] | a root's candidates; one package off disk |
//! | [`catalog`] | every root scanned in parallel, the cache, the index |
//! | [`search`] | the search index, and the Nexus labels it filters by |
//! | [`conflicts`] | which installed options put a file at the same game path |
//!
//! See `docs/BUILD-SPEC.md` § rift-catalog for the contract.

pub mod catalog;
pub mod classify;
pub mod conflicts;
pub mod modinfo;
pub mod scan;
pub mod search;
pub mod tree;

#[cfg(test)]
mod test_support;

pub use catalog::{
    CachedPackage, Catalog, CatalogCache, PassedOverCandidate, ScanProgress, ScanStats,
};
pub use conflicts::{Conflict, ConflictKey, Layer};
pub use scan::ScannedPackage;
pub use search::{
    LabelCount, LabelFacets, MatchedField, PackageLabels, SearchFilters, SearchHit, SearchIndex,
    SearchQuery,
};
