//! A stand-in for Nexus Mods: a one-thread HTTP/1.1 server on 127.0.0.1:0
//! that records every request and answers from a closure. The default test
//! run never touches the network.
//!
//! It also holds the client to Nexus's acceptable-use policy: EVERY request it
//! serves must name the Riftstone (`Application-Name: Riftstone`, an
//! `Application-Version`, and a `User-Agent` of `Riftstone/<that version> (…`).
//! A request without them is recorded as a violation, and the server fails
//! the test when it is dropped, so every test that talks to it enforces the
//! headers, including tests written later for request paths that do not exist
//! yet.
//!
//! (v0.3) A second stand-in plays the CDN a download comes from: binary
//! bodies ([`Reply::bytes`]), a `Content-Length` that lies or is left out, a
//! body that trickles in or stalls halfway.

// Each test file uses its own part of the stand-in.
#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use rift_nexus::NexusClient;

/// One request as the server saw it.
#[derive(Debug, Clone)]
pub struct Recorded {
    pub method: String,
    /// The request target: the path and, when there is one, `?` and the query.
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Recorded {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).expect("the request body is JSON")
    }

    /// The GraphQL query text of a POST.
    pub fn query(&self) -> String {
        self.json()["query"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    }

    /// The path without its query.
    pub fn route(&self) -> &str {
        self.path
            .split_once('?')
            .map_or(&self.path, |(path, _)| path)
    }

    /// The query's pairs, decoded (`+` is a space, `%XX` a byte).
    pub fn query_pairs(&self) -> Vec<(String, String)> {
        self.path
            .split_once('?')
            .map(|(_, query)| form_pairs(query))
            .unwrap_or_default()
    }

    /// An `application/x-www-form-urlencoded` body's pairs, decoded.
    pub fn form(&self) -> Vec<(String, String)> {
        form_pairs(&String::from_utf8_lossy(&self.body))
    }
}

/// `a=b&c=d`, each side decoded the way a form is: `+` is a space.
pub fn form_pairs(text: &str) -> Vec<(String, String)> {
    fn decode(text: &str) -> String {
        let bytes = text.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut at = 0;
        while at < bytes.len() {
            match bytes[at] {
                b'+' => {
                    out.push(b' ');
                    at += 1;
                }
                b'%' if at + 3 <= bytes.len() => {
                    let byte = u8::from_str_radix(&text[at + 1..at + 3], 16)
                        .expect("a %XX escape in the request");
                    out.push(byte);
                    at += 3;
                }
                byte => {
                    out.push(byte);
                    at += 1;
                }
            }
        }
        String::from_utf8(out).expect("the request's text is UTF-8")
    }
    text.split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(name), decode(value))
        })
        .collect()
}

/// How a reply states its body's length.
#[derive(Debug, Clone, Copy)]
enum Length {
    /// `Content-Length` is the body's own length.
    Exact,
    /// `Content-Length` says this, whatever the body is (a short body when it
    /// says more: the connection closes early).
    Declared(usize),
    /// No `Content-Length`: the body ends when the connection closes.
    Unstated,
}

/// What the server answers.
pub struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    delay: Option<Duration>,
    length: Length,
    /// Write the body in pieces of this many bytes, pausing between them.
    trickle: Option<(usize, Duration)>,
    /// After this many bytes of the body, pause once.
    stall: Option<(usize, Duration)>,
}

impl Reply {
    pub fn json(value: serde_json::Value) -> Self {
        Reply::raw(200, &serde_json::to_string(&value).unwrap())
    }

    /// A hand-written JSON fixture, sent verbatim.
    pub fn fixture(text: &str) -> Self {
        Reply::raw(200, text)
    }

    pub fn raw(status: u16, body: &str) -> Self {
        Reply {
            status,
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: body.as_bytes().to_vec(),
            delay: None,
            length: Length::Exact,
            trickle: None,
            stall: None,
        }
    }

    /// v0.3: a binary body (an archive the CDN serves), status 200.
    pub fn bytes(body: Vec<u8>) -> Self {
        Reply {
            status: 200,
            headers: vec![("Content-Type".into(), "application/octet-stream".into())],
            body,
            delay: None,
            length: Length::Exact,
            trickle: None,
            stall: None,
        }
    }

    /// Claim `Content-Length: length` whatever the body holds.
    pub fn declaring(mut self, length: usize) -> Self {
        self.length = Length::Declared(length);
        self
    }

    /// Send no `Content-Length`: the body ends when the connection closes.
    pub fn unstated_length(mut self) -> Self {
        self.length = Length::Unstated;
        self
    }

    /// Write the body `piece` bytes at a time with `gap` between pieces.
    pub fn trickle(mut self, piece: usize, gap: Duration) -> Self {
        self.trickle = Some((piece.max(1), gap));
        self
    }

    /// After `after` bytes of the body, go quiet for `pause`.
    pub fn stall_after(mut self, after: usize, pause: Duration) -> Self {
        self.stall = Some((after, pause));
        self
    }

    pub fn status(code: u16) -> Self {
        Reply::raw(code, "")
    }

