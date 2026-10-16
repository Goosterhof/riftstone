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
}

/// What the server answers.
pub struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    delay: Option<Duration>,
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
        }
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
        400 => "Bad Request",
        401 => "Unauthorized",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        _ => "Status",
    };
    let mut head = format!("HTTP/1.1 {} {reason}\r\n", reply.status);
    for (name, value) in &reply.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        reply.body.len()
    ));
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&reply.body);
    let _ = stream.flush();
}
