//! The downloader (v0.3): one archive from a Nexus mirror into a new file.
//!
//! It is a client of its own, apart from [`crate::NexusClient`], and it never
//! holds the API key: the download path takes only a mirror's link (the
//! `uri` of a [`crate::DownloadMirror`]), which the CDN signed for this
//! account and file. So no key can ride along a redirect, whatever host it
//! names. What it does hold to:
//!
//! - **Identity.** `Application-Name`, `Application-Version` and the
//!   Riftstone's `User-Agent` on every request, like the API client.
//! - **https only**, except towards a loopback test server; every redirect
//!   hop is held to the same rule, at most [`crate::MAX_DOWNLOAD_REDIRECTS`]
//!   of them, and no `Referer` is sent along (it would carry the signed link
//!   to the next host).
//! - **Patience without a deadline.** Connecting may take
//!   [`crate::DOWNLOAD_CONNECT_TIMEOUT`]; after that only silence counts: the
//!   answer's head and every read must arrive within the idle timeout
//!   ([`crate::DOWNLOAD_IDLE_TIMEOUT`]), and there is no total timeout, so a
//!   large archive on a slow line finishes.
//! - **A new file or none.** The destination is created new (never over an
//!   existing file); the bytes stream in reads of up to
//!   [`crate::DOWNLOAD_CHUNK_BYTES`], the MD5 computed on the way; `cancel` is
//!   checked between reads. On every failure and on cancel the file is
//!   deleted, even when the progress callback panics.
//! - **The size.** The byte count must match the server's `Content-Length`
//!   and the size Nexus stated ([`NexusError::SizeMismatch`]); a body that
//!   runs past either is stopped as soon as it does.
//! - **Secrets.** The link never appears in an error, a log line or `Debug`
//!   output: errors are built from fixed words and the innermost transport
//!   reason, with the link scrubbed out of even that.
//!
//! Blocking, like the rest of the crate: call it from a worker thread.

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, ErrorKind, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use md5::{Digest, Md5};
use reqwest::blocking::{Client, Response};
use reqwest::header::RETRY_AFTER;
use reqwest::{StatusCode, Url};

use crate::client::{
    clean_version, is_loopback_url, parse_retry_after, root_cause, transport_error, user_agent,
};
use crate::error::NexusError;
use crate::hash::hex;
use crate::{
    APP_NAME, DOWNLOAD_CHUNK_BYTES, DOWNLOAD_CONNECT_TIMEOUT, DOWNLOAD_IDLE_TIMEOUT,
    MAX_DOWNLOAD_REDIRECTS,
};

/// What a finished download wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Downloaded {
    /// Bytes written, equal to every size that was stated.
    pub bytes: u64,
    /// The archive's MD5, lower-case hex, computed while it streamed: what the
    /// verifying step asks Nexus about ([`crate::NexusClient::confirm_download`]).
    pub md5: String,
}

/// Fetches archives from Nexus's mirrors. Cheap to make and to clone: each
/// download builds its own connection and drops it after.
#[derive(Debug, Clone)]
pub struct Downloader {
    app_version: String,
    idle_timeout: Duration,
}

/// A link the downloader will follow, and whether it talks to a loopback
/// server (no proxy, `http` allowed).
pub(crate) struct UsableLink {
    url: Url,
    direct: bool,
}

/// `https`, or `http` towards a loopback test server; anything else (another
/// scheme, a link that does not parse) is no link to download from.
pub(crate) fn usable_link(uri: &str) -> Option<UsableLink> {
    let url = Url::parse(uri.trim()).ok()?;
    let loopback = is_loopback_url(&url);
    match url.scheme() {
        "https" => Some(UsableLink {
            url,
            direct: loopback,
        }),
        "http" if loopback => Some(UsableLink { url, direct: true }),
        _ => None,
    }
}

impl Downloader {
    /// A downloader announcing `app_version` (cleaned like the API client's).
    pub fn new(app_version: &str) -> Self {
        Downloader {
            app_version: clean_version(app_version),
            idle_timeout: DOWNLOAD_IDLE_TIMEOUT,
        }
    }

    /// How long the answer's head, and then each read, may keep the
    /// downloader waiting (default [`DOWNLOAD_IDLE_TIMEOUT`]). Tests and
    /// benches shorten it; there is never a total timeout.
    pub fn with_idle_timeout(mut self, idle: Duration) -> Self {
        self.idle_timeout = idle;
        self
    }