    pub fn with_status(mut self, code: u16) -> Self {
        self.status = code;
        self
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Answer only after `delay` (a slow Nexus).
    pub fn after(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }
}

pub struct TestServer {
    addr: SocketAddr,
    requests: Arc<Mutex<Vec<Recorded>>>,
    /// Requests that did not name the Riftstone, described.
    violations: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl TestServer {
    pub fn start(handler: impl Fn(&Recorded) -> Reply + Send + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
        let addr = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let violations = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let requests = Arc::clone(&requests);
            let violations = Arc::clone(&violations);
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    if let Ok(stream) = stream {
                        serve(stream, &handler, &requests, &violations);
                    }
                }
            })
        };
        TestServer {
            addr,
            requests,
            violations,
            stop,
            thread: Some(thread),
        }
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Hand over the identifying-header violations recorded so far (and
    /// forget them, so the drop check passes). Only for the test that proves
    /// the check bites.
    pub fn take_violations(&self) -> Vec<String> {
        std::mem::take(&mut *self.violations.lock().unwrap())
    }

    pub fn graphql_url(&self) -> String {
        format!("http://{}/v2/graphql", self.addr)
    }

    /// An address on this server (v0.3: a CDN link).
    pub fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.addr)
    }

    pub fn v1_base(&self) -> String {
        format!("http://{}/v1", self.addr)
    }

    /// A client aimed at this server.
    pub fn client(&self) -> NexusClient {
        NexusClient::new("0.1.0").with_endpoints(&self.graphql_url(), &self.v1_base())
    }

    pub fn requests(&self) -> Vec<Recorded> {
        self.requests.lock().unwrap().clone()
    }

    pub fn request_count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr); // wake the accept loop
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        if !std::thread::panicking() {
            let violations = self.violations.lock().unwrap();
            assert!(
                violations.is_empty(),
                "requests reached the stand-in without naming the Riftstone: {violations:#?}"
            );
        }
    }
}

/// Why a request fails Nexus's acceptable-use policy, if it does. The version
/// is not pinned here (tests may build clients with any version); the
/// `User-Agent` must announce the same one.
fn identity_violation(request: &Recorded) -> Option<String> {
    let mut wrong = Vec::new();
    let name = request.header("Application-Name");
    if name != Some("Riftstone") {
        wrong.push(format!("Application-Name is {name:?}"));
    }
    let version = request.header("Application-Version").filter(|version| {
        !version.is_empty()
            && version
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+' | '_'))
    });
    let agent = request.header("User-Agent");
    match version {
        None => wrong.push(format!(
            "Application-Version is {:?}",
            request.header("Application-Version")
        )),
        Some(version) => {
            let expected = format!("Riftstone/{version} (");
            if !agent.is_some_and(|agent| agent.starts_with(&expected)) {
                wrong.push(format!("User-Agent is {agent:?}, not {expected:?}…"));
            }
        }
    }
    (!wrong.is_empty())
        .then(|| format!("{} {}: {}", request.method, request.path, wrong.join("; ")))
}

/// A port nothing listens on.
pub fn dead_address() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    format!("http://{addr}")
}

fn serve(
    stream: TcpStream,
    handler: &dyn Fn(&Recorded) -> Reply,
    requests: &Mutex<Vec<Recorded>>,
    violations: &Mutex<Vec<String>>,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let Ok(read_half) = stream.try_clone() else {
        return;
    };
    let Some(request) = read_request(&mut BufReader::new(read_half)) else {
        return; // the shutdown wake-up, or a client that hung up
    };
    if let Some(violation) = identity_violation(&request) {
        violations.lock().unwrap().push(violation);
    }
    requests.lock().unwrap().push(request.clone());
    let reply = handler(&request);
    if let Some(delay) = reply.delay {
        std::thread::sleep(delay);
    }
    write_reply(stream, &reply);
}

fn read_request(reader: &mut impl BufRead) -> Option<Recorded> {
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();
    let mut headers = Vec::new();
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).ok()?;
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            headers.push((name.trim().to_string(), value.trim().to_string()));
        }
    }
    let length = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    Some(Recorded {
        method,
        path,
        headers,
        body,
    })
}

fn write_reply(mut stream: TcpStream, reply: &Reply) {
    let reason = match reply.status {
        200 => "OK",
        201 => "Created",
        302 => "Found",
        307 => "Temporary Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        410 => "Gone",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        _ => "Status",
    };
    let mut head = format!("HTTP/1.1 {} {reason}\r\n", reply.status);
    for (name, value) in &reply.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    match reply.length {
        Length::Exact => head.push_str(&format!("Content-Length: {}\r\n", reply.body.len())),
        Length::Declared(length) => head.push_str(&format!("Content-Length: {length}\r\n")),
        Length::Unstated => {}
    }
    head.push_str("Connection: close\r\n\r\n");
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }
    let _ = stream.flush();

    // The body: whole, in pieces with pauses, or with one stall. A client
    // that gave up closes the connection, and the next write fails: stop.
    let length = reply.body.len();
    let piece = reply.trickle.map_or(length.max(1), |(piece, _)| piece);
    let mut written = 0;
    let mut stalled = false;
    while written < length {
        if let Some((after, pause)) = reply.stall {
            if !stalled && written >= after {
                std::thread::sleep(pause);
                stalled = true;
            }
        }
        let mut end = (written + piece).min(length);
        if let Some((after, _)) = reply.stall {
            if !stalled && end > after {
                end = after;
            }
        }
        if stream.write_all(&reply.body[written..end]).is_err() {
            return;
        }
        let _ = stream.flush();
        written = end;
        if let Some((_, gap)) = reply.trickle {
            if written < length {
                std::thread::sleep(gap);
            }
        }
    }
}
