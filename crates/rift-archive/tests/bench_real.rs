//! Bench suite: real packages from the investor's own mods folder, READ-ONLY.
//!
//! Every test here is `#[ignore]`d and env-gated, so `cargo test` never
//! touches real data. Run one deliberately:
//!
//! ```sh
//! RIFT_BENCH_RAR='/mnt/c/<install dir>/mods/<a mod>.rar' \
//!   cargo test -p rift-archive --test bench_real -- --ignored --nocapture
//! ```
//!
//! The RAR bench only LISTS: it opens the archive (UnRAR's list mode plus the
//! frame walk, headers only) and prints counts — never entry names, never a
//! byte of content, never a file written.

use std::path::PathBuf;

use rift_archive::{sniff, RarSource};
use rift_core::{PackageFormat, PackageSource};

#[test]
#[ignore = "bench: needs RIFT_BENCH_RAR pointing at a real RAR (read-only listing)"]
fn bench_real_rar_lists_read_only() {
    let Some(path) = std::env::var_os("RIFT_BENCH_RAR").map(PathBuf::from) else {
        eprintln!("RIFT_BENCH_RAR is not set; nothing to list");
        return;
    };
    let started = std::time::Instant::now();
    assert_eq!(sniff(&path).expect("sniff"), Some(PackageFormat::Rar));
    let mut source = RarSource::open(&path).expect("the real RAR opens and lists");
    let entries = source.entries().expect("entries");
    let bytes: u64 = entries.iter().map(|entry| entry.size).sum();
    let depth = entries
        .iter()
        .map(|entry| entry.path.split('/').count())
        .max()
        .unwrap_or(0);
    eprintln!(
        "bench RAR: {} entries, {bytes} bytes unpacked, deepest path {depth} segments, solid: {}, listed in {:?}",
        entries.len(),
        source.is_solid(),
        started.elapsed()
    );
    assert!(
        !entries.is_empty(),
        "a real mod archive lists at least one file"
    );
    assert_eq!(source.decode_passes(), 0, "listing decompresses nothing");
}
