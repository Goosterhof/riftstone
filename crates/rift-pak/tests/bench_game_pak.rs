//! READ-ONLY bench against the real game pak. Ignored and env-gated: CI never runs it.
//!
//! RIFT_BENCH_GAME_PAK=<the game folder>/re_chunk_000.pak \
//!   cargo test -p rift-pak --test bench_game_pak -- --ignored --nocapture
//!
//! Opens the pak read-only and reads the text files the Muster names things from. Prints
//! counts and timings only.

use std::time::Instant;

use rift_pak::game::GamePak;
use rift_pak::msg::Msg;

const MUSTER_TEXTS: [&str; 17] = [
    "natives/stm/message/ui/itemname.msg.24",
    "natives/stm/message/ui/itemname_end.msg.24",
    "natives/stm/appdata/extracontents/contents_0011/itemname_contents_0011.msg.24",
    "natives/stm/message/ui/job.msg.24",
    "natives/stm/message/ui/personality.msg.24",
    "natives/stm/message/ui/subskill.msg.24",
    "natives/stm/message/ui/subskill_tu_end.msg.24",
    "natives/stm/message/ui/job01act.msg.24",
    "natives/stm/message/ui/job02act.msg.24",
    "natives/stm/message/ui/job03act.msg.24",
    "natives/stm/message/ui/job04act.msg.24",
    "natives/stm/message/ui/job05act.msg.24",
    "natives/stm/message/ui/job06act.msg.24",
    "natives/stm/message/ui/job07act.msg.24",
    "natives/stm/message/ui/job08act.msg.24",
    "natives/stm/message/ui/job09act.msg.24",
    "natives/stm/message/ui/job10act.msg.24",
];

#[test]
#[ignore = "bench: needs RIFT_BENCH_GAME_PAK"]
fn the_real_game_pak_yields_the_muster_texts() {
    let Ok(path) = std::env::var("RIFT_BENCH_GAME_PAK") else {
        eprintln!("bench skipped: RIFT_BENCH_GAME_PAK not set");
        return;
    };
    let started = Instant::now();
    let pak = GamePak::open(std::path::Path::new(&path)).expect("the game pak opens");
    println!(
        "bench: {} entries, table read and decrypted in {:?}",
        pak.len(),
        started.elapsed()
    );
    let reading = Instant::now();
    let (mut files, mut entries, mut english) = (0, 0, 0);
    for text in MUSTER_TEXTS {
        let bytes = pak
            .read(text)
            .expect("the entry reads")
            .unwrap_or_else(|| panic!("missing: {text}"));
        let msg = Msg::parse(&bytes).expect("the text parses");
        files += 1;
        entries += msg.len();
        english += msg.names().filter(|n| msg.english(n).is_some()).count();
    }
    println!(
        "bench: {files} text files, {entries} entries, {english} with English text, read in {:?}",
        reading.elapsed()
    );
    assert_eq!(files, MUSTER_TEXTS.len());
}
