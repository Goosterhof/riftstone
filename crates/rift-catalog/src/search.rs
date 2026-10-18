//! Search across every option of every package: names, descriptions,
//! authors, categories, breadcrumbs, bundles, package names AND the file
//! paths inside the archives.
//!
//! ## The query
//!
//! Lower-cased and split on whitespace. `"black necklace"` (quoted) stays one
//! term; an unclosed quote runs to the end. **Every term must match
//! somewhere** — any field, or any file path of the option (substring match).
//! A term written `-cape` (or `-"ermine cape"`) must match NOWHERE: it removes
//! options instead of finding them. A term holding `/` matches across folders
//! (`mesh/top_314`); a backslash reads as `/`, so a path pasted from Explorer
//! works.
//!
//! ## The score
//!
//! Each term scores its best field: name 10, trail 6, bundle 5, package name 4,
//! category 4, Nexus label 4, author 3, version 2, description 2, file 1 —
//! times 1.5 when the term starts a word in that field, times 2 when it is a
//! whole word. The option's score is the sum over its terms; hits sort by
//! score, then name, then library order. The trail field holds the menu
//! ancestors only (the package name and the bundle score as their own fields).
//! `matched` lists every field any term hit, best first (a tie keeps the order
//! above); `file_matches` lists up to five paths, the ones matching the most
//! terms first, then those matching beyond the option's own folder, then
//! payload before skipped files.
//!
//! ## Nexus labels (v0.2)
//!
//! A package linked to a Nexus mod carries that mod's labels — its category
//! (Nexus gives every mod one) and its tags (often none) — and every option of
//! the package inherits them. They are not part of the package: the spine
//! learns them from an update check and hands them over with
//! [`SearchIndex::set_labels`], which replaces the label layer alone (the
//! options, texts and paths are never rebuilt for it). Labels are
//!
//! - a text field, [`MatchedField::NexusLabel`]: `utilities` finds a mod
//!   tagged *Utilities for Players*, and `-utilities` removes it;
//! - two filters: [`SearchFilters::categories`] is ANY-of (a mod has one
//!   category, so two chips widen) and [`SearchFilters::tags`] is ALL-of (each
//!   chip narrows). Both compare folded like every field — Unicode
//!   lower-case, whitespace collapsed — and a package without labels fails
//!   any label filter that is set;
//! - facets: [`SearchIndex::label_facets`] counts the installable options
//!   under every category and tag, so a chip predicts what it finds.
//!
//! Display keeps Nexus's own spelling: hits carry the package's labels as
//! given ([`SearchHit::labels`]), and a facet shows the first spelling met in
//! library order.
//!
//! ## Why it is fast
//!
//! A library repeats itself: forty variants share a description, a thousand
//! paths share `natives/stm/appsystem/…`. The index interns every distinct
//! lower-cased field text and every distinct path segment ONCE, each with a
//! 128-bit signature of its character pairs. A term is matched against the
//! distinct strings only — its own signature rules out nearly all of them
//! before a single byte is compared — then fans out to options through a
//! segment → option posting list. Per-hit details are worked out only for the
//! hits that survive the limit. Measured in release on the WSL bench: 10,000
//! options and 200,000 paths answer in 0.3–7 ms and index in ~0.2 s; 300
//! options and 3,000 paths answer in under 0.4 ms (see the speed tests).

use std::cmp::{Ordering, Reverse};
use std::collections::HashMap;
use std::fmt;
use std::hash::{BuildHasherDefault, Hasher};
use std::ops::Range;

use serde::{Deserialize, Serialize};

use rift_core::{ArchiveEntry, ModOption, OptionId, Package, PackageId};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchFilters {
    /// `Some(true)` installed only, `Some(false)` not installed only.
    pub installed: Option<bool>,
    /// Include pure menu entries (default false — they install nothing).
    pub include_menu_only: bool,
    /// v0.2: Nexus categories, ANY-of — a mod has one category, so each entry
    /// widens. Folded before comparing; blank entries are ignored; empty = no
    /// filter. A package without labels never passes a set filter.
    #[serde(default)]
    pub categories: Vec<String>,
    /// v0.2: Nexus tags, ALL-of — each entry narrows. Folded before comparing;
    /// blank entries are ignored; empty = no filter.
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub text: String,
    pub filters: SearchFilters,
    /// Maximum hits returned; 0 = no cap, every match (v0.3 — v0.2 read 0 as
    /// 200). A positive limit keeps the best: ranked hits by score, an empty
    /// text the first in library order.
    pub limit: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MatchedField {
    Name,
    Trail,
    Bundle,
    Category,
    Author,
    Description,
    PackageName,
    Version,
    File,
    /// v0.2: the linked Nexus mod's category or one of its tags.
    NexusLabel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub option_id: OptionId,
    pub package_id: PackageId,
    pub score: f32,
    /// Which fields matched, best first.
    pub matched: Vec<MatchedField>,
    /// Up to five payload/entry paths (relative to the package) that matched.
    pub file_matches: Vec<String>,
    /// v0.2: the package's Nexus labels in Nexus's spelling, as the last
    /// [`SearchIndex::set_labels`] gave them — `None` for a package it gave
    /// nothing for (unlinked, or never checked).
    #[serde(default)]
    pub labels: Option<PackageLabels>,
}

/// What Nexus says about the mod a package is linked to (v0.2): its category,
/// its tags and its adult flag. Every option of the package inherits them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageLabels {
    /// Nexus gives every mod one; `None` when the check predates labels.
    #[serde(default)]
    pub category: Option<String>,
    /// Often none.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Nexus flags the mod as adult content.
    #[serde(default)]
    pub adult: bool,
}

impl PackageLabels {
    /// Outer whitespace trimmed, blank labels dropped, a tag repeated in
    /// another case kept once (its first spelling).
    fn tidied(self) -> PackageLabels {
        let category = self
            .category
            .map(|category| category.trim().to_string())
            .filter(|category| !category.is_empty());
        let mut folded: Vec<String> = Vec::with_capacity(self.tags.len());
        let mut tags = Vec::with_capacity(self.tags.len());
        for tag in self.tags {
            let key = normalize_text(&tag);
            if !key.is_empty() && !folded.contains(&key) {
                folded.push(key);
                tags.push(tag.trim().to_string());
            }
        }
        PackageLabels {
            category,
            tags,
            adult: self.adult,
        }
    }

    /// Says nothing at all: no category, no tag, not adult.
    fn is_blank(&self) -> bool {
        self.category.is_none() && self.tags.is_empty() && !self.adult
    }
}

/// One category or tag and how many options carry it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelCount {
    /// Nexus's spelling (the first one met in library order).
    pub name: String,
    /// Installable options carrying it (menu entries excluded): what a
    /// filter on it alone finds with no text and the default filters.
    pub options: usize,
}

/// Every category and tag the library's labels hold (v0.2), for the filter
/// chips. A label only menu entries carry is left out: its chip would find
/// nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelFacets {
    /// Count descending, then name.
    pub categories: Vec<LabelCount>,
    /// Count descending, then name.
    pub tags: Vec<LabelCount>,
    /// Packages with at least one installable option that a label filter
    /// hides: no category and no tag (unlinked, never checked, or checked
    /// before labels existed).
    pub unlabeled_packages: usize,
}

const MAX_FILE_MATCHES: usize = 5;
const NO_TEXT: u32 = u32::MAX;
const FILE_WEIGHT: f32 = 1.0;
/// A Nexus label weighs between a bundle (5) and an author (3).
const LABEL_WEIGHT: f32 = 4.0;

/// The text fields of an option, in slot order, with their weights.
const SLOTS: [(MatchedField, f32); 9] = [
    (MatchedField::Name, 10.0),
    (MatchedField::Trail, 6.0),
    (MatchedField::Bundle, 5.0),
    (MatchedField::PackageName, 4.0), // display name
    (MatchedField::PackageName, 4.0), // file name
    (MatchedField::Category, 4.0),
    (MatchedField::Author, 3.0),
    (MatchedField::Version, 2.0),
    (MatchedField::Description, 2.0),
];

/// Tie order for `matched`: by weight, then this order.
const FIELD_ORDER: [MatchedField; 10] = [
    MatchedField::Name,
    MatchedField::Trail,
    MatchedField::Bundle,
    MatchedField::PackageName,
    MatchedField::Category,
    MatchedField::NexusLabel,
    MatchedField::Author,
    MatchedField::Version,
    MatchedField::Description,
    MatchedField::File,
];

fn field_rank(field: MatchedField) -> usize {
    FIELD_ORDER
        .iter()
        .position(|f| *f == field)
        .unwrap_or(FIELD_ORDER.len())
}

/// How well a term sits in a text: 0 no match, 2 inside a word (×1), 3 at the
/// start of a word (×1.5), 4 a whole word (×2). Score = weight × quality / 2.
type Quality = u8;

fn weighted(weight: f32, quality: Quality) -> f32 {
    weight * f32::from(quality) / 2.0
}

// ---------------------------------------------------------------------------
// The index
// ---------------------------------------------------------------------------

/// An in-memory index. Rebuilt whenever the catalog changes (cheap: a few
/// thousand options and tens of thousands of paths build in milliseconds).
/// The Nexus labels are a layer of their own, replaced by
/// [`SearchIndex::set_labels`] without touching the rest.
#[derive(Clone, Default)]
pub struct SearchIndex {
    options: Vec<OptionRecord>,
    /// Every distinct package, in library order (an option's `package` slot).
    packages: Vec<PackageSlot>,
    /// Package id → its slot.
    package_slots: FastMap<PackageId, u32>,
    /// What Nexus says about each package, from the last `set_labels`.
    labels: LabelLayer,
    /// Distinct field texts, lower-cased, whitespace collapsed.
    texts: SignedPool,
    /// Distinct path segments, lower-cased, whitespace collapsed.
    segments: SignedPool,
    /// Distinct path segments as spelled (for `file_matches`).
    spelled_segments: Pool,
    /// Spelled segment id → lower-cased segment id.
    segment_lower: Vec<u32>,
    /// Every path as a run of spelled segment ids.
    path_segments: Vec<u32>,
    paths: Vec<PathRecord>,
    /// Lower-cased segment id → the options whose paths use it.
    postings: Postings,
}

#[derive(Debug, Clone)]
struct OptionRecord {
    id: OptionId,
    /// The package's slot in `SearchIndex::packages`.
    package: u32,
    name_key: String,
    menu_only: bool,
    texts: [u32; SLOTS.len()],
    paths: Range<u32>,
    /// Segments in the option root — matches beyond it are in the option's own files.
    root_depth: u32,
}

#[derive(Debug, Clone)]
struct PathRecord {
    segments: Range<u32>,
    payload: bool,
}

#[derive(Debug, Clone)]
struct PackageSlot {
    id: PackageId,
    /// Its options that install something (menu entries excluded).
    installable: u32,
}

/// The Nexus labels of the packages: built from scratch by every
/// `set_labels` (a few hundred distinct strings at most), so labels that
/// changed or went away leave nothing behind.
#[derive(Debug, Clone, Default)]
struct LabelLayer {
    /// Distinct folded label texts — categories and tags alike; which one a
    /// text is depends on the slot that names it.
    texts: SignedPool,
    /// Folded text → its id.
    ids: FastMap<String, u32>,
    /// Per text: the spelling the facets show (the first met in library order).
    spelled: Vec<String>,
    /// Per package slot (empty until labels are set).
    packages: Vec<Option<LabeledPackage>>,
    /// Every labeled package's tag text ids, flattened.
    tag_ids: Vec<u32>,
}

#[derive(Debug, Clone)]
struct LabeledPackage {
    /// As shown: Nexus's spelling, tidied.
    labels: PackageLabels,
    /// The category's text id (`NO_TEXT`: none).
    category: u32,
    /// Its tags' text ids: a range of `LabelLayer::tag_ids`.
    tags: Range<u32>,
}

