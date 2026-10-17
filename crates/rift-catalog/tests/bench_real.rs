//! Bench suite: the investor's real mods folder, READ-ONLY, COUNTS ONLY.
//!
//! `#[ignore]`d and env-gated, so `cargo test` never touches real data. Run it
//! deliberately, in release:
//!
//! ```sh
//! RIFT_BENCH_LIBRARY='/mnt/c/<install dir>/mods' \
//!   cargo test -p rift-catalog --release --test bench_real -- --ignored --nocapture
//! ```
//!
//! `RIFT_BENCH_LIBRARY` is THE mods folder, scanned the way the stone scans
//! it. It holds the investor's own mods, so this bench prints counts, shapes
//! and timings — never a package, option or file name, never a problem
//! message (those quote names), never a byte of content — and writes nothing
//! anywhere: the cache the cold scan builds stays in memory for the warm scan.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use rift_catalog::tree::{self, Nest};
use rift_catalog::{Catalog, CatalogCache, ScanStats, SearchFilters, SearchQuery};
use rift_core::PackageFormat;

/// The armour-variants collection in the investor's library: 281 options,
/// each with its own modinfo.ini (docs/BUILD-SPEC.md, "What the Riftstone is").
const LARGEST_COLLECTION_OPTIONS: usize = 281;

const QUERIES: [&str; 3] = ["top_314", "\"menu option\"", "variation 3"];

/// Repeats per query for the steady-state time.
const QUERY_RUNS: usize = 25;

fn mods_folder() -> Option<PathBuf> {
    std::env::var_os("RIFT_BENCH_LIBRARY").map(PathBuf::from)
}

fn ms(duration: Duration) -> String {
    format!("{:.1} ms", duration.as_secs_f64() * 1000.0)
}

fn format_name(format: PackageFormat) -> &'static str {
    match format {
        PackageFormat::Zip => "zip",
        PackageFormat::SevenZip => "7z",
        PackageFormat::Rar => "rar",
        PackageFormat::Folder => "folder",
    }
}

fn print_stats(label: &str, stats: ScanStats) {
    eprintln!(
        "  {label}: {} candidates — {} opened, {} from cache, {} unreadable, {} gone",
        stats.candidates, stats.opened, stats.reused, stats.unreadable, stats.gone
    );
}

/// Nodes, bundles and the deepest level of one package's tree.
fn tree_counts(nodes: &[Nest], depth: usize) -> (usize, usize, usize) {
    let (mut count, mut bundles, mut deepest) = (0, 0, depth);
    for node in nodes {
        let children = match node {
            Nest::Option { children, .. } => children,
            Nest::Bundle { children, .. } => {
                bundles += 1;
                children
            }
        };
        count += 1;
        let (below, below_bundles, below_deepest) = tree_counts(children, depth + 1);
        count += below;
        bundles += below_bundles;
        deepest = deepest.max(below_deepest);
    }
    (count, bundles, deepest)
}

