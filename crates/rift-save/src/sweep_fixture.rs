//! The privacy sweep's own save, `scripts/fixtures/sweep-invented-save.bin`: an invented party
//! sealed by this crate's test-only sealer, so the sweep's Python opens saves and carries no
//! sealer of its own (RS-8). `RIFT_WRITE_SWEEP_FIXTURE=1 cargo test -p rift-save sweep_fixture`
//! writes it again; this test fails while the committed file opens to anything else.
//!
//! The check is what the file opens to, not its bytes: deflate's output depends on the backend a
//! build links, and the workspace links flate2's zlib backend where this crate alone links the
//! Rust one.

use std::path::PathBuf;

use crate::stream::write::{stream, WClass, W};
use crate::{open, seal};

/// The account it is sealed for. The sweep's self-test opens it with the same.
const ACCOUNT: u32 = 4_242_001;
/// One of the record types whose `Name` is the game's own id (`GAME_ID_RECORDS` in
/// `scripts/dd2_saves.py`).
const GAME_ID_RECORD: u32 = 0xA5E6_74E7;

/// The classes the sweep's self-test reads: two people's names, a Rift id, a game string beside
/// a name, and a game id where a name could stand.
fn payload() -> Vec<u8> {
    let arisen = WClass::new("app.Invented")
        .with("Name", W::Str("Odalric Vernworthy".into()))
        .with("Title", W::Str("Dragon's Dogma 2".into()));
    let pawn = WClass::new("app.Invented")
        .with("_Name", W::Str("Wrenna".into()))
        .with("_SearchId", W::Str("INVENTED7Q".into()));
    let game_id = WClass {
        type_hash: GAME_ID_RECORD,
        fields: Vec::new(),
    }
    .with("Name", W::Str("sm000001_Invented_0000000001".into()));
    stream(&[("root", arisen), ("root", pawn), ("root", game_id)])
}

fn committed() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/fixtures/sweep-invented-save.bin")
}

#[test]
fn the_sweeps_save_opens_to_the_invented_party_and_nothing_else() {
    if std::env::var_os("RIFT_WRITE_SWEEP_FIXTURE").is_some() {
        std::fs::write(committed(), seal(&payload(), ACCOUNT, true))
            .expect("write the sweep's fixture");
    }
    let file = std::fs::read(committed()).expect("scripts/fixtures/sweep-invented-save.bin");
    assert_eq!(
        open(&file, ACCOUNT),
        Ok(payload()),
        "write it again with RIFT_WRITE_SWEEP_FIXTURE=1"
    );
    assert_eq!(
        &file[8..12],
        &0x18u32.to_le_bytes(),
        "deflated, as the 2026 slots are"
    );
    assert!(
        open(&file, ACCOUNT + 1).is_err(),
        "it opens for its own account only"
    );
}