impl LabeledPackage {
    /// Something a label filter can find it by.
    fn filterable(&self) -> bool {
        self.category != NO_TEXT || !self.tags.is_empty()
    }
}

impl LabelLayer {
    fn package(&self, slot: u32) -> Option<&LabeledPackage> {
        self.packages.get(slot as usize).and_then(Option::as_ref)
    }

    fn tags_of(&self, labeled: &LabeledPackage) -> &[u32] {
        &self.tag_ids[labeled.tags.start as usize..labeled.tags.end as usize]
    }

    /// The text ids of a package's category and tags.
    fn text_ids<'a>(&'a self, labeled: &'a LabeledPackage) -> impl Iterator<Item = u32> + 'a {
        (labeled.category != NO_TEXT)
            .then_some(labeled.category)
            .into_iter()
            .chain(self.tags_of(labeled).iter().copied())
    }

    fn add(&mut self, labels: PackageLabels) -> LabeledPackage {
        let category = labels
            .category
            .as_deref()
            .map_or(NO_TEXT, |category| self.text(category));
        let start = to_u32(self.tag_ids.len());
        for tag in &labels.tags {
            let id = self.text(tag);
            self.tag_ids.push(id);
        }
        LabeledPackage {
            category,
            tags: start..to_u32(self.tag_ids.len()),
            labels,
        }
    }

    fn text(&mut self, spelled: &str) -> u32 {
        let folded = normalize_text(spelled);
        if let Some(&id) = self.ids.get(&folded) {
            return id;
        }
        let id = self.texts.push(&folded);
        self.ids.insert(folded, id);
        self.spelled.push(spelled.to_string());
        id
    }

    /// The labels holding a count, best first: count descending, then name.
    fn ranked(&self, counts: &[usize]) -> Vec<LabelCount> {
        let mut ranked: Vec<(usize, u32)> = counts
            .iter()
            .enumerate()
            .filter(|(_, &options)| options > 0)
            .map(|(id, &options)| (options, to_u32(id)))
            .collect();
        ranked.sort_unstable_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| self.texts.get(a.1).cmp(self.texts.get(b.1)))
        });
        ranked
            .into_iter()
            .map(|(options, id)| LabelCount {
                name: self.spelled[id as usize].clone(),
                options,
            })
            .collect()
    }
}

/// Filter values folded like the index's texts; blanks and repeats dropped.
fn folded_values(values: &[String]) -> Vec<String> {
    let mut folded: Vec<String> = Vec::with_capacity(values.len());
    for value in values {
        let key = normalize_text(value);
        if !key.is_empty() && !folded.contains(&key) {
            folded.push(key);
        }
    }
    folded
}

/// An append-only pool of strings.
#[derive(Debug, Clone, Default)]
struct Pool {
    text: String,
    spans: Vec<(u32, u32)>,
}

impl Pool {
    fn push(&mut self, s: &str) -> u32 {
        let id = to_u32(self.spans.len());
        let start = to_u32(self.text.len());
        self.text.push_str(s);
        self.spans.push((start, to_u32(self.text.len())));
        id
    }

    fn get(&self, id: u32) -> &str {
        let (start, end) = self.spans[id as usize];
        &self.text[start as usize..end as usize]
    }

    fn len(&self) -> usize {
        self.spans.len()
    }
}

/// A pool whose strings carry character-pair signatures.
#[derive(Debug, Clone, Default)]
struct SignedPool {
    pool: Pool,
    signatures: Vec<u128>,
}

impl SignedPool {
    fn push(&mut self, s: &str) -> u32 {
        self.signatures.push(signature(s));
        self.pool.push(s)
    }

    fn get(&self, id: u32) -> &str {
        self.pool.get(id)
    }

    fn len(&self) -> usize {
        self.pool.len()
    }

    /// The quality of `needle` in every string (0 where it does not occur).
    fn qualities(&self, needle: &str) -> Vec<Quality> {
        let wanted = signature(needle);
        let mut out = vec![0; self.len()];
        for (id, signature) in self.signatures.iter().enumerate() {
            if signature & wanted == wanted {
                out[id] = quality(self.get(to_u32(id)), needle);
            }
        }
        out
    }
}

/// Compressed rows: `items[offsets[k]..offsets[k + 1]]` belong to key `k`.
#[derive(Debug, Clone, Default)]
struct Postings {
    offsets: Vec<u32>,
    items: Vec<u32>,
}

impl Postings {
    fn row(&self, key: usize) -> &[u32] {
        match (self.offsets.get(key), self.offsets.get(key + 1)) {
            (Some(&start), Some(&end)) => &self.items[start as usize..end as usize],
            _ => &[],
        }
    }
}

impl fmt::Debug for SearchIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SearchIndex")
            .field("options", &self.options.len())
            .field("paths", &self.paths.len())
            .field("distinct_texts", &self.texts.len())
            .field("distinct_segments", &self.segments.len())
            .field("packages", &self.packages.len())
            .field("distinct_labels", &self.labels.texts.len())
            .finish()
    }
}

impl SearchIndex {
    /// Index every option of `packages` (in library order). The paths searched
    /// are each option's payload and skipped files — package-relative entry
    /// paths. `entries` (the full listings) is accepted for the catalog's
    /// convenience; files outside every option (a README beside the mod
    /// folders) belong to no option and so cannot be a hit.
    pub fn build(packages: &[Package], _entries: &HashMap<PackageId, Vec<ArchiveEntry>>) -> Self {
        let mut builder = Builder::default();
        let options = packages.iter().map(|p| p.options.len()).sum();
        let paths = packages
            .iter()
            .flat_map(|p| &p.options)
            .map(|o| o.payload.len() + o.skipped.len())
            .sum();
        builder.index.options.reserve_exact(options);
        builder.option_segment_rows.reserve_exact(options);
        builder.index.paths.reserve_exact(paths);
        builder.spelled_ids.reserve(paths);
        builder.lower_ids.reserve(paths);
        for package in packages {
            builder.add_package(package);
        }
        builder.finish()
    }

    /// How many options the index holds.
    pub fn option_count(&self) -> usize {
        self.options.len()
    }

    /// How many file paths the index holds.
    pub fn path_count(&self) -> usize {
        self.paths.len()
    }

    /// Give each package the labels of the Nexus mod it is linked to (v0.2),
    /// replacing whatever the last call gave: a package missing from `labels`
    /// has none from now on, and an id the index does not hold is ignored.
    /// Labels are tidied (outer whitespace trimmed, blank ones dropped, a tag
    /// repeated in another case kept once); an entry left with no category,
    /// no tag and no adult flag counts as no entry.
    ///
    /// Cheap: only the label layer is rebuilt — one lookup per package plus
    /// its few labels — never the options, texts or paths (the speed tests
    /// time it). The spine calls it after every scan and every Nexus check.
    pub fn set_labels(&mut self, mut labels: HashMap<PackageId, PackageLabels>) {
        let mut layer = LabelLayer::default();
        layer.packages.reserve_exact(self.packages.len());
        for slot in &self.packages {
            let labeled = labels
                .remove(&slot.id)
                .map(PackageLabels::tidied)
                .filter(|given| !given.is_blank())
                .map(|given| layer.add(given));
            layer.packages.push(labeled);
        }
        self.labels = layer;
    }

    /// A package's labels as the last [`SearchIndex::set_labels`] gave them
    /// (tidied) — `None` when it gave nothing for it.
    pub fn labels(&self, package: &PackageId) -> Option<&PackageLabels> {
        let slot = *self.package_slots.get(package)?;
        self.labels.package(slot).map(|labeled| &labeled.labels)
    }

    /// The filter chips (v0.2): every category and every tag with the number
    /// of installable options carrying it (menu entries excluded, installed
    /// or not — what a chip alone finds with no text and the default
    /// filters), and the packages a label filter would hide. O(packages +
    /// labels); nothing is cached, so it always agrees with the last
    /// `set_labels`.
    pub fn label_facets(&self) -> LabelFacets {
        let distinct = self.labels.texts.len();
        let mut categories = vec![0usize; distinct];
        let mut tags = vec![0usize; distinct];
        let mut unlabeled_packages = 0;
        for (slot, package) in self.packages.iter().enumerate() {
            if package.installable == 0 {
                continue; // no chip finds it, and no chip hides it
            }
            let options = package.installable as usize;
            let labeled = self
                .labels
                .package(to_u32(slot))
                .filter(|labeled| labeled.filterable());
            let Some(labeled) = labeled else {
                unlabeled_packages += 1;
                continue;
            };
            if labeled.category != NO_TEXT {
                categories[labeled.category as usize] += options;
            }
            for &tag in self.labels.tags_of(labeled) {
                tags[tag as usize] += options;
            }
        }
        LabelFacets {
            categories: self.labels.ranked(&categories),
            tags: self.labels.ranked(&tags),
            unlabeled_packages,
        }
    }

    /// Per package slot: does it pass the label filters? `None` when neither
    /// is set. Categories are ANY-of, tags ALL-of; a package without labels
    /// passes no filter.
    fn label_allowed(&self, filters: &SearchFilters) -> Option<Vec<bool>> {
        let categories = folded_values(&filters.categories);
        let tags = folded_values(&filters.tags);
        if categories.is_empty() && tags.is_empty() {
            return None;
        }
        let id = |folded: &String| self.labels.ids.get(folded).copied();
        let wanted_categories: Vec<u32> = categories.iter().filter_map(id).collect();
        // ALL-of: a tag no package carries leaves nothing to find.
        let wanted_tags: Option<Vec<u32>> = tags.iter().map(id).collect();
        let allowed = (0..self.packages.len())
            .map(|slot| {
                let Some(labeled) = self.labels.package(to_u32(slot)) else {
                    return false;
                };
                let category_ok =
                    categories.is_empty() || wanted_categories.contains(&labeled.category);
                category_ok
                    && wanted_tags.as_ref().is_some_and(|wanted| {
                        let own = self.labels.tags_of(labeled);
                        wanted.iter().all(|tag| own.contains(tag))
                    })
            })
            .collect();
        Some(allowed)
    }

    /// Per package slot, the best quality of `needle` among its labels —
    /// empty when no label holds it.
    fn label_quality(&self, needle: &str) -> Vec<Quality> {
        let per_text = self.labels.texts.qualities(needle);
        if per_text.iter().all(|&quality| quality == 0) {
            return Vec::new();
        }
        self.labels
            .packages
            .iter()
            .map(|labeled| {
                labeled.as_ref().map_or(0, |labeled| {
                    self.labels
                        .text_ids(labeled)
                        .map(|id| per_text[id as usize])
                        .max()
                        .unwrap_or(0)
                })
            })
            .collect()
    }

