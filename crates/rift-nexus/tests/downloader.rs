//! The downloader (v0.3) against loopback stand-ins: one plays Nexus's API,
//! another plays the CDN a download link points at. Every request the CDN
//! stand-in serves is held to the identity headers too (see `support`).
//!
//! What is proven: a binary archive lands whole with its MD5; the CDN never
//! hears the API key or the site link's key; every failure (a body that
//! breaks off, a size that disagrees, a cancel, a stall, a refusal, a
//! redirect that breaks the rules, a panic in the progress callback) leaves
//! no file behind; an existing file is never touched; a slow download has no
//! total timeout; no error carries the link. Archive bytes, names and links
//! are invented.

mod support;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use md5::{Digest, Md5};
use rift_nexus::{Downloaded, Downloader, NexusError, NxmGrant, MAX_DOWNLOAD_REDIRECTS};
use serde_json::json;
use support::{dead_address, Reply, TestServer};

/// The invented signature every CDN link here carries: it must never show.
const SIGNATURE: &str = "invented-signature";
const API_KEY: &str = "invented-personal-key";
const GRANT_KEY: &str = "Invented-Grant_Key.0123";

/// `len` bytes of an invented archive: every byte value, no simple period.
fn archive(len: usize) -> Vec<u8> {
    (0..len)
        .map(|i| ((i % 251) as u8) ^ ((i / 251) as u8).wrapping_mul(7))
        .collect()
}