#[test]
#[ignore = "bench: needs RIFT_BENCH_LIBRARY (read-only, prints counts only)"]
fn bench_real_library_scan_and_search() {
    let Some(library) = mods_folder() else {
        eprintln!("RIFT_BENCH_LIBRARY is not set; nothing to scan");
        return;
    };

    // Cold: nothing cached.
    let mut events = 0usize;
    let started = Instant::now();
    let (cold, cache) = Catalog::scan(&library, &CatalogCache::default(), &mut |_| events += 1);
    let cold_time = started.elapsed();

    // Warm: the second launch, from the cache the cold scan built.
    let started = Instant::now();
    let (warm, _) = Catalog::scan(&library, &cache, &mut |_| {});
    let warm_time = started.elapsed();

    // The startup preview, from the same cache, without the disk.
    let started = Instant::now();
    let preview = Catalog::from_cache(&library, &cache);
    let preview_time = started.elapsed();

    let packages = cold.packages();
    let mut options_per_package: Vec<usize> = packages.iter().map(|p| p.options.len()).collect();
    options_per_package.sort_unstable();
    let options: usize = options_per_package.iter().sum();
    let entries: usize = packages.iter().map(|p| p.entry_count).sum();
    let payload: usize = packages
        .iter()
        .flat_map(|p| &p.options)
        .map(|o| o.payload.len())
        .sum();
    let skipped: usize = packages
        .iter()
        .flat_map(|p| &p.options)
        .map(|o| o.skipped.len())
        .sum();
    let menus = packages
        .iter()
        .flat_map(|p| &p.options)
        .filter(|o| o.menu_only)
        .count();
    let parented = packages
        .iter()
        .flat_map(|p| &p.options)
        .filter(|o| o.parent.is_some())
        .count();

    let mut formats: BTreeMap<&str, usize> = BTreeMap::new();
    let mut problems: BTreeMap<String, usize> = BTreeMap::new();
    let (mut tree_top, mut tree_nodes, mut tree_bundles, mut tree_depth) = (0, 0, 0, 0);
    let started = Instant::now();
    for package in packages {
        *formats.entry(format_name(package.format)).or_default() += 1;
        for problem in &package.problems {
            *problems.entry(format!("{:?}", problem.kind)).or_default() += 1;
        }
        let nested = tree::nest(package);
        let (nodes, bundles, depth) = tree_counts(&nested, 1);
        tree_top += nested.len();
        tree_nodes += nodes;
        tree_bundles += bundles;
        tree_depth = tree_depth.max(depth);
    }
    let nest_time = started.elapsed();
    let linked = packages.iter().filter(|p| p.nexus.is_some()).count();

    eprintln!("rift-catalog bench (counts only)");
    print_stats("cold scan", cold.stats());
    print_stats("warm scan", warm.stats());
    eprintln!("  progress events (cold): {events}");
    eprintln!("  packages: {}, formats {formats:?}", packages.len());
    eprintln!("  options: {options}; per package (sorted): {options_per_package:?}");
    eprintln!("  menu-only options: {menus}; options with a resolved parent: {parented}");
    eprintln!("  entries: {entries}; payload files: {payload}; skipped files: {skipped}");
    eprintln!("  packages linked to Nexus (file name or homepage): {linked}");
    eprintln!("  problems by kind: {problems:?}");
    eprintln!(
        "  library tree: {tree_top} top nodes under the packages, {tree_nodes} nodes in all, \
         {tree_bundles} bundles, {tree_depth} levels at most; nested in {}",
        ms(nest_time)
    );
    eprintln!(
        "  cold: {}   warm (from cache): {}   startup preview: {}",
        ms(cold_time),
        ms(warm_time),
        ms(preview_time)
    );

    let largest = options_per_package.last().copied().unwrap_or(0);
    eprintln!(
        "  largest collection: {largest} options — {}",
        if largest == LARGEST_COLLECTION_OPTIONS {
            format!("matches the expected {LARGEST_COLLECTION_OPTIONS}")
        } else {
            format!("DOES NOT match the expected {LARGEST_COLLECTION_OPTIONS}")
        }
    );

    for (label, include_menu_only) in [("installable", false), ("menus included", true)] {
        for text in QUERIES {
            let query = SearchQuery {
                text: text.to_string(),
                filters: SearchFilters {
                    include_menu_only,
                    ..SearchFilters::default()
                },
                limit: 0, // every match, as the window asks
            };
            let started = Instant::now();
            let hits = warm.search(&query, &|_| false).len();
            let first = started.elapsed();
            let started = Instant::now();
            for _ in 0..QUERY_RUNS {
                assert_eq!(warm.search(&query, &|_| false).len(), hits);
            }
            let steady = started.elapsed() / u32::try_from(QUERY_RUNS).unwrap();
            eprintln!(
                "  query {text:<16} ({label}): {hits} hits, first {}, steady {}",
                ms(first),
                ms(steady)
            );
        }
    }

    assert_eq!(
        warm.stats().opened,
        0,
        "the warm scan reopened packages the cache already held"
    );
    assert_eq!(warm.packages().len(), packages.len());
    assert_eq!(
        preview.packages(),
        warm.packages(),
        "the startup preview shows what the scan does"
    );
}

/// Where a scan's time goes, one package at a time (no pool, no cache):
/// listing, open + list, the modinfo.ini reads, classification. Prints the
/// format, size, counts and milliseconds per package — never a name.
#[test]
#[ignore = "bench: needs RIFT_BENCH_LIBRARY (read-only, prints counts only)"]
fn bench_real_package_phases() {
    use rift_catalog::{classify, modinfo, scan};
    use rift_core::path::{file_name, parent};
    use rift_core::PackageId;

    let Some(library) = mods_folder() else {
        eprintln!("RIFT_BENCH_LIBRARY is not set; nothing to time");
        return;
    };
    let started = Instant::now();
    let candidates = scan::list_library(&library);
    eprintln!(
        "rift-catalog package phases (counts only): {} candidates listed in {}",
        candidates.len(),
        ms(started.elapsed())
    );
    let mut rows: Vec<(u64, String)> = Vec::new();
    for path in &candidates {
        let started = Instant::now();
        let Ok(mut source) = rift_archive::open(path) else {
            rows.push((0, "  unreadable package".to_string()));
            continue;
        };
        let entries = source.entries().unwrap_or_default();
        let open_time = started.elapsed();
        let size = if source.format() == PackageFormat::Folder {
            entries.iter().map(|entry| entry.size).sum()
        } else {
            std::fs::metadata(path).map_or(0, |metadata| metadata.len())
        };

        let started = Instant::now();
        let mut modinfos = BTreeMap::new();
        for entry in &entries {
            if file_name(&entry.path).eq_ignore_ascii_case("modinfo.ini") {
                if let Ok(bytes) = source.read_entry(&entry.path, Some(scan::MODINFO_READ_LIMIT)) {
                    modinfos.insert(parent(&entry.path).to_string(), modinfo::parse(&bytes).info);
                }
            }
        }
        let modinfo_time = started.elapsed();

        let started = Instant::now();
        let (options, _) = classify::build_options(
            &PackageId::for_archive("bench", size),
            "bench",
            &entries,
            &modinfos,
        );
        let classify_time = started.elapsed();

        let row = format!(
            "  {:<6} {:>8.1} MB  {:>5} entries  {:>3} modinfo  {:>3} options | open+list {:>9}  modinfo {:>9}  classify {:>8}",
            format_name(source.format()),
            size as f64 / 1_048_576.0,
            entries.len(),
            modinfos.len(),
            options.len(),
            ms(open_time),
            ms(modinfo_time),
            ms(classify_time),
        );
        rows.push((size, row));
    }
    // Sorted by size, so the order says nothing about the names.
    rows.sort_unstable();
    for (_, row) in rows {
        eprintln!("{row}");
    }
}