    /// Run a query. `is_installed` tells the index which options are installed
    /// (the deploy journal owns that truth). An empty query text returns every
    /// option that passes the filters, in library order. The label filters
    /// read the labels the last [`SearchIndex::set_labels`] gave.
    pub fn search(
        &self,
        query: &SearchQuery,
        is_installed: &dyn Fn(&OptionId) -> bool,
    ) -> Vec<SearchHit> {
        let limit = if query.limit == 0 {
            usize::MAX
        } else {
            query.limit
        };
        let filters = &query.filters;
        let label_allowed = self.label_allowed(filters);
        if label_allowed
            .as_ref()
            .is_some_and(|allowed| !allowed.contains(&true))
        {
            return Vec::new(); // a label filter no package passes
        }

        let terms = parse_terms(&query.text);
        let positives: Vec<TermMatch> = terms
            .iter()
            .filter(|t| !t.negated)
            .map(|t| self.match_term(&t.text))
            .collect();
        if positives.iter().any(TermMatch::is_empty) {
            return Vec::new(); // a term that matches nothing anywhere: AND fails everywhere
        }
        let negatives: Vec<TermMatch> = terms
            .iter()
            .filter(|t| t.negated)
            .map(|t| self.match_term(&t.text))
            .collect();
        let ranked = !positives.is_empty();

        let mut candidates: Vec<(u32, f32)> = Vec::new();
        'options: for (index, option) in self.options.iter().enumerate() {
            if (option.menu_only && !filters.include_menu_only)
                || label_allowed
                    .as_ref()
                    .is_some_and(|allowed| !allowed[option.package as usize])
            {
                continue;
            }
            let mut score = 0.0;
            for term in &positives {
                let best = self.term_score(index, option, term);
                if best == 0.0 {
                    continue 'options;
                }
                score += best;
            }
            if negatives
                .iter()
                .any(|term| self.term_score(index, option, term) > 0.0)
            {
                continue;
            }
            if filters
                .installed
                .is_some_and(|wanted| is_installed(&option.id) != wanted)
            {
                continue;
            }
            candidates.push((to_u32(index), score));
            if !ranked && candidates.len() == limit {
                break;
            }
        }

        if ranked {
            let order = |a: &(u32, f32), b: &(u32, f32)| -> Ordering {
                b.1.total_cmp(&a.1)
                    .then_with(|| {
                        self.options[a.0 as usize]
                            .name_key
                            .cmp(&self.options[b.0 as usize].name_key)
                    })
                    .then(a.0.cmp(&b.0))
            };
            if candidates.len() > limit {
                candidates.select_nth_unstable_by(limit - 1, order);
                candidates.truncate(limit);
            }
            candidates.sort_unstable_by(order);
        }

        candidates
            .into_iter()
            .map(|(index, score)| self.hit(index as usize, score, &positives))
            .collect()
    }

    /// The best weighted quality of one term in one option (0 = no match).
    fn term_score(&self, index: usize, option: &OptionRecord, term: &TermMatch) -> f32 {
        let mut best = 0.0f32;
        for (slot, &text) in option.texts.iter().enumerate() {
            if text != NO_TEXT {
                let quality = term.text_quality[text as usize];
                if quality > 0 {
                    best = best.max(weighted(SLOTS[slot].1, quality));
                }
            }
        }
        let file = term.file_quality[index];
        if file > 0 {
            best = best.max(weighted(FILE_WEIGHT, file));
        }
        let label = term.label(option);
        if label > 0 {
            best = best.max(weighted(LABEL_WEIGHT, label));
        }
        best
    }

    fn match_term(&self, needle: &str) -> TermMatch {
        let text_quality = self.texts.qualities(needle);
        let label_quality = self.label_quality(needle);
        let mut file_quality = vec![0; self.options.len()];
        let slash = needle.contains('/');
        let segment_quality = if slash {
            // Across folders: only options holding the term's longest piece can match.
            let piece = needle
                .split('/')
                .max_by_key(|piece| piece.len())
                .unwrap_or("");
            let holders =
                (!piece.is_empty()).then(|| self.options_with(&self.segments.qualities(piece)));
            for (index, option) in self.options.iter().enumerate() {
                if holders.as_ref().is_some_and(|holders| holders[index] == 0) {
                    continue;
                }
                for path in &self.paths[option.paths.start as usize..option.paths.end as usize] {
                    let quality = quality(&self.lower_path(path), needle);
                    file_quality[index] = file_quality[index].max(quality);
                }
            }
            Vec::new()
        } else {
            let qualities = self.segments.qualities(needle);
            for (segment, &quality) in qualities.iter().enumerate() {
                if quality > 0 {
                    for &option in self.postings.row(segment) {
                        let slot = &mut file_quality[option as usize];
                        *slot = (*slot).max(quality);
                    }
                }
            }
            qualities
        };
        TermMatch {
            needle: needle.to_string(),
            slash,
            text_quality,
            segment_quality,
            file_quality,
            label_quality,
        }
    }

    /// Per option, the best quality among the segments it uses.
    fn options_with(&self, segment_quality: &[Quality]) -> Vec<Quality> {
        let mut best = vec![0; self.options.len()];
        for (segment, &quality) in segment_quality.iter().enumerate() {
            if quality > 0 {
                for &option in self.postings.row(segment) {
                    best[option as usize] = best[option as usize].max(quality);
                }
            }
        }
        best
    }

    fn lower_path(&self, path: &PathRecord) -> String {
        let mut out = String::new();
        for &spelled in self.path_ids(path) {
            if !out.is_empty() {
                out.push('/');
            }
            out.push_str(self.segments.get(self.segment_lower[spelled as usize]));
        }
        out
    }

    fn spelled_path(&self, path: &PathRecord) -> String {
        let mut out = String::new();
        for &spelled in self.path_ids(path) {
            if !out.is_empty() {
                out.push('/');
            }
            out.push_str(self.spelled_segments.get(spelled));
        }
        out
    }

    fn path_ids(&self, path: &PathRecord) -> &[u32] {
        &self.path_segments[path.segments.start as usize..path.segments.end as usize]
    }

    fn hit(&self, index: usize, score: f32, positives: &[TermMatch]) -> SearchHit {
        let option = &self.options[index];

        let mut field_scores = [0.0f32; FIELD_ORDER.len()];
        for term in positives {
            for (slot, &text) in option.texts.iter().enumerate() {
                if text != NO_TEXT {
                    let quality = term.text_quality[text as usize];
                    if quality > 0 {
                        let rank = field_rank(SLOTS[slot].0);
                        field_scores[rank] =
                            field_scores[rank].max(weighted(SLOTS[slot].1, quality));
                    }
                }
            }
            let file = term.file_quality[index];
            if file > 0 {
                let rank = field_rank(MatchedField::File);
                field_scores[rank] = field_scores[rank].max(weighted(FILE_WEIGHT, file));
            }
            let label = term.label(option);
            if label > 0 {
                let rank = field_rank(MatchedField::NexusLabel);
                field_scores[rank] = field_scores[rank].max(weighted(LABEL_WEIGHT, label));
            }
        }
        let mut matched: Vec<(usize, f32)> = field_scores
            .iter()
            .enumerate()
            .filter(|(_, score)| **score > 0.0)
            .map(|(rank, score)| (rank, *score))
            .collect();
        matched.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));

        SearchHit {
            option_id: option.id.clone(),
            package_id: self.packages[option.package as usize].id.clone(),
            score,
            matched: matched
                .into_iter()
                .map(|(rank, _)| FIELD_ORDER[rank])
                .collect(),
            file_matches: self.file_matches(index, positives),
            labels: self
                .labels
                .package(option.package)
                .map(|labeled| labeled.labels.clone()),
        }
    }

    /// Up to five matching paths: most terms first, then matches beyond the
    /// option's own folder, then payload before skipped, then archive order.
    fn file_matches(&self, index: usize, positives: &[TermMatch]) -> Vec<String> {
        let option = &self.options[index];
        let touching: Vec<&TermMatch> = positives
            .iter()
            .filter(|term| term.file_quality[index] > 0)
            .collect();
        if touching.is_empty() {
            return Vec::new();
        }

        let mut found: Vec<FileRank> = Vec::new();
        let mut perfect = 0;
        let first = option.paths.start as usize;
        for (order, path) in self.paths[first..option.paths.end as usize]
            .iter()
            .enumerate()
        {
            let ids = self.path_ids(path);
            let (mut terms, mut beyond_root) = (0, false);
            for term in &touching {
                let (hit, beyond) = if term.slash {
                    let hit = quality(&self.lower_path(path), &term.needle) > 0;
                    (hit, hit)
                } else {
                    let mut hit = false;
                    let mut beyond = false;
                    for (depth, &spelled) in ids.iter().enumerate() {
                        if term.segment_quality[self.segment_lower[spelled as usize] as usize] > 0 {
                            hit = true;
                            if to_u32(depth) >= option.root_depth {
                                beyond = true;
                                break;
                            }
                        }
                    }
                    (hit, beyond)
                };
                if hit {
                    terms += 1;
                    beyond_root |= beyond;
                }
            }
            if terms == 0 {
                continue;
            }
            if terms == touching.len() && beyond_root && path.payload {
                perfect += 1;
            }
            found.push((
                Reverse(terms),
                Reverse(beyond_root),
                Reverse(path.payload),
                order,
            ));
            if perfect == MAX_FILE_MATCHES {
                break; // nothing later can outrank these
            }
        }
        found.sort_unstable();
        found
            .into_iter()
            .take(MAX_FILE_MATCHES)
            .map(|(.., order)| self.spelled_path(&self.paths[first + order]))
            .collect()
    }
}

/// How a path ranks among an option's file matches — terms matched, matched
/// beyond the option's own folder, payload — each reversed so an ascending
/// sort puts the best first, then archive order.
type FileRank = (Reverse<usize>, Reverse<bool>, Reverse<bool>, usize);

/// One query term, matched against the whole index.
struct TermMatch {
    needle: String,
    slash: bool,
    /// Per distinct field text.
    text_quality: Vec<Quality>,
    /// Per distinct lower-cased segment (empty for a term holding `/`).
    segment_quality: Vec<Quality>,
    /// Per option: the best quality among its file paths.
    file_quality: Vec<Quality>,
    /// Per package slot: the best quality among its Nexus labels (empty when
    /// no label holds the term).
    label_quality: Vec<Quality>,
}

impl TermMatch {
    fn is_empty(&self) -> bool {
        self.text_quality.iter().all(|q| *q == 0)
            && self.file_quality.iter().all(|q| *q == 0)
            && self.label_quality.iter().all(|q| *q == 0)
    }