fn md5_hex(bytes: &[u8]) -> String {
    Md5::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// A link on the CDN stand-in, signed the way a CDN signs one.
fn link(cdn: &TestServer, path: &str) -> String {
    cdn.url(&format!("{path}?md5={SIGNATURE}&expires=1791987830"))
}

/// The partial file a download writes, in a fresh folder.
fn part_in(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join(".riftstone-0001.part")
}

fn downloader() -> Downloader {
    Downloader::new("0.3.0")
}

/// Every `(bytes so far, total)` the progress callback heard.
type ProgressLog = Vec<(u64, Option<u64>)>;

/// Download with a progress log and no cancel.
fn fetch(
    downloader: &Downloader,
    uri: &str,
    dest: &Path,
    expected: Option<u64>,
) -> (Result<Downloaded, NexusError>, ProgressLog) {
    let mut seen = Vec::new();
    let result = downloader.download_to(
        uri,
        dest,
        expected,
        &mut |done, total| seen.push((done, total)),
        &AtomicBool::new(false),
    );
    (result, seen)
}

/// The error's sentence and its Debug never show the link or a key.
fn assert_no_secret(err: &NexusError) {
    for shown in [err.to_string(), format!("{err:?}")] {
        for secret in [SIGNATURE, API_KEY, GRANT_KEY, "lantern.zip"] {
            assert!(!shown.contains(secret), "{secret} leaked: {shown}");
        }
    }
}

#[test]
fn a_binary_archive_lands_whole_with_its_md5() {
    let body = archive(3 * (1 << 20) + 123);
    let served = body.clone();
    let cdn = TestServer::start(move |_| Reply::bytes(served.clone()));
    let dir = tempfile::tempdir().unwrap();
    let dest = part_in(&dir);
    let uri = link(&cdn, "/cdn/2207/9001/lantern.zip");

    let (result, seen) = fetch(&downloader(), &uri, &dest, Some(body.len() as u64));
    let downloaded = result.unwrap();
    assert_eq!(
        downloaded,
        Downloaded {
            bytes: body.len() as u64,
            md5: md5_hex(&body),
        }
    );
    assert_eq!(std::fs::read(&dest).unwrap(), body, "every byte, in order");
    assert_eq!(
        rift_nexus::md5_file(&dest, &mut |_| {}).unwrap(),
        downloaded.md5,
        "the MD5 on the way is the file's MD5"
    );

    let total = Some(body.len() as u64);
    assert_eq!(
        seen.first(),
        Some(&(0, total)),
        "the total is known at once"
    );
    assert_eq!(seen.last(), Some(&(body.len() as u64, total)));
    assert!(
        seen.windows(2).all(|pair| pair[0].0 <= pair[1].0),
        "{seen:?}"
    );

    let request = &cdn.requests()[0];
    assert_eq!(request.method, "GET");
    assert_eq!(request.route(), "/cdn/2207/9001/lantern.zip");
    assert_eq!(request.header("apikey"), None);
    assert_eq!(request.header("Application-Name"), Some("Riftstone"));
    assert_eq!(request.header("Application-Version"), Some("0.3.0"));
}

#[test]
fn the_whole_path_from_link_to_archive_keeps_every_key_off_the_cdn() {
    let body = archive(70_000);
    let served = body.clone();
    let cdn = TestServer::start(move |_| Reply::bytes(served.clone()));
    let uri = link(&cdn, "/cdn/2207/9001/lantern.zip");
    let api = TestServer::start(move |_| {
        Reply::json(json!([{ "name": "Nexus CDN", "short_name": "Nexus CDN", "URI": uri }]))
    });
    let client = api.client().with_api_key(Some(API_KEY.into()));
    let grant = NxmGrant {
        key: GRANT_KEY.into(),
        expires_s: 1_791_987_830,
    };
    let mirrors = client.download_links(2207, 9001, Some(&grant)).unwrap();
    let shown = format!("{mirrors:?}");
    assert!(
        !shown.contains(SIGNATURE) && shown.contains("<redacted>"),
        "{shown}"
    );

    let dir = tempfile::tempdir().unwrap();
    let dest = part_in(&dir);
    let (result, _) = fetch(&client.downloader(), &mirrors[0].uri, &dest, Some(70_000));
    assert_eq!(result.unwrap().md5, md5_hex(&body));

    let asked = &api.requests()[0];
    assert_eq!(
        asked.header("apikey"),
        Some(API_KEY),
        "the API hears the key"
    );
    let fetched = &cdn.requests()[0];
    assert_eq!(fetched.header("apikey"), None, "the CDN never does");
    let everything = format!("{fetched:?}");
    assert!(
        !everything.contains(API_KEY) && !everything.contains(GRANT_KEY),
        "no key anywhere in the CDN request: {everything}"
    );
}

#[test]
fn a_body_that_breaks_off_leaves_nothing() {
    let cdn = TestServer::start(|_| Reply::bytes(archive(1000)).declaring(5000));
    let dir = tempfile::tempdir().unwrap();
    let dest = part_in(&dir);
    let (result, _) = fetch(&downloader(), &link(&cdn, "/cdn/lantern.zip"), &dest, None);
    match result {
        Err(err @ NexusError::Offline(_)) => {
            assert!(err.to_string().contains("broke off"), "{err}");
            assert_no_secret(&err);
        }
        other => panic!("expected the download to break off, got {other:?}"),
    }
    assert!(!dest.exists(), "the partial file is gone");
}

#[test]
fn a_body_without_a_length_that_ends_short_is_a_size_mismatch() {
    let cdn = TestServer::start(|_| Reply::bytes(archive(1000)).unstated_length());
    let dir = tempfile::tempdir().unwrap();
    let dest = part_in(&dir);
    let (result, seen) = fetch(
        &downloader(),
        &link(&cdn, "/cdn/lantern.zip"),
        &dest,
        Some(5000),
    );
    assert_eq!(
        result,
        Err(NexusError::SizeMismatch {
            expected: 5000,
            got: 1000
        })
    );
    assert_eq!(
        seen.first(),
        Some(&(0, Some(5000))),
        "Nexus's size is the total"
    );
    assert!(!dest.exists());
}

#[test]
fn a_length_that_disagrees_with_nexus_is_refused_before_the_body() {
    let cdn = TestServer::start(|_| Reply::bytes(archive(4096)));
    let dir = tempfile::tempdir().unwrap();
    let dest = part_in(&dir);
    let (result, seen) = fetch(
        &downloader(),
        &link(&cdn, "/cdn/lantern.zip"),
        &dest,
        Some(5000),
    );
    assert_eq!(
        result,
        Err(NexusError::SizeMismatch {
            expected: 5000,
            got: 4096
        })
    );
    assert!(seen.is_empty(), "not a byte was read: {seen:?}");
    assert!(!dest.exists());
}

#[test]
fn a_body_running_past_the_stated_size_is_stopped() {
    let stated: u64 = 1 << 20;
    let cdn = TestServer::start(|_| Reply::bytes(archive(3 << 20)).unstated_length());
    let dir = tempfile::tempdir().unwrap();
    let dest = part_in(&dir);
    let (result, _) = fetch(
        &downloader(),
        &link(&cdn, "/cdn/lantern.zip"),
        &dest,
        Some(stated),
    );
    match result {
        Err(NexusError::SizeMismatch { expected, got }) => {
            assert_eq!(expected, stated);
            assert!(got > stated && got <= 2 * stated, "stopped at once: {got}");
        }
        other => panic!("expected a size mismatch, got {other:?}"),
    }
    assert!(!dest.exists());
}

#[test]
fn a_cancel_from_the_progress_callback_leaves_no_file() {
    let cdn = TestServer::start(|_| {
        Reply::bytes(archive(3 << 20)).trickle(64 * 1024, Duration::from_millis(2))
    });
    let dir = tempfile::tempdir().unwrap();
    let dest = part_in(&dir);
    let cancel = AtomicBool::new(false);
    let mut calls = 0;
    let result = downloader().download_to(
        &link(&cdn, "/cdn/lantern.zip"),
        &dest,
        None,
        &mut |done, _| {
            calls += 1;
            if done > 0 {
                cancel.store(true, Ordering::SeqCst);
            }
        },
        &cancel,
    );
    assert_eq!(result, Err(NexusError::Cancelled));
    assert_eq!(calls, 2, "the head, then one read, then the cancel holds");
    assert!(!dest.exists(), "nothing left behind");
}

#[test]
fn a_cancel_before_the_start_sends_nothing() {
    let cdn = TestServer::start(|_| Reply::bytes(archive(10)));
    let dir = tempfile::tempdir().unwrap();
    let dest = part_in(&dir);
    let result = downloader().download_to(
        &link(&cdn, "/cdn/lantern.zip"),
        &dest,
        None,
        &mut |_, _| panic!("no progress for a download that never started"),
        &AtomicBool::new(true),
    );
    assert_eq!(result, Err(NexusError::Cancelled));
    assert!(!dest.exists());
    assert_eq!(cdn.request_count(), 0);
}

#[test]
fn a_panic_in_the_progress_callback_leaves_no_file() {
    let cdn = TestServer::start(|_| Reply::bytes(archive(200_000)));
    let dir = tempfile::tempdir().unwrap();
    let dest = part_in(&dir);
    let uri = link(&cdn, "/cdn/lantern.zip");
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        downloader().download_to(
            &uri,
            &dest,
            None,
            &mut |done, _| assert!(done == 0, "the window's callback fell over"),
            &AtomicBool::new(false),
        )
    }));
    assert!(outcome.is_err(), "the panic went through");
    assert!(!dest.exists(), "and the partial file did not stay");
}

