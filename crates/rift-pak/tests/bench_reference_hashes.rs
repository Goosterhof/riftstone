//! Bench: cross-check `path_hash` against hashes another implementation made.
//!
//! Ignored by default. Point `RIFT_BENCH_PAK_HASHES` at a text file of
//! `<lower hex> <upper hex> <path>` lines and run
//! `cargo test -p rift-pak --test bench_reference_hashes -- --ignored --nocapture`.
//!
//! The file is produced outside this repository, e.g. by an independent port
//! of REE.PAK.Tool's C# hashing run over one of its `Projects/*.list` files.
//! A path may be spelled in any case and with either slash: the expected hash
//! is always the canonical one, so the case and separator folding is checked
//! too. Never commit such a file; it is reference data, not a fixture.

use std::time::Instant;

#[test]
#[ignore = "bench: set RIFT_BENCH_PAK_HASHES to a '<lower> <upper> <path>' file"]
fn every_reference_hash_matches() {
    let Ok(file) = std::env::var("RIFT_BENCH_PAK_HASHES") else {
        eprintln!("skipped: RIFT_BENCH_PAK_HASHES is not set");
        return;
    };
    let text = std::fs::read_to_string(&file)
        .unwrap_or_else(|err| panic!("could not read the reference file {file}: {err}"));

    let started = Instant::now();
    let mut checked = 0usize;
    let mut mismatches = Vec::new();
    for (number, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let mut fields = line.splitn(3, ' ');
        let (Some(lower), Some(upper), Some(path)) = (fields.next(), fields.next(), fields.next())
        else {
            panic!(
                "line {} is not '<lower> <upper> <path>': {line}",
                number + 1
            );
        };
        let lower = u32::from_str_radix(lower, 16)
            .unwrap_or_else(|err| panic!("line {}: bad lower hash: {err}", number + 1));
        let upper = u32::from_str_radix(upper, 16)
            .unwrap_or_else(|err| panic!("line {}: bad upper hash: {err}", number + 1));

        let hash = rift_pak::path_hash(path);
        if (hash.lower, hash.upper) != (lower, upper) {
            mismatches.push(format!(
                "line {}: {path}: expected {lower:08x} {upper:08x}, got {:08x} {:08x}",
                number + 1,
                hash.lower,
                hash.upper
            ));
        }
        checked += 1;
    }

    eprintln!(
        "checked {checked} paths from {file} in {} ms, {} mismatches",
        started.elapsed().as_millis(),
        mismatches.len()
    );
    assert!(checked > 0, "the reference file {file} holds no paths");
    assert!(
        mismatches.is_empty(),
        "{} of {checked} hashes differ; first ones:\n{}",
        mismatches.len(),
        mismatches
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