    /// The best quality of the term among the option's package labels.
    fn label(&self, option: &OptionRecord) -> Quality {
        self.label_quality
            .get(option.package as usize)
            .copied()
            .unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// Building
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Builder {
    index: SearchIndex,
    text_ids: FastMap<String, u32>,
    spelled_ids: FastMap<String, u32>,
    lower_ids: FastMap<String, u32>,
    /// The previous path's segment ids: consecutive paths share folders, so
    /// the common prefix is reused without hashing.
    previous: Vec<u32>,
    /// Per lower-cased segment: 1 + the last option that used it (0 = none).
    last_user: Vec<u32>,
    /// Distinct lower-cased segments per option, flattened.
    option_segments: Vec<u32>,
    option_segment_rows: Vec<Range<usize>>,
}

/// What every option of one package shares.
struct PackageTexts {
    slot: u32,
    display_name: u32,
    file_name: u32,
}

impl Builder {
    fn add_package(&mut self, package: &Package) {
        // One slot per package id: the catalog keeps one copy of a package,
        // and a second copy would carry the same labels anyway.
        let slot = match self.index.package_slots.get(&package.id) {
            Some(&slot) => slot,
            None => {
                let slot = to_u32(self.index.packages.len());
                self.index.packages.push(PackageSlot {
                    id: package.id.clone(),
                    installable: 0,
                });
                self.index.package_slots.insert(package.id.clone(), slot);
                slot
            }
        };
        let installable = package.options.iter().filter(|o| !o.menu_only).count();
        self.index.packages[slot as usize].installable += to_u32(installable);
        let shared = PackageTexts {
            slot,
            display_name: self.text(&package.display_name),
            file_name: self.text(&package.file_name),
        };
        for option in &package.options {
            self.add_option(package, &shared, option);
        }
    }

    fn add_option(&mut self, package: &Package, shared: &PackageTexts, option: &ModOption) {
        let info = option.info.as_ref();
        let texts = [
            self.text(&option.name),
            self.text(&trail_text(package, option)),
            self.text(option.bundle.as_deref().unwrap_or("")),
            shared.display_name,
            shared.file_name,
            self.text(info.and_then(|i| i.category.as_deref()).unwrap_or("")),
            self.text(info.and_then(|i| i.author.as_deref()).unwrap_or("")),
            self.text(info.and_then(|i| i.version.as_deref()).unwrap_or("")),
            self.text(info.and_then(|i| i.description.as_deref()).unwrap_or("")),
        ];

        let first_path = to_u32(self.index.paths.len());
        for file in &option.payload {
            self.add_path(&file.entry, true);
        }
        for file in &option.skipped {
            self.add_path(&file.entry, false);
        }
        let paths = first_path..to_u32(self.index.paths.len());

        // The option's distinct segments, for the posting lists.
        let mark = to_u32(self.index.options.len()) + 1;
        let row_start = self.option_segments.len();
        for path in &self.index.paths[paths.start as usize..paths.end as usize] {
            for &spelled in
                &self.index.path_segments[path.segments.start as usize..path.segments.end as usize]
            {
                let lower = self.index.segment_lower[spelled as usize];
                let last = &mut self.last_user[lower as usize];
                if *last != mark {
                    *last = mark;
                    self.option_segments.push(lower);
                }
            }
        }
        self.option_segment_rows
            .push(row_start..self.option_segments.len());

        self.index.options.push(OptionRecord {
            id: option.id.clone(),
            package: shared.slot,
            name_key: option.name.to_lowercase(),
            menu_only: option.menu_only,
            texts,
            paths,
            root_depth: if option.root.is_empty() {
                0
            } else {
                to_u32(option.root.split('/').count())
            },
        });
    }

    fn text(&mut self, raw: &str) -> u32 {
        let normalized = normalize_text(raw);
        if normalized.is_empty() {
            return NO_TEXT;
        }
        if let Some(&id) = self.text_ids.get(&normalized) {
            return id;
        }
        let id = self.index.texts.push(&normalized);
        self.text_ids.insert(normalized, id);
        id
    }

    fn add_path(&mut self, path: &str, payload: bool) {
        let start = to_u32(self.index.path_segments.len());
        let mut shared = true;
        for (depth, segment) in path.split('/').enumerate() {
            let reused = if shared {
                self.previous
                    .get(depth)
                    .copied()
                    .filter(|&id| self.index.spelled_segments.get(id) == segment)
            } else {
                None
            };
            let id = match reused {
                Some(id) => id,
                None => {
                    shared = false;
                    self.segment(segment)
                }
            };
            self.index.path_segments.push(id);
        }
        let end = to_u32(self.index.path_segments.len());
        self.previous.clear();
        self.previous
            .extend_from_slice(&self.index.path_segments[start as usize..end as usize]);
        self.index.paths.push(PathRecord {
            segments: start..end,
            payload,
        });
    }

    fn segment(&mut self, spelled: &str) -> u32 {
        if let Some(&id) = self.spelled_ids.get(spelled) {
            return id;
        }
        let lower = normalize_text(spelled);
        let lower_id = match self.lower_ids.get(&lower) {
            Some(&id) => id,
            None => {
                let id = self.index.segments.push(&lower);
                self.lower_ids.insert(lower, id);
                self.last_user.push(0);
                id
            }
        };
        let id = self.index.spelled_segments.push(spelled);
        self.index.segment_lower.push(lower_id);
        self.spelled_ids.insert(spelled.to_string(), id);
        id
    }

    fn finish(mut self) -> SearchIndex {
        let mut offsets = vec![0u32; self.index.segments.len() + 1];
        for &segment in &self.option_segments {
            offsets[segment as usize + 1] += 1;
        }
        for k in 1..offsets.len() {
            offsets[k] += offsets[k - 1];
        }
        let mut fill = offsets.clone();
        let mut items = vec![0u32; self.option_segments.len()];
        for (option, row) in self.option_segment_rows.iter().enumerate() {
            for &segment in &self.option_segments[row.clone()] {
                let at = &mut fill[segment as usize];
                items[*at as usize] = to_u32(option);
                *at += 1;
            }
        }
        self.index.postings = Postings { offsets, items };
        self.index
    }
}

/// The menu ancestors of an option, joined: its trail without the leading
/// package name and its own bundle (those score as their own fields).
fn trail_text(package: &Package, option: &ModOption) -> String {
    let mut crumbs: &[String] = &option.trail;
    if crumbs
        .first()
        .is_some_and(|first| same_name(first, &package.display_name))
    {
        crumbs = &crumbs[1..];
    }
    if let (Some(bundle), Some(first)) = (option.bundle.as_deref(), crumbs.first()) {
        if same_name(first, bundle) {
            crumbs = &crumbs[1..];
        }
    }
    crumbs.join(" › ")
}

/// Equal ignoring case and surrounding whitespace, without allocating.
fn same_name(a: &str, b: &str) -> bool {
    a.trim()
        .chars()
        .flat_map(char::to_lowercase)
        .eq(b.trim().chars().flat_map(char::to_lowercase))
}

/// FxHash, the rustc hasher: quick on the short strings the builder interns.
/// The index holds the user's own data, so flood resistance buys nothing.
#[derive(Default, Clone, Copy)]
struct FxHasher(u64);

impl FxHasher {
    fn add(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}

impl Hasher for FxHasher {
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            let mut word = [0u8; 8];
            word.copy_from_slice(chunk);
            self.add(u64::from_le_bytes(word));
        }
        let rest = chunks.remainder();
        if !rest.is_empty() {
            let mut word = [0u8; 8];
            word[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(word));
        }
    }

    fn write_u8(&mut self, byte: u8) {
        self.add(u64::from(byte));
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<FxHasher>>;

// ---------------------------------------------------------------------------
// Matching primitives
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
struct Term {
    text: String,
    negated: bool,
}

/// Split a query into terms (see the module docs). Backslashes read as `/`,
/// so a path pasted from Explorer finds the archive's own spelling.
fn parse_terms(text: &str) -> Vec<Term> {
    let lower = text.to_lowercase().replace('\\', "/");
    let mut chars = lower.chars().peekable();
    let mut terms: Vec<Term> = Vec::new();
    loop {
        while chars.next_if(|c| c.is_whitespace()).is_some() {}
        if chars.peek().is_none() {
            break;
        }
        let mut negated = false;
        if chars.next_if_eq(&'-').is_some() {
            match chars.peek() {
                None => break,
                Some(c) if c.is_whitespace() => continue, // a lone "-"
                Some(_) => negated = true,
            }
        }
        let mut raw = String::new();
        if chars.next_if_eq(&'"').is_some() {
            for c in chars.by_ref() {
                if c == '"' {
                    break;
                }
                raw.push(c);
            }
        } else {
            while let Some(c) = chars.next_if(|c| !c.is_whitespace() && *c != '"') {
                raw.push(c);
            }
        }
        let text = collapse_whitespace(&raw);
        if !text.is_empty() && !terms.iter().any(|t| t.text == text && t.negated == negated) {
            terms.push(Term { text, negated });
        }
    }
    terms
}

/// Lower-case and collapse whitespace runs into single spaces (one
/// allocation when there is nothing to collapse — the usual case).
fn normalize_text(raw: &str) -> String {
    let lower = raw.to_lowercase();
    let mut previous_space = true; // leading whitespace is a run too
    let tidy = lower.chars().all(|c| {
        let ok = !c.is_whitespace() || (c == ' ' && !previous_space);
        previous_space = c.is_whitespace();
        ok
    }) && !(previous_space && !lower.is_empty());
    if tidy {
        lower
    } else {
        collapse_whitespace(&lower)
    }
}

fn collapse_whitespace(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for word in raw.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out
}

/// A 128-bit set of the character pairs in `text`. If `needle` occurs in
/// `haystack`, every pair of `needle` occurs in `haystack`, so
/// `sig(haystack) & sig(needle) == sig(needle)` is a free pre-check.
fn signature(text: &str) -> u128 {
    let mut signature = 0u128;
    let mut previous: Option<char> = None;
    for c in text.chars() {
        if let Some(p) = previous {
            let pair = (u64::from(u32::from(p)) << 32) | u64::from(u32::from(c));
            signature |= 1u128 << (pair.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 57);
        }
        previous = Some(c);
    }
    signature
}

/// The best [`Quality`] of `needle` anywhere in `haystack`.
fn quality(haystack: &str, needle: &str) -> Quality {
    let mut best = 0;
    for (at, _) in haystack.match_indices(needle) {
        let starts_word = haystack[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        let ends_word = haystack[at + needle.len()..]
            .chars()
            .next()
            .is_none_or(|c| !c.is_alphanumeric());
        let found = match (starts_word, ends_word) {
            (true, true) => 4,
            (true, false) => 3,
            _ => 2,
        };
        if found > best {
            best = found;
            if best == 4 {
                break;
            }
        }
    }
    best
}

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).expect("the search index holds fewer than 4 billion items")
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    use rift_core::{
        DeployTarget, ModInfo, PackageFormat, PayloadFile, PayloadKinds, SkipReason, SkippedFile,
    };

    use super::*;

    // -- fixtures -------------------------------------------------------------

    struct Opt<'a> {
        name: &'a str,
        root: &'a str,
        description: Option<&'a str>,
        author: Option<&'a str>,
        category: Option<&'a str>,
        version: Option<&'a str>,
        bundle: Option<&'a str>,
        trail: &'a [&'a str],
        payload: &'a [&'a str],
        skipped: &'a [&'a str],
        menu_only: bool,
    }

    fn opt<'a>(name: &'a str, root: &'a str) -> Opt<'a> {
        Opt {
            name,
            root,
            description: None,
            author: None,
            category: None,
            version: None,
            bundle: None,
            trail: &[],
            payload: &[],
            skipped: &[],
            menu_only: false,
        }
    }

    fn package(display_name: &str, file_name: &str, options: &[Opt<'_>]) -> Package {
        let id = PackageId::for_archive(file_name, 1);
        let options = options
            .iter()
            .map(|o| {
                let payload: Vec<PayloadFile> = o
                    .payload
                    .iter()
                    .map(|entry| PayloadFile {
                        entry: (*entry).to_string(),
                        target: DeployTarget::Loose {
                            game_path: (*entry).to_string(),
                        },
                        size: 1,
                    })
                    .collect();
                ModOption {
                    id: OptionId::new(&id, o.root),
                    package_id: id.clone(),
                    root: o.root.to_string(),
                    name: o.name.to_string(),
                    info: Some(ModInfo {
                        name: Some(o.name.to_string()),
                        description: o.description.map(str::to_string),
                        author: o.author.map(str::to_string),
                        category: o.category.map(str::to_string),
                        version: o.version.map(str::to_string),
                        ..ModInfo::default()
                    }),
                    screenshot: None,
                    kinds: PayloadKinds::tally(&payload),
                    payload,
                    skipped: o
                        .skipped
                        .iter()
                        .map(|entry| SkippedFile {
                            entry: (*entry).to_string(),
                            reason: SkipReason::Picture,
                        })
                        .collect(),
                    parent: None,
                    addon_for: None,
                    bundle: o.bundle.map(str::to_string),
                    menu_only: o.menu_only,
                    trail: std::iter::once(display_name)
                        .chain(o.bundle)
                        .chain(o.trail.iter().copied())
                        .map(str::to_string)
                        .collect(),
                }
            })
            .collect();
        Package {
            id,
            path: PathBuf::from(format!("/mods/{file_name}")),
            file_name: file_name.to_string(),
            display_name: display_name.to_string(),
            format: PackageFormat::Zip,
            size: 1,
            modified_ms: 0,
            entry_count: 0,
            options,
            nexus: None,
            problems: Vec::new(),
        }
    }

    const CAPE: &str = "Vernworth Cloaks Collection/Cloaks/Ermine Cape";

    fn library() -> Vec<Package> {
        vec![
            package(
                "Vernworth Cloaks Collection",
                "Vernworth Cloaks Collection.zip",
                &[
                    Opt {
                        menu_only: true,
                        ..opt("Cloaks", "Vernworth Cloaks Collection/Cloaks")
                    },
                    Opt {
                        menu_only: true,
                        description: Some("Menu Option. Enabling/disabling does nothing."),
                        trail: &["Cloaks"],
                        ..opt("Ermine Cape (Menu Option)", CAPE)
                    },
                    Opt {
                        bundle: Some("Ermine Capes"),
                        trail: &["Cloaks", "Ermine Cape (Menu Option)"],
                        author: Some("Vernworth Tailors"),
                        description: Some("A white ermine cape with a black necklace."),
                        payload: &[
                            "Vernworth Cloaks Collection/Cloaks/Ermine Cape/White/natives/stm/appsystem/topswb/mesh/top_314_wb_f.mesh.260421070",
                            "Vernworth Cloaks Collection/Cloaks/Ermine Cape/White/natives/stm/appsystem/topswb/tex/top_314_alba.tex.251211553",
                        ],
                        skipped: &["Vernworth Cloaks Collection/Cloaks/Ermine Cape/White/screenshot.png"],
                        ..opt("Ermine Cape - White", "Vernworth Cloaks Collection/Cloaks/Ermine Cape/White")
                    },
                    Opt {
                        bundle: Some("Ermine Capes"),
                        trail: &["Cloaks", "Ermine Cape (Menu Option)"],
                        author: Some("Vernworth Tailors"),
                        description: Some("The black cape, necklace sold separately."),
                        payload: &[
                            "Vernworth Cloaks Collection/Cloaks/Ermine Cape/Black/natives/stm/appsystem/topswb/mesh/top_315_wb_f.mesh.260421070",
                        ],
                        ..opt("Ermine Cape - Black", "Vernworth Cloaks Collection/Cloaks/Ermine Cape/Black")
                    },
                ],
            ),
            package(
                "Lantern Glow Recolor",
                "Lantern Glow-123-1-2-1711234567.zip",
                &[Opt {
                    category: Some("Lighting"),
                    version: Some("1.2"),
                    payload: &["Lantern Glow Recolor/Lantern Glow Recolor.pak"],
                    ..opt("Lantern Glow Recolor", "Lantern Glow Recolor")
                }],
            ),
            package(
                "Vernworth Banners",
                "Vernworth Banners.zip",
                &[Opt {
                    description: Some("Banners that glow like a lantern at dusk."),
                    payload: &["Vernworth Banners/natives/stm/gui/banner_red.tex.251211553"],
                    ..opt("Vernworth Banners", "Vernworth Banners")
                }],
            ),
        ]
    }

    fn index() -> SearchIndex {
        SearchIndex::build(&library(), &HashMap::new())
    }

    fn query(text: &str) -> SearchQuery {
        SearchQuery {
            text: text.to_string(),
            ..SearchQuery::default()
        }
    }

    fn nothing_installed(_: &OptionId) -> bool {
        false
    }

    fn names(index: &SearchIndex, hits: &[SearchHit]) -> Vec<String> {
        hits.iter()
            .map(|hit| {
                index
                    .options
                    .iter()
                    .find(|o| o.id == hit.option_id)
                    .map(|o| o.name_key.clone())
                    .unwrap_or_default()
            })
            .collect()
    }

    fn search(index: &SearchIndex, text: &str) -> Vec<String> {
        names(index, &index.search(&query(text), &nothing_installed))
    }

    // -- the query language ---------------------------------------------------

    #[test]
    fn terms_split_on_whitespace_and_keep_quoted_phrases_whole() {
        let terms = parse_terms(
            "  Ermine   \"Black   Necklace\" -cape -\"menu option\" - \"unclosed  phrase",
        );
        let shown: Vec<(&str, bool)> = terms.iter().map(|t| (t.text.as_str(), t.negated)).collect();
        assert_eq!(
            shown,
            [
                ("ermine", false),
                ("black necklace", false),
                ("cape", true),
                ("menu option", true),
                ("unclosed phrase", false),
            ]
        );
        assert!(parse_terms("   ").is_empty());
        assert!(parse_terms("\"\" -").is_empty());
        assert_eq!(
            parse_terms("robe robe ROBE").len(),
            1,
            "duplicates collapse"
        );
        assert_eq!(
            parse_terms("top\"314 wb\"")
                .iter()
                .map(|t| t.text.as_str())
                .collect::<Vec<_>>(),
            ["top", "314 wb"]
        );
    }

    #[test]
    fn quality_rewards_word_starts_and_whole_words() {
        assert_eq!(quality("ermine cape - white", "cape"), 4);
        assert_eq!(quality("ermine cape - white", "cap"), 3);
        assert_eq!(quality("ermine cape - white", "ape"), 2);
        assert_eq!(quality("ermine cape - white", "robe"), 0);
        assert_eq!(
            quality("escape the cape", "cape"),
            4,
            "the best occurrence counts"
        );
        assert_eq!(
            quality("top_314_wb_f.mesh", "314"),
            4,
            "underscores separate words"
        );
        assert_eq!(quality("black necklace", "black necklace"), 4);
    }

    #[test]
    fn signatures_never_rule_out_a_real_match() {
        for (haystack, needle) in [
            ("ermine cape - white", "cape - w"),
            ("top_314_wb_f.mesh", "_314_"),
            ("ドラゴンの外套", "ンの外"),
        ] {
            assert_eq!(
                signature(haystack) & signature(needle),
                signature(needle),
                "{needle} in {haystack}"
            );
        }
    }

    // -- the spec's cases -------------------------------------------------------

    #[test]
    fn every_term_must_match_somewhere() {
        let index = index();
        assert_eq!(search(&index, "ermine white"), ["ermine cape - white"]);
        assert_eq!(
            search(&index, "white tailors"),
            ["ermine cape - white"],
            "AND across fields"
        );
        assert!(search(&index, "ermine zzz").is_empty());
        assert!(search(&index, "nothing-like-this").is_empty());
    }

    #[test]
    fn quoted_phrases_match_as_one_term() {
        let index = index();
        assert_eq!(
            search(&index, "\"black necklace\""),
            ["ermine cape - white"]
        );
        assert_eq!(
            search(&index, "black necklace"),
            ["ermine cape - black", "ermine cape - white"],
            "unquoted, the black cape's description matches both words too"
        );
        assert!(search(&index, "\"necklace black\"").is_empty());
    }

    #[test]
    fn file_paths_inside_the_archive_are_searched() {
        let index = index();
        let hits = index.search(&query("top_314"), &nothing_installed);
        assert_eq!(names(&index, &hits), ["ermine cape - white"]);
        assert_eq!(hits[0].matched, [MatchedField::File]);
        assert_eq!(
            hits[0].file_matches,
            [
                "Vernworth Cloaks Collection/Cloaks/Ermine Cape/White/natives/stm/appsystem/topswb/mesh/top_314_wb_f.mesh.260421070",
                "Vernworth Cloaks Collection/Cloaks/Ermine Cape/White/natives/stm/appsystem/topswb/tex/top_314_alba.tex.251211553",
            ]
        );
        assert_eq!(search(&index, "TOP_314 alba"), ["ermine cape - white"]);
        assert_eq!(
            search(&index, "mesh/top_31"),
            ["ermine cape - black", "ermine cape - white"],
            "a term across folders"
        );
        assert_eq!(
            search(&index, "topswb/mesh/top_315"),
            ["ermine cape - black"]
        );
        assert_eq!(
            search(&index, r"topswb\mesh\top_315"),
            ["ermine cape - black"],
            "a path pasted from Explorer"
        );
    }

    #[test]
    fn filters_narrow_the_hits() {
        let index = index();
        let installed: HashSet<OptionId> = HashSet::from([OptionId::new(
            &PackageId::for_archive("Vernworth Cloaks Collection.zip", 1),
            "Vernworth Cloaks Collection/Cloaks/Ermine Cape/White",
        )]);
        let is_installed = |id: &OptionId| installed.contains(id);
        let mut q = query("ermine");
        q.filters.installed = Some(true);
        assert_eq!(
            names(&index, &index.search(&q, &is_installed)),
            ["ermine cape - white"]
        );
        q.filters.installed = Some(false);
        assert_eq!(
            names(&index, &index.search(&q, &is_installed)),
            ["ermine cape - black"]
        );

        let mut q = query("cloaks");
        assert_eq!(
            names(&index, &index.search(&q, &nothing_installed)),
            ["ermine cape - black", "ermine cape - white"],
            "menu entries hide by default"
        );
        q.filters.include_menu_only = true;
        assert_eq!(
            names(&index, &index.search(&q, &nothing_installed)),
            [
                "cloaks",
                "ermine cape (menu option)",
                "ermine cape - black",
                "ermine cape - white"
            ]
        );
    }

    #[test]
    fn a_name_hit_outranks_a_description_hit() {
        let index = index();
        let hits = index.search(&query("lantern"), &nothing_installed);
        assert_eq!(
            names(&index, &hits),
            ["lantern glow recolor", "vernworth banners"]
        );
        assert!(hits[0].score > hits[1].score);
        assert_eq!(hits[0].matched[0], MatchedField::Name);
        assert_eq!(hits[1].matched, [MatchedField::Description]);
        assert_eq!(hits[0].score, 20.0, "a whole word in the name: 10 × 2");
        assert_eq!(hits[1].score, 4.0, "a whole word in the description: 2 × 2");
    }

    #[test]
    fn whole_words_outrank_word_starts_outrank_substrings() {
        let packages = vec![package(
            "Lanterns",
            "Lanterns.zip",
            &[opt("Glowcape", "a"), opt("Cape", "b"), opt("Capeless", "c")],
        )];
        let index = SearchIndex::build(&packages, &HashMap::new());
        let hits = index.search(&query("cape"), &nothing_installed);
        assert_eq!(names(&index, &hits), ["cape", "capeless", "glowcape"]);
        let scores: Vec<f32> = hits.iter().map(|h| h.score).collect();
        assert_eq!(scores, [20.0, 15.0, 10.0]);
    }

    #[test]
    fn the_trail_finds_variants_by_their_menus() {
        let index = index();
        let hits = index.search(&query("\"menu option\""), &nothing_installed);
        assert_eq!(
            names(&index, &hits),
            ["ermine cape - black", "ermine cape - white"]
        );
        assert_eq!(hits[0].matched, [MatchedField::Trail]);
    }

    #[test]
    fn package_names_and_versions_are_fields_too() {
        let index = index();
        let hits = index.search(&query("1711234567"), &nothing_installed);
        assert_eq!(names(&index, &hits), ["lantern glow recolor"]);
        assert_eq!(hits[0].matched, [MatchedField::PackageName]);
        let hits = index.search(&query("1.2 lighting"), &nothing_installed);
        assert_eq!(
            hits[0].matched,
            [MatchedField::Category, MatchedField::Version]
        );
        let hits = index.search(&query("1-2 lighting"), &nothing_installed);
        assert_eq!(
            hits[0].matched,
            [MatchedField::PackageName, MatchedField::Category],
            "both weigh 4; a tie keeps the spec's order"
        );
    }

    #[test]
    fn matched_lists_every_field_best_first() {
        let index = index();
        let hits = index.search(&query("white"), &nothing_installed);
        assert_eq!(names(&index, &hits), ["ermine cape - white"]);
        assert_eq!(
            hits[0].matched,
            [
                MatchedField::Name,
                MatchedField::Description,
                MatchedField::File
            ]
        );
        assert_eq!(
            hits[0].file_matches.len(),
            3,
            "the option folder holds every file, so every path matches"
        );
    }

    #[test]
    fn file_matches_prefer_the_most_terms_and_the_option_own_files() {
        let index = index();
        let hits = index.search(&query("white screenshot"), &nothing_installed);
        assert_eq!(
            hits[0].file_matches[0],
            "Vernworth Cloaks Collection/Cloaks/Ermine Cape/White/screenshot.png",
            "the only path matching both terms leads, though it is a skipped file"
        );

        let packages = vec![package(
            "Big",
            "Big.zip",
            &[Opt {
                payload: &[
                    "Big/natives/a/one.mesh",
                    "Big/natives/b/two.mesh",
                    "Big/natives/c/three.mesh",
                    "Big/natives/d/four.mesh",
                    "Big/natives/e/five.mesh",
                    "Big/natives/f/six.mesh",
                    "Big/natives/g/seven.mesh",
                ],
                skipped: &["Big/mesh-preview.png"],
                ..opt("Big Mesh Pack", "Big")
            }],
        )];
        let index = SearchIndex::build(&packages, &HashMap::new());
        let hits = index.search(&query("mesh"), &nothing_installed);
        assert_eq!(hits[0].file_matches.len(), MAX_FILE_MATCHES);
        assert_eq!(
            hits[0].file_matches[0], "Big/natives/a/one.mesh",
            "payload first, archive order"
        );
        assert!(hits[0]
            .file_matches
            .iter()
            .all(|p| p.starts_with("Big/natives/")));
    }

    #[test]
    fn a_minus_removes_options() {
        let index = index();
        assert_eq!(
            search(&index, "ermine -black"),
            Vec::<String>::new(),
            "both capes mention black"
        );
        assert_eq!(search(&index, "ermine -necklace"), Vec::<String>::new());
        assert_eq!(
            search(&index, "ermine -\"black necklace\""),
            ["ermine cape - black"]
        );
        assert_eq!(
            search(&index, "cape -top_315"),
            ["ermine cape - white"],
            "file paths exclude too"
        );
        assert_eq!(
            search(&index, "-lantern"),
            ["ermine cape - white", "ermine cape - black"],
            "only exclusions: library order, menu entries hidden"
        );
    }

    #[test]
    fn empty_text_lists_everything_in_library_order_up_to_the_limit() {
        let index = index();
        assert_eq!(
            search(&index, "   "),
            [
                "ermine cape - white",
                "ermine cape - black",
                "lantern glow recolor",
                "vernworth banners"
            ]
        );
        let mut q = query("");
        q.limit = 2;
        q.filters.include_menu_only = true;
        let hits = index.search(&q, &nothing_installed);
        assert_eq!(
            names(&index, &hits),
            ["cloaks", "ermine cape (menu option)"]
        );
        assert!(hits
            .iter()
            .all(|h| h.score == 0.0 && h.matched.is_empty() && h.file_matches.is_empty()));
    }

    #[test]
    fn the_limit_keeps_the_best() {
        let index = index();
        let mut q = query("e");
        q.limit = 1;
        let hits = index.search(&q, &nothing_installed);
        assert_eq!(hits.len(), 1);
        let all = index.search(&query("e"), &nothing_installed);
        assert_eq!(hits[0], all[0]);
    }

    /// v0.3: a search shows everything it finds — v0.2 stopped at 200.
    #[test]
    fn a_zero_limit_returns_every_match_and_a_positive_one_still_caps() {
        let library = synthetic_library(3, 1_000, 1);
        let index = SearchIndex::build(&library, &HashMap::new());
        assert_eq!(query("").limit, 0, "the default asks for every match");

        let every = index.search(&query(""), &nothing_installed);
        assert_eq!(every.len(), 3_000, "no text: every option, uncapped");
        let robes = index.search(&query("robe"), &nothing_installed);
        assert_eq!(robes.len(), 300, "every robe, ranked, uncapped");

        let mut q = query("robe");
        q.limit = 250;
        assert_eq!(
            index.search(&q, &nothing_installed),
            robes[..250],
            "a positive limit keeps the best, in the same order"
        );
        let mut q = query("");
        q.limit = 250;
        assert_eq!(
            index.search(&q, &nothing_installed),
            every[..250],
            "and with no text, the first in library order"
        );
    }

    #[test]
    fn an_empty_index_answers_nothing() {
        let index = SearchIndex::default();
        assert!(index
            .search(&query("anything"), &nothing_installed)
            .is_empty());
        assert!(index.search(&query(""), &nothing_installed).is_empty());
        assert_eq!(
            format!("{index:?}"),
            "SearchIndex { options: 0, paths: 0, distinct_texts: 0, distinct_segments: 0, packages: 0, distinct_labels: 0 }"
        );
    }

    #[test]
    fn hits_serialise_for_the_window() {
        let index = index();
        let hit = &index.search(&query("top_314"), &nothing_installed)[0];
        let json = serde_json::to_value(hit).unwrap();
        assert_eq!(json["matched"][0], "file");
        assert!(json["optionId"]
            .as_str()
            .unwrap()
            .ends_with("#Vernworth Cloaks Collection/Cloaks/Ermine Cape/White"));
        assert!(json["fileMatches"].is_array());
        // A v0.1 query: no label filters, and a rootIds filter v0.3 no longer knows.
        let q: SearchQuery =
            serde_json::from_str(r#"{"text":"lantern","filters":{"installed":null,"rootIds":["mods"],"includeMenuOnly":true},"limit":0}"#).unwrap();
        assert!(q.filters.include_menu_only);
        assert!(
            q.filters.categories.is_empty() && q.filters.tags.is_empty(),
            "a v0.1 query (no label filters) still reads"
        );
        assert_eq!(
            names(&index, &index.search(&q, &nothing_installed)),
            ["lantern glow recolor", "vernworth banners"],
            "the unknown filter narrows nothing"
        );
        assert!(serde_json::to_value(&q).unwrap()["filters"]
            .get("rootIds")
            .is_none());
        assert!(json["labels"].is_null(), "no labels were set");
    }

    // -- Nexus labels (v0.2) ----------------------------------------------------

    const CLOAKS_ZIP: &str = "Vernworth Cloaks Collection.zip";
    const LANTERN_ZIP: &str = "Lantern Glow-123-1-2-1711234567.zip";
    const BANNERS_ZIP: &str = "Vernworth Banners.zip";
    const CAPES: [&str; 2] = ["ermine cape - white", "ermine cape - black"];

    /// The id the fixture's `package` gives an archive.
    fn id_of(file_name: &str) -> PackageId {
        PackageId::for_archive(file_name, 1)
    }

    fn labels(category: Option<&str>, tags: &[&str]) -> PackageLabels {
        PackageLabels {
            category: category.map(str::to_string),
            tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
            adult: false,
        }
    }

    fn count(name: &str, options: usize) -> LabelCount {
        LabelCount {
            name: name.to_string(),
            options,
        }
    }

    /// The collection is Armour (Lore Friendly, Recolour), the lantern Visuals
    /// (spelling it recolour, and Utilities for Players); the banners are not
    /// linked to Nexus.
    fn nexus_labels() -> HashMap<PackageId, PackageLabels> {
        HashMap::from([
            (
                id_of(CLOAKS_ZIP),
                labels(Some("Armour"), &["Lore Friendly", "Recolour"]),
            ),
            (
                id_of(LANTERN_ZIP),
                labels(Some("Visuals"), &["recolour", "Utilities for Players"]),
            ),
        ])
    }

    fn labeled_index() -> SearchIndex {
        let mut index = index();
        index.set_labels(nexus_labels());
        index
    }

    fn label_query(text: &str, categories: &[&str], tags: &[&str]) -> SearchQuery {
        let mut q = query(text);
        q.filters.categories = categories.iter().map(|c| (*c).to_string()).collect();
        q.filters.tags = tags.iter().map(|t| (*t).to_string()).collect();
        q
    }

    fn filtered(
        index: &SearchIndex,
        text: &str,
        categories: &[&str],
        tags: &[&str],
    ) -> Vec<String> {
        names(
            index,
            &index.search(&label_query(text, categories, tags), &nothing_installed),
        )
    }

    #[test]
    fn categories_are_any_of_and_tags_are_all_of() {
        let index = labeled_index();
        let capes_and_lantern = [CAPES[0], CAPES[1], "lantern glow recolor"];
        assert_eq!(filtered(&index, "", &["Armour"], &[]), CAPES);
        assert_eq!(
            filtered(&index, "", &["Armour", "Visuals"], &[]),
            capes_and_lantern,
            "a second category widens: a mod has only one"
        );
        assert_eq!(
            filtered(&index, "", &["Armour", "Nowhere"], &[]),
            CAPES,
            "a category nobody has widens by nothing"
        );
        assert!(filtered(&index, "", &["Nowhere"], &[]).is_empty());

        assert_eq!(filtered(&index, "", &[], &["Recolour"]), capes_and_lantern);
        assert_eq!(
            filtered(&index, "", &[], &["Recolour", "Lore Friendly"]),
            CAPES,
            "a second tag narrows"
        );
        assert_eq!(
            filtered(&index, "", &[], &["Recolour", "Utilities for Players"]),
            ["lantern glow recolor"]
        );
        assert!(
            filtered(&index, "", &[], &["Lore Friendly", "Utilities for Players"]).is_empty(),
            "no mod carries both"
        );
        assert!(
            filtered(&index, "", &[], &["Recolour", "Nowhere"]).is_empty(),
            "a tag nobody carries leaves nothing"
        );

        // Both at once: any of the categories AND all of the tags.
        assert_eq!(
            filtered(
                &index,
                "",
                &["Armour", "Visuals"],
                &["Utilities for Players"]
            ),
            ["lantern glow recolor"]
        );
        assert!(filtered(&index, "", &["Armour"], &["Utilities for Players"]).is_empty());

        // A tag is no category, and a category no tag.
        assert!(filtered(&index, "", &["Recolour"], &[]).is_empty());
        assert!(filtered(&index, "", &[], &["Armour"]).is_empty());

        // The filters AND with the text, which still ranks.
        assert_eq!(
            filtered(&index, "black", &[], &["Recolour"]),
            ["ermine cape - black", "ermine cape - white"]
        );
        assert!(filtered(&index, "black", &[], &["Utilities for Players"]).is_empty());

        // Menu entries inherit their package's labels too.
        let mut q = label_query("", &["Armour"], &[]);
        q.filters.include_menu_only = true;
        assert_eq!(
            names(&index, &index.search(&q, &nothing_installed)),
            [
                "cloaks",
                "ermine cape (menu option)",
                "ermine cape - white",
                "ermine cape - black"
            ]
        );
        q.limit = 1;
        assert_eq!(
            names(&index, &index.search(&q, &nothing_installed)),
            ["cloaks"],
            "library order, and the limit applies"
        );
    }

    #[test]
    fn label_filters_fold_case_and_whitespace_like_every_field() {
        let mut index = labeled_index();
        assert_eq!(filtered(&index, "", &["ARMOUR"], &[]), CAPES);
        assert_eq!(
            filtered(&index, "", &["armour"], &["  lore   FRIENDLY "]),
            CAPES
        );
        assert_eq!(
            filtered(&index, "", &[], &["RECOLOUR"]).len(),
            3,
            "the collection spells it Recolour, the lantern recolour"
        );
        assert_eq!(
            filtered(&index, "", &["", "  "], &["\t"]),
            search(&index, ""),
            "blank entries are no filter"
        );

        // Unicode, not only ASCII.
        index.set_labels(HashMap::from([(
            id_of(BANNERS_ZIP),
            labels(Some("Épées"), &["Überwurf Und Mantel"]),
        )]));
        assert_eq!(filtered(&index, "", &["ÉPÉES"], &[]), ["vernworth banners"]);
        assert_eq!(
            filtered(&index, "", &[], &["überwurf und mantel"]),
            ["vernworth banners"]
        );
        assert_eq!(search(&index, "ÜBERWURF"), ["vernworth banners"]);
    }

    #[test]
    fn free_text_finds_a_mod_by_its_nexus_labels() {
        let index = labeled_index();
        let hits = index.search(&query("utilities"), &nothing_installed);
        assert_eq!(names(&index, &hits), ["lantern glow recolor"]);
        assert_eq!(hits[0].matched, [MatchedField::NexusLabel]);
        assert_eq!(hits[0].score, 8.0, "a whole word in a label: 4 × 2");
        assert_eq!(
            index.search(&query("util"), &nothing_installed)[0].score,
            6.0,
            "a word start: 4 × 1.5"
        );
        assert_eq!(
            search(&index, "\"utilities for players\""),
            ["lantern glow recolor"]
        );

        // The category is a label too; equal scores fall back to the name.
        assert_eq!(
            search(&index, "armour"),
            ["ermine cape - black", "ermine cape - white"]
        );
        assert_eq!(search(&index, "armour white"), ["ermine cape - white"]);

        // A minus removes by label, like by any field.
        assert!(search(&index, "ermine -armour").is_empty());
        assert_eq!(search(&index, "-recolour"), ["vernworth banners"]);

        assert!(
            search(&self::index(), "utilities").is_empty(),
            "without labels the word is nowhere"
        );
    }

    #[test]
    fn a_label_weighs_between_the_bundle_and_the_author() {
        let mut index = index();
        index.set_labels(HashMap::from([
            (
                id_of(CLOAKS_ZIP),
                labels(Some("Armour"), &["Tailors Guild", "Capes"]),
            ),
            (id_of(LANTERN_ZIP), labels(Some("Lighting"), &[])),
        ]));
        let hits = index.search(&query("tailor"), &nothing_installed);
        assert_eq!(names(&index, &hits).len(), 2);
        assert_eq!(
            hits[0].matched,
            [MatchedField::NexusLabel, MatchedField::Author],
            "a word start in both: 4 × 1.5 beats 3 × 1.5"
        );
        let hits = index.search(&query("capes"), &nothing_installed);
        assert_eq!(
            hits[0].matched,
            [MatchedField::Bundle, MatchedField::NexusLabel],
            "a whole word in both: 5 × 2 beats 4 × 2"
        );
        let hits = index.search(&query("lighting"), &nothing_installed);
        assert_eq!(
            hits[0].matched,
            [MatchedField::Category, MatchedField::NexusLabel],
            "modinfo's category and Nexus's tie at 4; a tie keeps the documented order"
        );
        assert_eq!(hits[0].score, 8.0, "the best field counts once");
    }

    #[test]
    fn a_package_without_labels_hides_under_a_label_filter_and_shows_without_one() {
        let index = labeled_index();
        assert_eq!(
            search(&index, "lantern"),
            ["lantern glow recolor", "vernworth banners"]
        );
        assert_eq!(
            filtered(&index, "lantern", &[], &["Recolour"]),
            ["lantern glow recolor"]
        );
        assert_eq!(
            filtered(&index, "lantern", &["Visuals"], &[]),
            ["lantern glow recolor"]
        );
        assert!(filtered(&index, "banners", &["Armour", "Visuals"], &[]).is_empty());
        assert_eq!(search(&index, "banners"), ["vernworth banners"]);

        let hits = index.search(&query("lantern"), &nothing_installed);
        assert_eq!(
            hits[0].labels,
            Some(labels(
                Some("Visuals"),
                &["recolour", "Utilities for Players"]
            )),
            "each hit carries its package's labels in Nexus's spelling"
        );
        assert_eq!(hits[1].labels, None, "the banners are not linked");
    }

    #[test]
    fn new_labels_replace_the_old_without_a_rebuild() {
        let mut index = labeled_index();
        let built = (
            index.option_count(),
            index.path_count(),
            index.texts.len(),
            index.segments.len(),
        );
        assert_eq!(index.labels.texts.len(), 5, "Recolour and recolour are one");

        // The next check: the collection lost a tag, the banners were
        // linked, and the lantern was unlinked.
        index.set_labels(HashMap::from([
            (
                id_of(CLOAKS_ZIP),
                labels(Some("Armour"), &["Lore Friendly"]),
            ),
            (id_of(BANNERS_ZIP), labels(Some("Visuals"), &["Recolour"])),
        ]));
        assert_eq!(
            filtered(&index, "", &[], &["Recolour"]),
            ["vernworth banners"]
        );
        assert_eq!(
            filtered(&index, "", &["visuals"], &[]),
            ["vernworth banners"]
        );
        assert!(
            search(&index, "utilities").is_empty(),
            "the lantern's old tag is gone"
        );
        assert_eq!(index.labels(&id_of(LANTERN_ZIP)), None);
        assert_eq!(
            index.labels(&id_of(BANNERS_ZIP)),
            Some(&labels(Some("Visuals"), &["Recolour"]))
        );
        assert_eq!(
            (
                index.option_count(),
                index.path_count(),
                index.texts.len(),
                index.segments.len()
            ),
            built,
            "nothing but the labels changed"
        );
        assert_eq!(
            index.labels.texts.len(),
            4,
            "the old labels left nothing behind"
        );

        // Again and again: the layer never grows.
        for _ in 0..3 {
            index.set_labels(nexus_labels());
        }
        assert_eq!(index.labels.texts.len(), 5);
        assert_eq!(filtered(&index, "", &["Armour"], &[]), CAPES);

        // Nothing given: no labels anywhere.
        index.set_labels(HashMap::new());
        assert!(filtered(&index, "", &["Armour"], &[]).is_empty());
        assert!(index
            .search(&query(""), &nothing_installed)
            .iter()
            .all(|hit| hit.labels.is_none()));
        assert_eq!(
            index.label_facets(),
            LabelFacets {
                unlabeled_packages: 3,
                ..LabelFacets::default()
            }
        );

        // Labels for a package the index does not hold are ignored.
        index.set_labels(HashMap::from([(
            PackageId("ffffffffffffffff".to_string()),
            labels(Some("Armour"), &["Recolour"]),
        )]));
        assert_eq!(index.label_facets().unlabeled_packages, 3);
        assert!(index.label_facets().categories.is_empty());
    }

    #[test]
    fn facets_count_installable_options_per_category_and_tag() {
        let mut library = library();
        library.push(package(
            "Vernworth Menus",
            "Vernworth Menus.zip",
            &[Opt {
                menu_only: true,
                ..opt("Vernworth Menus", "Vernworth Menus")
            }],
        ));
        let mut index = SearchIndex::build(&library, &HashMap::new());
        let mut given = nexus_labels();
        given.insert(
            id_of("Vernworth Menus.zip"),
            labels(Some("Gameplay"), &["Menus Only"]),
        );
        index.set_labels(given);

        let facets = index.label_facets();
        assert_eq!(
            facets,
            LabelFacets {
                categories: vec![count("Armour", 2), count("Visuals", 1)],
                tags: vec![
                    count("Recolour", 3),
                    count("Lore Friendly", 2),
                    count("Utilities for Players", 1)
                ],
                unlabeled_packages: 1,
            },
            "the collection's two menu entries count for nothing, the menus-only package \
             neither shows a chip nor counts as hidden, and the banners carry no labels"
        );
        for facet in &facets.categories {
            assert_eq!(
                filtered(&index, "", &[facet.name.as_str()], &[]).len(),
                facet.options,
                "the {} chip predicts its results",
                facet.name
            );
        }
        for facet in &facets.tags {
            assert_eq!(
                filtered(&index, "", &[], &[facet.name.as_str()]).len(),
                facet.options,
                "the {} chip predicts its results",
                facet.name
            );
        }

        // Equal counts go by name; a facet shows the first spelling met in
        // library order (the collection, the lantern, the banners).
        index.set_labels(HashMap::from([
            (
                id_of(CLOAKS_ZIP),
                labels(Some("Visuals"), &["Zebra Stripes"]),
            ),
            (
                id_of(LANTERN_ZIP),
                labels(Some("armour"), &["Arisen Approved"]),
            ),
            (
                id_of(BANNERS_ZIP),
                labels(Some("ARMOUR"), &["arisen approved"]),
            ),
        ]));
        let facets = index.label_facets();
        assert_eq!(facets.categories, [count("armour", 2), count("Visuals", 2)]);
        assert_eq!(
            facets.tags,
            [count("Arisen Approved", 2), count("Zebra Stripes", 2)]
        );
        assert_eq!(facets.unlabeled_packages, 0);
    }

    #[test]
    fn labels_are_tidied_and_an_adult_flag_alone_still_reaches_the_hits() {
        let mut index = index();
        index.set_labels(HashMap::from([
            (
                id_of(CLOAKS_ZIP),
                PackageLabels {
                    category: Some("  Armour ".to_string()),
                    tags: ["Lore Friendly", " ", "LORE FRIENDLY", "Recolour"]
                        .map(str::to_string)
                        .to_vec(),
                    adult: false,
                },
            ),
            (
                id_of(LANTERN_ZIP),
                PackageLabels {
                    adult: true,
                    ..PackageLabels::default()
                },
            ),
            (
                id_of(BANNERS_ZIP),
                PackageLabels {
                    category: Some("   ".to_string()),
                    ..PackageLabels::default()
                },
            ),
        ]));

        let tidy = labels(Some("Armour"), &["Lore Friendly", "Recolour"]);
        let hits = index.search(&query("ermine"), &nothing_installed);
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|hit| hit.labels.as_ref() == Some(&tidy)));
        assert_eq!(index.labels(&id_of(CLOAKS_ZIP)), Some(&tidy));

        let adult_only = PackageLabels {
            adult: true,
            ..PackageLabels::default()
        };
        assert_eq!(index.labels(&id_of(LANTERN_ZIP)), Some(&adult_only));
        assert_eq!(
            index.search(&query("lantern glow"), &nothing_installed)[0].labels,
            Some(adult_only),
            "the veil learns the flag from the hit"
        );
        assert_eq!(
            index.labels(&id_of(BANNERS_ZIP)),
            None,
            "a blank entry is no entry"
        );

        let facets = index.label_facets();
        assert_eq!(facets.categories, [count("Armour", 2)]);
        assert_eq!(
            facets.tags,
            [count("Lore Friendly", 2), count("Recolour", 2)],
            "a tag given twice counts once"
        );
        assert_eq!(
            facets.unlabeled_packages, 2,
            "an adult flag alone gives no chip to find the lantern by"
        );
        assert!(filtered(&index, "lantern", &["Armour"], &[]).is_empty());
    }

    #[test]
    fn labels_serialise_for_the_spine() {
        let index = labeled_index();
        let hit = &index.search(&query("utilities"), &nothing_installed)[0];
        let json = serde_json::to_value(hit).unwrap();
        assert_eq!(json["matched"], serde_json::json!(["nexusLabel"]));
        assert_eq!(json["labels"]["category"], "Visuals");
        assert_eq!(
            json["labels"]["tags"],
            serde_json::json!(["recolour", "Utilities for Players"])
        );
        assert_eq!(json["labels"]["adult"], false);

        let facets = serde_json::to_value(index.label_facets()).unwrap();
        assert_eq!(facets["unlabeledPackages"], 1);
        assert_eq!(
            facets["categories"][0],
            serde_json::json!({ "name": "Armour", "options": 2 })
        );

        let q: SearchQuery = serde_json::from_str(
            r#"{"text":"","filters":{"installed":null,"rootIds":[],"includeMenuOnly":false,"categories":["Armour"],"tags":["Recolour","Lore Friendly"]},"limit":0}"#,
        )
        .unwrap();
        assert_eq!(
            q,
            label_query("", &["Armour"], &["Recolour", "Lore Friendly"])
        );
        assert_eq!(names(&index, &index.search(&q, &nothing_installed)), CAPES);

        let given: PackageLabels = serde_json::from_str(r#"{"category":"Armour"}"#).unwrap();
        assert_eq!(given, labels(Some("Armour"), &[]), "absent fields default");
    }

    // -- speed ----------------------------------------------------------------

    const GARMENTS: [&str; 10] = [
        "Robe",
        "Cape",
        "Cloak",
        "Tunic",
        "Gauntlets",
        "Greaves",
        "Helm",
        "Necklace",
        "Bracers",
        "Boots",
    ];
    const COLORS: [&str; 10] = [
        "Black", "White", "Crimson", "Azure", "Emerald", "Gold", "Silver", "Ashen", "Violet",
        "Amber",
    ];
    const CATEGORIES: [&str; 5] = ["Tops", "Bottoms", "Headwear", "Accessories", "Footwear"];

    /// A library shaped like many collections: packages of variants under
    /// menu options, each variant with `paths_per_option` files — half shared
    /// with its sibling variants (the same game paths), half its own.
    fn synthetic_library(
        packages: usize,
        options_per_package: usize,
        paths_per_option: usize,
    ) -> Vec<Package> {
        let mut library = Vec::with_capacity(packages);
        let mut serial = 0usize;
        for p in 0..packages {
            let display_name = format!("Vernworth Collection {p:03}");
            let id = PackageId::for_archive(&format!("{display_name}.zip"), p as u64);
            let mut options = Vec::with_capacity(options_per_package);
            for o in 0..options_per_package {
                serial += 1;
                let garment = GARMENTS[o % GARMENTS.len()];
                let color = COLORS[(o / GARMENTS.len()) % COLORS.len()];
                let category = CATEGORIES[o % CATEGORIES.len()];
                let menu = format!("{garment} (Menu Option)");
                let name = format!("{garment} - {color} {serial}");
                let root = format!("{display_name}/{category}/{menu}/{color} {serial}");
                let g = garment.to_lowercase();
                let payload: Vec<PayloadFile> = (0..paths_per_option)
                    .map(|j| {
                        let game_path = if j % 2 == 0 {
                            format!("natives/stm/appsystem/{g}wb/mesh/{g}_{p}_{j}_wb_f.mesh.260421070")
                        } else {
                            format!("natives/stm/appsystem/{g}wb/tex/{g}_{serial}_{j}_alba.tex.251211553")
                        };
                        PayloadFile {
                            entry: format!("{root}/{game_path}"),
                            target: DeployTarget::Loose { game_path },
                            size: 1,
                        }
                    })
                    .collect();
                options.push(ModOption {
                    id: OptionId::new(&id, &root),
                    package_id: id.clone(),
                    root,
                    name: name.clone(),
                    info: Some(ModInfo {
                        name: Some(name),
                        description: Some(format!(
                            "A {} {} from the Vernworth tailors, fitted for the Arisen.",
                            color.to_lowercase(),
                            g
                        )),
                        author: Some(format!("Tailor {}", o % 20)),
                        category: Some(category.to_string()),
                        version: Some(format!("1.{}", o % 10)),
                        ..ModInfo::default()
                    }),
                    screenshot: None,
                    kinds: PayloadKinds::tally(&payload),
                    payload,
                    skipped: vec![SkippedFile {
                        entry: format!(
                            "{display_name}/{category}/{menu}/{color} {serial}/screenshot.png"
                        ),
                        reason: SkipReason::Picture,
                    }],
                    parent: None,
                    addon_for: Some(menu.clone()),
                    bundle: None,
                    menu_only: false,
                    trail: vec![display_name.clone(), category.to_string(), menu],
                });
            }
            library.push(Package {
                id,
                path: PathBuf::from(format!("/mods/{display_name}.zip")),
                file_name: format!("{display_name}.zip"),
                display_name,
                format: PackageFormat::Zip,
                size: 1,
                modified_ms: 0,
                entry_count: 0,
                options,
                nexus: None,
                problems: Vec::new(),
            });
        }
        library
    }

    const SPEED_QUERIES: [&str; 9] = [
        "robe",
        "black robe",
        "\"robe - black\"",
        "robe_7_4",
        "mesh",
        "tex/robe_10",
        "tailor 3 azure",
        "robe -black -white",
        "zzzz",
    ];

    const NEXUS_CATEGORIES: [&str; 4] = ["Armour", "Visuals", "Utilities", "Gameplay"];
    const NEXUS_TAGS: [&str; 6] = [
        "Lore Friendly",
        "Recolour",
        "Utilities for Players",
        "Immersion",
        "Retexture",
        "Arisen Approved",
    ];

    /// Text, categories, tags.
    const LABEL_QUERIES: [(&str, &[&str], &[&str]); 5] = [
        ("", &["Armour"], &[]),
        ("", &["Armour", "Visuals"], &["Recolour"]),
        ("utilities", &[], &[]),
        ("robe", &[], &["Recolour", "Utilities for Players"]),
        ("black robe -utilities", &["Gameplay"], &[]),
    ];

    /// Nine packages in ten linked: a category each, up to three tags, and
    /// an adult flag now and then.
    fn synthetic_labels(library: &[Package]) -> HashMap<PackageId, PackageLabels> {
        library
            .iter()
            .enumerate()
            .filter(|(p, _)| p % 10 != 9)
            .map(|(p, package)| {
                let labels = PackageLabels {
                    category: Some(NEXUS_CATEGORIES[p % NEXUS_CATEGORIES.len()].to_string()),
                    tags: (0..p % 4)
                        .map(|k| NEXUS_TAGS[(p + k) % NEXUS_TAGS.len()].to_string())
                        .collect(),
                    adult: p % 7 == 0,
                };
                (package.id.clone(), labels)
            })
            .collect()
    }

    fn millis(duration: Duration) -> f64 {
        duration.as_secs_f64() * 1000.0
    }

    /// Time `set_labels` (best of five, each given its own copy of the
    /// labels), `label_facets` (best of five) and the label queries (best of
    /// three); print them all and return the slowest.
    fn time_labels(index: &mut SearchIndex, library: &[Package]) -> Duration {
        let labels = synthetic_labels(library);
        let mut set = Duration::MAX;
        for _ in 0..5 {
            let copy = labels.clone();
            let start = Instant::now();
            index.set_labels(copy);
            set = set.min(start.elapsed());
        }
        let mut facets = LabelFacets::default();
        let mut counted = Duration::MAX;
        for _ in 0..5 {
            let start = Instant::now();
            facets = index.label_facets();
            counted = counted.min(start.elapsed());
        }
        println!(
            "  set_labels ({} of {} packages linked)    {:>8.3} ms",
            labels.len(),
            library.len(),
            millis(set)
        );
        println!(
            "  label_facets ({} categories, {} tags)      {:>8.3} ms",
            facets.categories.len(),
            facets.tags.len(),
            millis(counted)
        );
        let mut slowest = set.max(counted);
        for (text, categories, tags) in LABEL_QUERIES {
            let q = label_query(text, categories, tags);
            let (mut best, mut hits) = (Duration::MAX, 0);
            for _ in 0..3 {
                let start = Instant::now();
                hits = index.search(&q, &nothing_installed).len();
                best = best.min(start.elapsed());
            }
            let shown = format!("{text:?} {categories:?} {tags:?}");
            println!("  {shown:<42} {hits:>4} hits  {:>8.3} ms", millis(best));
            slowest = slowest.max(best);
        }
        slowest
    }

    /// Every chip's count is what a filter on it alone finds.
    fn assert_chips_predict_their_results(index: &SearchIndex) {
        let facets = index.label_facets();
        let found = |categories: &[&str], tags: &[&str]| {
            let q = label_query("", categories, tags);
            assert_eq!(q.limit, 0, "every match");
            index.search(&q, &nothing_installed).len()
        };
        for facet in &facets.categories {
            assert_eq!(
                found(&[facet.name.as_str()], &[]),
                facet.options,
                "{}",
                facet.name
            );
        }
        for facet in &facets.tags {
            assert_eq!(
                found(&[], &[facet.name.as_str()]),
                facet.options,
                "{}",
                facet.name
            );
        }
    }

    /// Time every speed query (best of three runs); returns the slowest.
    fn time_queries(index: &SearchIndex) -> Duration {
        let mut slowest = Duration::ZERO;
        for text in SPEED_QUERIES {
            let mut best = Duration::MAX;
            let mut hits = 0;
            for _ in 0..3 {
                let start = Instant::now();
                hits = index.search(&query(text), &nothing_installed).len();
                best = best.min(start.elapsed());
            }
            println!(
                "  {text:<22} {hits:>4} hits  {:>8.3} ms",
                best.as_secs_f64() * 1000.0
            );
            slowest = slowest.max(best);
        }
        slowest
    }

    #[test]
    fn the_investors_scale_answers_well_under_five_milliseconds_in_release() {
        let library = synthetic_library(3, 100, 10); // 300 options, 3,000 paths
        let index = SearchIndex::build(&library, &HashMap::new());
        assert_eq!((index.option_count(), index.path_count()), (300, 3_300));
        println!("300 options / 3,000 payload paths:");
        let slowest = time_queries(&index);
        assert!(
            slowest < Duration::from_millis(50),
            "slowest query took {slowest:?}"
        );
    }

    #[test]
    fn ten_thousand_options_and_two_hundred_thousand_paths_stay_fast() {
        let start = Instant::now();
        let library = synthetic_library(50, 200, 20); // 10,000 options, 200,000 paths
        let generated = start.elapsed();
        let start = Instant::now();
        let index = SearchIndex::build(&library, &HashMap::new());
        let built = start.elapsed();
        assert_eq!(
            (index.option_count(), index.path_count()),
            (10_000, 210_000)
        );
        println!(
            "10,000 options / 200,000 payload paths: generated in {:.0} ms, indexed in {:.0} ms ({index:?})",
            generated.as_secs_f64() * 1000.0,
            built.as_secs_f64() * 1000.0
        );

        // The results are right, not just fast.
        let hits = index.search(&query("black robe"), &nothing_installed);
        assert_eq!(
            hits.len(),
            100,
            "every black robe: two per package, uncapped"
        );
        let hits = index.search(&query("robe_7_4"), &nothing_installed);
        assert!(hits.iter().all(|h| h.matched.contains(&MatchedField::File)));
        assert!(index.search(&query("zzzz"), &nothing_installed).is_empty());

        let slowest = time_queries(&index);
        assert!(
            slowest < Duration::from_millis(250),
            "slowest query took {slowest:?}"
        );

        let mut index = index;
        println!("the same, with Nexus labels:");
        let slowest = time_labels(&mut index, &library);
        assert!(
            slowest < Duration::from_millis(250),
            "the slowest label step took {slowest:?}"
        );
        assert_chips_predict_their_results(&index);
    }

    #[test]
    fn labels_on_ten_thousand_options_in_two_thousand_packages_stay_cheap() {
        // set_labels works per package, so many small packages are its worst case.
        let library = synthetic_library(2_000, 5, 2); // 10,000 options, 20,000 payload paths
        let mut index = SearchIndex::build(&library, &HashMap::new());
        assert_eq!(index.option_count(), 10_000);
        println!("10,000 options in 2,000 packages, with Nexus labels:");
        let slowest = time_labels(&mut index, &library);
        assert!(
            slowest < Duration::from_millis(250),
            "the slowest label step took {slowest:?}"
        );
        assert_chips_predict_their_results(&index);
        let facets = index.label_facets();
        assert_eq!(
            facets.unlabeled_packages, 200,
            "every tenth package is unlinked"
        );
        assert_eq!(
            facets.categories.iter().map(|c| c.options).sum::<usize>(),
            9_000,
            "every option of a linked package sits under exactly one category"
        );
    }
}