#[test]
fn an_existing_destination_is_refused_and_left_untouched() {
    let cdn = TestServer::start(|_| Reply::bytes(archive(10)));
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("Lantern Glow Recolor.zip");
    std::fs::write(&dest, b"the player's own archive").unwrap();
    let (result, seen) = fetch(&downloader(), &link(&cdn, "/cdn/lantern.zip"), &dest, None);
    match result {
        Err(err @ NexusError::Disk(_)) => {
            assert!(err.to_string().contains("already there"), "{err}")
        }
        other => panic!("expected the disk refusal, got {other:?}"),
    }
    assert_eq!(std::fs::read(&dest).unwrap(), b"the player's own archive");
    assert!(seen.is_empty());
    assert_eq!(cdn.request_count(), 0, "refused before anything was asked");
}

#[test]
fn a_missing_folder_is_a_disk_failure() {
    let cdn = TestServer::start(|_| Reply::bytes(archive(10)));
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("gone").join(".riftstone-0001.part");
    let (result, _) = fetch(&downloader(), &link(&cdn, "/cdn/lantern.zip"), &dest, None);
    assert!(matches!(result, Err(NexusError::Disk(_))), "{result:?}");
    assert_eq!(cdn.request_count(), 0);
}

#[test]
fn mirror_refusals_are_named_and_leave_nothing() {
    let cases = [
        (401, NexusError::LinkExpired),
        (403, NexusError::LinkExpired),
        (410, NexusError::LinkExpired),
        (404, NexusError::FileGone),
        (
            429,
            NexusError::RateLimited {
                retry_after_s: Some(7),
            },
        ),
        (500, NexusError::Http(500)),
    ];
    for (status, expected) in cases {
        let cdn = TestServer::start(move |_| {
            Reply::raw(status, "<html>refused</html>").header("Retry-After", "7")
        });
        let dir = tempfile::tempdir().unwrap();
        let dest = part_in(&dir);
        let (result, seen) = fetch(
            &downloader(),
            &link(&cdn, "/cdn/lantern.zip"),
            &dest,
            Some(10),
        );
        let err = result.unwrap_err();
        assert_eq!(err, expected, "HTTP {status}");
        assert_no_secret(&err);
        assert!(seen.is_empty(), "HTTP {status}");
        assert!(!dest.exists(), "HTTP {status}");
    }
}