    /// The version every download announces.
    pub fn app_version(&self) -> &str {
        &self.app_version
    }

    /// Download `uri` into `dest`, a file that must not exist yet.
    ///
    /// `expected` is the size Nexus stated for the file, when it did.
    /// `progress` hears `(bytes so far, total)` once when the answer's head
    /// arrives (with 0 bytes) and after every read; the total is the
    /// server's `Content-Length`, else `expected`. `cancel` is checked before
    /// anything is sent and between reads; setting it from inside `progress`
    /// stops the download at the next check.
    ///
    /// On success `dest` holds exactly the bytes that arrived, flushed to
    /// disk. On every failure and on cancel `dest` is deleted (an existing
    /// file at `dest` is refused first and never touched).
    ///
    /// Errors: [`NexusError::Cancelled`]; [`NexusError::Disk`] (the file
    /// exists or cannot be written); [`NexusError::SizeMismatch`];
    /// [`NexusError::Offline`] (no connection, a stall, a body that broke
    /// off); [`NexusError::LinkExpired`] (the mirror answered 401, 403 or
    /// 410: its signed link no longer serves); [`NexusError::FileGone`]
    /// (404); [`NexusError::RateLimited`] (429); [`NexusError::BadResponse`]
    /// (the link is not https, or the redirects broke the rules);
    /// [`NexusError::Http`] for any other status.
    pub fn download_to(
        &self,
        uri: &str,
        dest: &Path,
        expected: Option<u64>,
        progress: &mut dyn FnMut(u64, Option<u64>),
        cancel: &AtomicBool,
    ) -> Result<Downloaded, NexusError> {
        if cancel.load(Ordering::SeqCst) {
            return Err(NexusError::Cancelled);
        }
        let link = usable_link(uri).ok_or_else(|| {
            NexusError::BadResponse(
                "the download link is not an https link the Riftstone can use".to_string(),
            )
        })?;
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dest)
            .map_err(|err| disk_error(&err))?;
        let mut part = PartFile {
            path: dest,
            writer: Some(BufWriter::with_capacity(DOWNLOAD_CHUNK_BYTES, file)),
            kept: false,
        };
        let downloaded = self
            .stream(link, &mut part, expected, progress, cancel)
            .map_err(|err| scrub_error(err, uri))?;
        part.kept = true;
        log::info!("a download landed: {} bytes", downloaded.bytes);
        Ok(downloaded)
    }

    fn stream(
        &self,
        link: UsableLink,
        part: &mut PartFile<'_>,
        expected: Option<u64>,
        progress: &mut dyn FnMut(u64, Option<u64>),
        cancel: &AtomicBool,
    ) -> Result<Downloaded, NexusError> {
        let client = self.client(link.direct)?;
        let mut response = client
            .get(link.url)
            .header("Application-Name", APP_NAME)
            .header("Application-Version", self.app_version.as_str())
            .send()
            .map_err(download_transport_error)?;
        let status = response.status();
        if !status.is_success() {
            return Err(status_error(status, &response));
        }
        if cancel.load(Ordering::SeqCst) {
            return Err(NexusError::Cancelled);
        }
        let declared = response.content_length();
        if let (Some(declared), Some(expected)) = (declared, expected) {
            if declared != expected {
                return Err(NexusError::SizeMismatch {
                    expected,
                    got: declared,
                });
            }
        }
        let total = declared.or(expected);
        progress(0, total);

        let mut hasher = Md5::new();
        let mut buffer = vec![0u8; DOWNLOAD_CHUNK_BYTES];
        let mut done: u64 = 0;
        loop {
            if cancel.load(Ordering::SeqCst) {
                return Err(NexusError::Cancelled);
            }
            let read = match response.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => read,
                Err(err) if err.kind() == ErrorKind::Interrupted => continue,
                Err(err) => return Err(self.read_error(&err)),
            };
            done += read as u64;
            if let Some(limit) = total.filter(|limit| done > *limit) {
                return Err(NexusError::SizeMismatch {
                    expected: limit,
                    got: done,
                });
            }
            hasher.update(&buffer[..read]);
            part.write(&buffer[..read])?;
            progress(done, total);
        }
        if cancel.load(Ordering::SeqCst) {
            return Err(NexusError::Cancelled);
        }
        for stated in [expected, declared].into_iter().flatten() {
            if done != stated {
                return Err(NexusError::SizeMismatch {
                    expected: stated,
                    got: done,
                });
            }
        }
        part.finish()?;
        Ok(Downloaded {
            bytes: done,
            md5: hex(&hasher.finalize()),
        })
    }

    /// The download client. Its `timeout` is the IDLE timeout: reqwest's
    /// blocking client applies it to each wait on its own (the answer's head,
    /// then every read of the body) and never to the whole transfer, as long
    /// as no request sets a timeout of its own (none does).
    fn client(&self, direct: bool) -> Result<Client, NexusError> {
        let mut builder = Client::builder()
            .user_agent(user_agent(&self.app_version))
            .connect_timeout(DOWNLOAD_CONNECT_TIMEOUT)
            .timeout(self.idle_timeout)
            .redirect(redirect_policy(direct))
            .referer(false);
        builder = if direct {
            builder.no_proxy()
        } else {
            builder.https_only(true)
        };
        builder.build().map_err(|err| {
            NexusError::Offline(format!(
                "the download client could not start: {}",
                root_cause(&err.without_url())
            ))
        })
    }

    fn read_error(&self, err: &std::io::Error) -> NexusError {
        if is_timeout(err) {
            NexusError::Offline(format!(
                "the download stalled: nothing arrived for {}",
                spoken(self.idle_timeout)
            ))
        } else {
            NexusError::Offline(format!("the download broke off: {}", root_cause(err)))
        }
    }
}

