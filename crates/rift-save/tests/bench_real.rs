//! READ-ONLY bench against a COPY of a real save. Ignored and env-gated: CI never runs it.
//!
//! RIFT_BENCH_SAVE=<a copy of a data0NNSlot.bin> RIFT_BENCH_SAVE_ACCOUNT=<account id> \
//!   cargo test -p rift-save --test bench_real -- --ignored --nocapture
//!
//! Prints sizes and counts only: never a string from the save, never the account.

use std::time::Instant;

#[test]
#[ignore = "bench: needs RIFT_BENCH_SAVE and RIFT_BENCH_SAVE_ACCOUNT"]
fn a_real_save_opens_and_parses_end_to_end() {
    let (Ok(path), Ok(account)) = (
        std::env::var("RIFT_BENCH_SAVE"),
        std::env::var("RIFT_BENCH_SAVE_ACCOUNT"),
    ) else {
        eprintln!("bench skipped: RIFT_BENCH_SAVE / RIFT_BENCH_SAVE_ACCOUNT not set");
        return;
    };
    let account: u32 = account.trim().parse().expect("the account id is a number");
    let bytes = std::fs::read(&path).expect("read the copy");
    let started = Instant::now();
    let payload = rift_save::open(&bytes, account).expect("the save opens with its own account");
    let opened = started.elapsed();
    let roots = rift_save::stream::parse(&payload).expect("the stream parses");
    let mut classes = 0usize;
    let mut types = std::collections::HashSet::new();
    rift_save::stream::walk(&roots, &mut |c| {
        classes += 1;
        types.insert(c.type_hash);
    });
    println!(
        "bench: file {} B, payload {} B, opened in {:?}, parsed in {:?}; {} roots, {} classes of {} types",
        bytes.len(),
        payload.len(),
        opened,
        started.elapsed() - opened,
        roots.len(),
        classes,
        types.len()
    );
    assert!(
        rift_save::open(&bytes, account.wrapping_add(1)).is_err(),
        "another account must not open it"
    );
}