#[test]
fn a_stalled_download_gives_up_after_the_idle_timeout() {
    let cdn = TestServer::start(|_| {
        Reply::bytes(archive(64 * 1024)).stall_after(1024, Duration::from_millis(1500))
    });
    let dir = tempfile::tempdir().unwrap();
    let dest = part_in(&dir);
    let started = Instant::now();
    let (result, _) = fetch(
        &downloader().with_idle_timeout(Duration::from_millis(300)),
        &link(&cdn, "/cdn/lantern.zip"),
        &dest,
        None,
    );
    match result {
        Err(err @ NexusError::Offline(_)) => {
            assert!(err.to_string().contains("stalled"), "{err}");
            assert!(err.to_string().contains("300 ms"), "{err}");
            assert_no_secret(&err);
        }
        other => panic!("expected the stall to end the download, got {other:?}"),
    }
    assert!(
        started.elapsed() < Duration::from_millis(1400),
        "gave up before the CDN woke"
    );
    assert!(!dest.exists());
}

#[test]
fn a_slow_download_has_no_total_timeout() {
    let body = archive(4096);
    let served = body.clone();
    // Eight pieces, 150 ms apart: over a second in all, never 400 ms silent.
    let cdn = TestServer::start(move |_| {
        Reply::bytes(served.clone()).trickle(512, Duration::from_millis(150))
    });
    let dir = tempfile::tempdir().unwrap();
    let dest = part_in(&dir);
    let started = Instant::now();
    let (result, seen) = fetch(
        &downloader().with_idle_timeout(Duration::from_millis(400)),
        &link(&cdn, "/cdn/lantern.zip"),
        &dest,
        Some(4096),
    );
    assert_eq!(result.unwrap().md5, md5_hex(&body));
    assert!(
        started.elapsed() > Duration::from_millis(800),
        "it took its time"
    );
    assert!(seen.len() >= 4, "progress as the pieces came: {seen:?}");
    assert_eq!(std::fs::read(&dest).unwrap(), body);
}

/// A CDN that sends `/hop/N` on to `/hop/N-1` (the signature along) and
/// serves the archive at `/hop/0`.
fn hopping_cdn(body: Vec<u8>) -> TestServer {
    let base = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let base_in = std::sync::Arc::clone(&base);
    let server = TestServer::start(move |request| {
        let hop: usize = request
            .route()
            .trim_start_matches("/hop/")
            .parse()
            .expect("a hop number");
        if hop == 0 {
            Reply::bytes(body.clone())
        } else {
            let next = format!(
                "{}/hop/{}?md5={SIGNATURE}&expires=1791987830",
                base_in.lock().unwrap(),
                hop - 1
            );
            Reply::status(302).header("Location", &next)
        }
    });
    *base.lock().unwrap() = format!("http://{}", server.addr());
    server
}

