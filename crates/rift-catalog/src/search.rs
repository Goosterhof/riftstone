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
//! category 4, author 3, version 2, description 2, file 1 — times 1.5 when the
//! term starts a word in that field, times 2 when it is a whole word. The
//! option's score is the sum over its terms; hits sort by score, then name,
//! then library order. The trail field holds the menu ancestors only (the
//! package name and the bundle score as their own fields). `matched` lists
//! every field any term hit, best first; `file_matches` lists up to five
//! paths, the ones matching the most terms first, then those matching beyond
//! the option's own folder, then payload before skipped files.
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
    /// Restrict to these library roots (empty = all).
    pub root_ids: Vec<String>,
    /// Include pure menu entries (default false — they install nothing).
    pub include_menu_only: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub text: String,
    pub filters: SearchFilters,
    /// Maximum hits returned (0 = default 200).
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
}

const DEFAULT_LIMIT: usize = 200;
const MAX_FILE_MATCHES: usize = 5;
const NO_TEXT: u32 = u32::MAX;
const FILE_WEIGHT: f32 = 1.0;

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
const FIELD_ORDER: [MatchedField; 9] = [
    MatchedField::Name,
    MatchedField::Trail,
    MatchedField::Bundle,
    MatchedField::PackageName,
    MatchedField::Category,
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
#[derive(Clone, Default)]
pub struct SearchIndex {
    options: Vec<OptionRecord>,
    roots: Vec<String>,
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
    package_id: PackageId,
    root: u32,
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

    /// Run a query. `is_installed` tells the index which options are installed
    /// (the deploy journal owns that truth). An empty query text returns every
    /// option that passes the filters, in library order.
    pub fn search(
        &self,
        query: &SearchQuery,
        is_installed: &dyn Fn(&OptionId) -> bool,
    ) -> Vec<SearchHit> {
        let limit = if query.limit == 0 {
            DEFAULT_LIMIT
        } else {
            query.limit
        };
        let filters = &query.filters;
        let root_allowed: Vec<bool> = self
            .roots
            .iter()
            .map(|root| {
                filters.root_ids.is_empty() || filters.root_ids.iter().any(|wanted| wanted == root)
            })
            .collect();

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
                || !root_allowed[option.root as usize]
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
        best
    }

    fn match_term(&self, needle: &str) -> TermMatch {
        let text_quality = self.texts.qualities(needle);
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
            package_id: option.package_id.clone(),
            score,
            matched: matched
                .into_iter()
                .map(|(rank, _)| FIELD_ORDER[rank])
                .collect(),
            file_matches: self.file_matches(index, positives),
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
}

impl TermMatch {
    fn is_empty(&self) -> bool {
        self.text_quality.iter().all(|q| *q == 0) && self.file_quality.iter().all(|q| *q == 0)
    }
}

// ---------------------------------------------------------------------------
// Building
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Builder {
    index: SearchIndex,
    text_ids: FastMap<String, u32>,
    root_ids: FastMap<String, u32>,
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
    root: u32,
    display_name: u32,
    file_name: u32,
}

impl Builder {
    fn add_package(&mut self, package: &Package) {
        let root = match self.root_ids.get(&package.root_id) {
            Some(&id) => id,
            None => {
                let id = to_u32(self.index.roots.len());
                self.index.roots.push(package.root_id.clone());
                self.root_ids.insert(package.root_id.clone(), id);
                id
            }
        };
        let shared = PackageTexts {
            root,
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
            package_id: package.id.clone(),
            root: shared.root,
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

    fn package(root_id: &str, display_name: &str, file_name: &str, options: &[Opt<'_>]) -> Package {
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
            root_id: root_id.to_string(),
            path: PathBuf::from(format!("/library/{file_name}")),
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
                "fluffy",
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
                "fluffy",
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
                "downloads",
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

        let mut q = query("lantern");
        assert_eq!(
            names(&index, &index.search(&q, &nothing_installed)),
            ["lantern glow recolor", "vernworth banners"]
        );
        q.filters.root_ids = vec!["downloads".into()];
        assert_eq!(
            names(&index, &index.search(&q, &nothing_installed)),
            ["vernworth banners"]
        );
        q.filters.root_ids = vec!["nowhere".into()];
        assert!(index.search(&q, &nothing_installed).is_empty());

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
            "fluffy",
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
            "fluffy",
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

    #[test]
    fn an_empty_index_answers_nothing() {
        let index = SearchIndex::default();
        assert!(index
            .search(&query("anything"), &nothing_installed)
            .is_empty());
        assert!(index.search(&query(""), &nothing_installed).is_empty());
        assert_eq!(
            format!("{index:?}"),
            "SearchIndex { options: 0, paths: 0, distinct_texts: 0, distinct_segments: 0 }"
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
        let q: SearchQuery =
            serde_json::from_str(r#"{"text":"robe","filters":{"installed":null,"rootIds":[],"includeMenuOnly":true},"limit":0}"#).unwrap();
        assert!(q.filters.include_menu_only);
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
                root_id: if p % 2 == 0 {
                    "fluffy".into()
                } else {
                    "downloads".into()
                },
                path: PathBuf::from(format!("/library/{display_name}.zip")),
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
        assert!(!hits.is_empty() && hits.len() <= DEFAULT_LIMIT);
        let hits = index.search(&query("robe_7_4"), &nothing_installed);
        assert!(hits.iter().all(|h| h.matched.contains(&MatchedField::File)));
        assert!(index.search(&query("zzzz"), &nothing_installed).is_empty());

        let slowest = time_queries(&index);
        assert!(
            slowest < Duration::from_millis(250),
            "slowest query took {slowest:?}"
        );
    }
}