/// Every hop https (or, for a loopback test server, http to loopback), at
/// most [`MAX_DOWNLOAD_REDIRECTS`] hops. reqwest's `previous()` starts with
/// the first link, which is no redirect.
fn redirect_policy(allow_loopback_http: bool) -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(move |attempt| {
        if attempt.previous().len() > MAX_DOWNLOAD_REDIRECTS {
            return attempt.error(format!(
                "the download was sent on more than {MAX_DOWNLOAD_REDIRECTS} redirects"
            ));
        }
        let next = attempt.url();
        let allowed = next.scheme() == "https"
            || (allow_loopback_http && next.scheme() == "http" && is_loopback_url(next));
        if allowed {
            attempt.follow()
        } else {
            attempt.error("the download was redirected to a link that is not https".to_string())
        }
    })
}

/// A transport failure, without the link: reqwest attaches the URL to a
/// request's error, so it is stripped before anything reads the error.
fn download_transport_error(err: reqwest::Error) -> NexusError {
    let err = err.without_url();
    if err.is_redirect() {
        NexusError::BadResponse(root_cause(&err))
    } else {
        transport_error(err)
    }
}

/// A mirror's refusal. 401, 403 and 410 mean its signed link no longer
/// serves (it expired, or it was made for another session): a fresh link
/// is the remedy, never a new key.
fn status_error(status: StatusCode, response: &Response) -> NexusError {
    match status.as_u16() {
        401 | 403 | 410 => NexusError::LinkExpired,
        404 => NexusError::FileGone,
        429 => NexusError::RateLimited {
            retry_after_s: response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(parse_retry_after),
        },
        code => NexusError::Http(code),
    }
}

fn disk_error(err: &std::io::Error) -> NexusError {
    match err.kind() {
        ErrorKind::AlreadyExists => {
            NexusError::Disk("a file of that name is already there; it was left alone".to_string())
        }
        _ => NexusError::Disk(err.to_string()),
    }
}