#[test]
fn redirects_are_followed_at_most_five_times_and_carry_no_referer() {
    let body = archive(2048);
    let cdn = hopping_cdn(body.clone());
    let dir = tempfile::tempdir().unwrap();
    let dest = part_in(&dir);
    let start = link(&cdn, &format!("/hop/{MAX_DOWNLOAD_REDIRECTS}"));
    let (result, _) = fetch(&downloader(), &start, &dest, Some(2048));
    assert_eq!(result.unwrap().md5, md5_hex(&body), "five hops are fine");
    let requests = cdn.requests();
    assert_eq!(requests.len(), MAX_DOWNLOAD_REDIRECTS + 1);
    for request in &requests {
        assert_eq!(request.header("Referer"), None, "the signed link stays put");
        assert_eq!(request.header("apikey"), None);
    }

    let too_far = hopping_cdn(body);
    let dir = tempfile::tempdir().unwrap();
    let dest = part_in(&dir);
    let start = link(&too_far, &format!("/hop/{}", MAX_DOWNLOAD_REDIRECTS + 1));
    let (result, _) = fetch(&downloader(), &start, &dest, Some(2048));
    match result {
        Err(err @ NexusError::BadResponse(_)) => {
            assert!(err.to_string().contains("more than 5 redirects"), "{err}");
            assert_no_secret(&err);
        }
        other => panic!("expected the sixth redirect to be refused, got {other:?}"),
    }
    assert_eq!(
        too_far.request_count(),
        MAX_DOWNLOAD_REDIRECTS + 1,
        "the sixth hop's target was never asked"
    );
    assert!(!dest.exists());
}

#[test]
fn a_redirect_to_plain_http_elsewhere_is_refused() {
    // 192.0.2.1 is TEST-NET-1 (RFC 5737): never routed, and never dialled,
    // because the redirect is refused before any connection is opened.
    let cdn = TestServer::start(|_| {
        Reply::status(302).header(
            "Location",
            &format!("http://192.0.2.1/cdn/lantern.zip?md5={SIGNATURE}"),
        )
    });
    let dir = tempfile::tempdir().unwrap();
    let dest = part_in(&dir);
    let started = Instant::now();
    let (result, _) = fetch(&downloader(), &link(&cdn, "/cdn/lantern.zip"), &dest, None);
    match result {
        Err(err @ NexusError::BadResponse(_)) => {
            assert!(err.to_string().contains("not https"), "{err}");
            assert_no_secret(&err);
        }
        other => panic!("expected the plain-http hop to be refused, got {other:?}"),
    }
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "nothing was dialled"
    );
    assert!(!dest.exists());
}

#[test]
fn a_link_that_is_not_https_is_refused_before_anything_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let dest = part_in(&dir);
    for uri in [
        format!("http://192.0.2.1/cdn/lantern.zip?md5={SIGNATURE}"),
        format!("ftp://127.0.0.1/cdn/lantern.zip?md5={SIGNATURE}"),
        "lantern.zip".to_string(),
        String::new(),
    ] {
        let (result, seen) = fetch(&downloader(), &uri, &dest, None);
        match result {
            Err(err @ NexusError::BadResponse(_)) => assert_no_secret(&err),
            other => panic!("{uri}: expected a refusal, got {other:?}"),
        }
        assert!(seen.is_empty());
        assert!(!dest.exists(), "{uri}: not even an empty file");
    }
}

#[test]
fn an_unreachable_mirror_is_offline_and_leaves_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let dest = part_in(&dir);
    let uri = format!(
        "{}/cdn/lantern.zip?md5={SIGNATURE}&expires=1791987830",
        dead_address()
    );
    let (result, _) = fetch(&downloader(), &uri, &dest, None);
    match result {
        Err(err @ NexusError::Offline(_)) => {
            assert!(err.to_string().contains("could not connect"), "{err}");
            assert_no_secret(&err);
        }
        other => panic!("expected Offline, got {other:?}"),
    }
    assert!(!dest.exists());
}
