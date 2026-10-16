//! Bench suite: the investor's real library, READ-ONLY, COUNTS ONLY.
//!
//! `#[ignore]`d and env-gated, so `cargo test` never touches real data. Run it
//! deliberately, in release:
//!
//! ```sh
//! RIFT_BENCH_LIBRARY='/mnt/c/<fluffy>/Games/DragonsDogma2/Mods' \
//! RIFT_BENCH_DOWNLOADS='/mnt/c/Users/<you>/Downloads' \
//!   cargo test -p rift-catalog --release --test bench_real -- --ignored --nocapture
//! ```
//!
//! `RIFT_BENCH_LIBRARY` is scanned as a Fluffy root; the optional
//! `RIFT_BENCH_DOWNLOADS` as a Downloads root (`dd2_shaped_only`). The library
//! holds the investor's own mods, so this bench prints counts, shapes and
//! timings — never a package, option or file name, never a problem message
//! (those quote names), never a byte of content — and writes nothing anywhere:
//! the cache the cold scan builds stays in memory for the warm scan.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use rift_catalog::{Catalog, ScanStats, SearchFilters, SearchQuery};
use rift_core::{LibraryRoot, PackageFormat, RootKind};

/// The armour-variants collection in the investor's library: 281 options,
/// each with its own modinfo.ini (docs/BUILD-SPEC.md, "What the Riftstone is").
const LARGEST_COLLECTION_OPTIONS: usize = 281;

const QUERIES: [&str; 3] = ["top_314", "\"menu option\"", "variation 3"];

/// Repeats per query for the steady-state time.
const QUERY_RUNS: usize = 25;

fn root(id: &str, path: PathBuf, kind: RootKind, dd2_shaped_only: bool) -> LibraryRoot {
    LibraryRoot {
        id: id.to_string(),
        path,
        label: id.to_string(),
        kind,
        dd2_shaped_only,
    }
}

fn ms(duration: Duration) -> String {
    format!("{:.1} ms", duration.as_secs_f64() * 1000.0)
}

fn print_stats(label: &str, stats: ScanStats) {
    eprintln!(
        "  {label}: {} candidates — {} opened, {} from cache, {} passed over (not DD2; {} of \
         them from cache), {} unreadable, {} duplicates, {} gone",
        stats.candidates,
        stats.opened,
        stats.reused,
        stats.passed_over,
        stats.passed_over_from_cache,
        stats.unreadable,
        stats.duplicates,
        stats.gone
    );
}

#[test]
#[ignore = "bench: needs RIFT_BENCH_LIBRARY (read-only, prints counts only)"]
fn bench_real_library_scan_and_search() {
    let Some(library) = std::env::var_os("RIFT_BENCH_LIBRARY").map(PathBuf::from) else {
        eprintln!("RIFT_BENCH_LIBRARY is not set; nothing to scan");
        return;
    };
    let mut roots = vec![root("fluffy", library, RootKind::Fluffy, false)];
    if let Some(downloads) = std::env::var_os("RIFT_BENCH_DOWNLOADS").map(PathBuf::from) {
        roots.push(root("downloads", downloads, RootKind::Downloads, true));
    }

    // Cold: nothing cached.
    let mut events = 0usize;
    let started = Instant::now();
    let (cold, cache) = Catalog::scan(&roots, None, &mut |_| events += 1);
    let cold_time = started.elapsed();

    // Warm: the second launch, from the cache the cold scan built.
    let started = Instant::now();
    let (warm, _) = Catalog::scan(&roots, Some(&cache), &mut |_| {});
    let warm_time = started.elapsed();

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

    let mut per_root: BTreeMap<&str, usize> = BTreeMap::new();
    let mut formats: BTreeMap<&str, usize> = BTreeMap::new();
    let mut problems: BTreeMap<String, usize> = BTreeMap::new();
    for package in packages {
        *per_root.entry(package.root_id.as_str()).or_default() += 1;
        let format = match package.format {
            PackageFormat::Zip => "zip",
            PackageFormat::SevenZip => "7z",
            PackageFormat::Rar => "rar",
            PackageFormat::Folder => "folder",
        };
        *formats.entry(format).or_default() += 1;
        for problem in &package.problems {
            *problems.entry(format!("{:?}", problem.kind)).or_default() += 1;
        }
    }
    let linked = packages.iter().filter(|p| p.nexus.is_some()).count();

    eprintln!("rift-catalog bench (counts only)");
    eprintln!("  roots: {}", roots.len());
    print_stats("cold scan", cold.stats());
    print_stats("warm scan", warm.stats());
    eprintln!("  progress events (cold): {events}");
    eprintln!(
        "  packages: {} per root {per_root:?}, formats {formats:?}",
        packages.len()
    );
    eprintln!("  options: {options}; per package (sorted): {options_per_package:?}");
    eprintln!("  menu-only options: {menus}; options with a resolved parent: {parented}");
    eprintln!("  entries: {entries}; payload files: {payload}; skipped files: {skipped}");
    eprintln!("  packages linked to Nexus (file name or homepage): {linked}");
    eprintln!("  problems by kind: {problems:?}");
    eprintln!(
        "  cold: {}   warm (from cache): {}",
        ms(cold_time),
        ms(warm_time)
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
                limit: usize::MAX,
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

    let Some(library) = std::env::var_os("RIFT_BENCH_LIBRARY").map(PathBuf::from) else {
        eprintln!("RIFT_BENCH_LIBRARY is not set; nothing to time");
        return;
    };
    let fluffy = root("fluffy", library, RootKind::Fluffy, false);
    let started = Instant::now();
    let candidates = scan::list_root(&fluffy);
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

        let format = match source.format() {
            PackageFormat::Zip => "zip",
            PackageFormat::SevenZip => "7z",
            PackageFormat::Rar => "rar",
            PackageFormat::Folder => "folder",
        };
        let row = format!(
            "  {format:<6} {:>8.1} MB  {:>5} entries  {:>3} modinfo  {:>3} options | open+list {:>9}  modinfo {:>9}  classify {:>8}",
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