/// Did this read fail because nothing arrived in time?
fn is_timeout(err: &std::io::Error) -> bool {
    if err.kind() == ErrorKind::TimedOut {
        return true;
    }
    let mut current: Option<&(dyn std::error::Error + 'static)> = err
        .get_ref()
        .map(|inner| inner as &(dyn std::error::Error + 'static));
    while let Some(error) = current {
        if error
            .downcast_ref::<reqwest::Error>()
            .is_some_and(reqwest::Error::is_timeout)
            || error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|io| io.kind() == ErrorKind::TimedOut)
        {
            return true;
        }
        current = error.source();
    }
    false
}

/// `60 seconds`, `1 second`, `250 ms`.
fn spoken(duration: Duration) -> String {
    match duration.as_secs() {
        0 => format!("{} ms", duration.as_millis()),
        1 => "1 second".to_string(),
        seconds => format!("{seconds} seconds"),
    }
}

/// The error with every trace of the link replaced: the whole link, its
/// query (the signature) and its path. Belt and braces: the URL is stripped
/// from reqwest's errors already.
fn scrub_error(err: NexusError, uri: &str) -> NexusError {
    let scrub = |text: String| {
        let uri = uri.trim();
        let query = uri.split_once('?').map(|(_, query)| query);
        let path = Url::parse(uri).ok().map(|url| url.path().to_string());
        let mut text = text;
        for secret in [Some(uri), query, path.as_deref()].into_iter().flatten() {
            if secret.len() >= 8 && text.contains(secret) {
                text = text.replace(secret, "<the download link>");
            }
        }
        text
    };
    match err {
        NexusError::Offline(text) => NexusError::Offline(scrub(text)),
        NexusError::BadResponse(text) => NexusError::BadResponse(scrub(text)),
        NexusError::Disk(text) => NexusError::Disk(scrub(text)),
        other => other,
    }
}

/// The partial file: deleted when dropped unless kept, so every early
/// return, every `?` and a panic in the progress callback leave nothing.
struct PartFile<'a> {
    path: &'a Path,
    writer: Option<BufWriter<File>>,
    kept: bool,
}

impl PartFile<'_> {
    fn write(&mut self, bytes: &[u8]) -> Result<(), NexusError> {
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| NexusError::Disk("the download file was already closed".to_string()))?;
        writer.write_all(bytes).map_err(|err| disk_error(&err))
    }

    /// Flush and sync: the bytes are on disk before anyone renames the file.
    fn finish(&mut self) -> Result<(), NexusError> {
        let writer = self
            .writer
            .take()
            .ok_or_else(|| NexusError::Disk("the download file was already closed".to_string()))?;
        let file = writer.into_inner().map_err(|err| disk_error(err.error()))?;
        file.sync_all().map_err(|err| disk_error(&err))
    }
}

impl Drop for PartFile<'_> {
    fn drop(&mut self) {
        if self.kept {
            return;
        }
        // Close first: Windows cannot delete a file that is still open.
        drop(self.writer.take());
        if let Err(err) = std::fs::remove_file(self.path) {
            if err.kind() != ErrorKind::NotFound {
                log::warn!("a cut-off download could not be removed: {err}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_https_or_loopback_http_is_a_link() {
        assert!(
            usable_link("https://cdn.example.invalid/2207/lantern.zip?sig=x")
                .is_some_and(|l| !l.direct)
        );
        assert!(usable_link("http://127.0.0.1:9/lantern.zip").is_some_and(|l| l.direct));
        assert!(usable_link("http://[::1]:9/lantern.zip").is_some_and(|l| l.direct));
        assert!(usable_link("https://localhost:9/lantern.zip").is_some_and(|l| l.direct));
        for refused in [
            "http://cdn.example.invalid/lantern.zip",
            "ftp://127.0.0.1/lantern.zip",
            "file:///tmp/lantern.zip",
            "not a link",
            "",
        ] {
            assert!(usable_link(refused).is_none(), "{refused}");
        }
    }

    #[test]
    fn durations_are_spoken() {
        assert_eq!(spoken(Duration::from_secs(60)), "60 seconds");
        assert_eq!(spoken(Duration::from_secs(1)), "1 second");
        assert_eq!(spoken(Duration::from_millis(250)), "250 ms");
    }

    #[test]
    fn errors_are_scrubbed_of_the_link() {
        let uri = "https://cdn.example.invalid/files/2207/lantern.zip?md5=SIGNED&expires=99";
        let scrubbed = scrub_error(
            NexusError::Offline(format!(
                "error for url ({uri}); path /files/2207/lantern.zip; query md5=SIGNED&expires=99"
            )),
            uri,
        );
        let text = scrubbed.to_string();
        assert!(
            !text.contains("SIGNED") && !text.contains("lantern.zip"),
            "{text}"
        );
        assert!(text.contains("<the download link>"), "{text}");
        // Other errors pass untouched.
        assert_eq!(scrub_error(NexusError::FileGone, uri), NexusError::FileGone);
    }

    #[test]
    fn a_timeout_is_told_from_a_break() {
        assert!(is_timeout(&std::io::Error::from(ErrorKind::TimedOut)));
        assert!(is_timeout(&std::io::Error::other(std::io::Error::from(
            ErrorKind::TimedOut
        ))));
        assert!(!is_timeout(&std::io::Error::from(ErrorKind::UnexpectedEof)));
    }

    #[test]
    fn the_downloader_can_cross_threads() {
        fn assert_send_sync<T: Send + Sync + Clone>() {}
        assert_send_sync::<Downloader>();
    }
}
