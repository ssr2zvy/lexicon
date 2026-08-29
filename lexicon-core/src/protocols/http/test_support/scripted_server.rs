//! HTTP-03 scripted shadow server.
//!
//! A shadow HTTP/1.1 server that returns a deterministic, scripted sequence
//! of responses per request. Replaces the inline `ensure_recording_test_server`
//! helper for tests that need to enumerate attempt counts, redirect chains,
//! retry cycles, body byte counts, and SHA-256 verification.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// One response step the scripted server will emit. The server consumes
/// steps in order; attempts past the end of the list raise an error inside
/// the loop and the connection is closed.
#[derive(Clone, Debug)]
pub(crate) enum ScriptedStep {
    /// Respond with the given status and a literal body.
    Respond { status: u16, body: Vec<u8> },
    /// Respond with a 302 redirect to the absolute Location provided.
    Redirect { location: String },
    /// Respond as if the server sent `announced_len` bytes of body, but
    /// only writes `truncate_to` bytes before closing the connection.
    Truncate {
        status: u16,
        announced_len: usize,
        truncate_to: usize,
    },
    /// Respond, but after writing `close_after_bytes` of body, abruptly
    /// close the connection (no FIN order).
    CloseAfter {
        status: u16,
        body_prefix: Vec<u8>,
        close_after_bytes: usize,
    },
}

/// Observations surfaced to the test about a single request the shadow
/// server received.
#[derive(Clone, Debug)]
pub(crate) struct ReceivedRequest {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
}

#[derive(Default)]
struct ServerState {
    request_count: AtomicU32,
    last_request: std::sync::Mutex<Option<ReceivedRequest>>,
    attempt_counter: AtomicU32,
    redirect_counter: AtomicU32,
    truncated_observed: AtomicU64,
}

/// Handle to a running scripted server. The Drop impl does not block; tests
/// must drop the handle when they no longer need the URL.
pub(crate) struct ScriptedServerHandle {
    pub base_url: String,
    state: Arc<ServerState>,
    _join: Option<thread::JoinHandle<()>>,
}

impl ScriptedServerHandle {
    /// Number of TCP-level requests the shadow server has accepted since
    /// startup. Use this to assert "exactly three distinct attempts" or
    /// "exactly two levels of redirect".
    pub fn request_count(&self) -> u32 {
        self.state.request_count.load(Ordering::SeqCst)
    }

    /// Snapshot of the last request the server accepted, useful for
    /// asserting that mandatory sensitive headers are absent in redirects.
    pub fn last_request(&self) -> Option<ReceivedRequest> {
        self.state.last_request.lock().ok().and_then(|guard| guard.clone())
    }

    pub fn attempt_counter(&self) -> u32 {
        self.state.attempt_counter.load(Ordering::SeqCst)
    }

    pub fn redirect_counter(&self) -> u32 {
        self.state.redirect_counter.load(Ordering::SeqCst)
    }

    pub fn truncated_observed(&self) -> u64 {
        self.state.truncated_observed.load(Ordering::SeqCst)
    }
}

/// Spawn a shadow HTTP server that emits `steps[step_index]` per request and
/// increments the index each time a request arrives.
pub(crate) fn start_scripted_server(steps: Vec<ScriptedStep>) -> ScriptedServerHandle {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind 127.0.0.1:0");
    let port = listener.local_addr().unwrap().port();
    let state = Arc::new(ServerState::default());
    let step_state = state.clone();
    let steps_arc = Arc::new(steps);
    let steps_for_thread = steps_arc.clone();

    let join = thread::spawn(move || run_listener(listener, step_state, steps_for_thread));

    ScriptedServerHandle {
        base_url: format!("http://127.0.0.1:{port}"),
        state,
        _join: Some(join),
    }
}

fn run_listener(
    listener: TcpListener,
    state: Arc<ServerState>,
    steps: Arc<Vec<ScriptedStep>>,
) {
    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };
        state.request_count.fetch_add(1, Ordering::SeqCst);
        let received = parse_request(&mut stream);

        if let Ok(mut guard) = state.last_request.lock() {
            *guard = Some(received);
        }

        let idx = state.attempt_counter.fetch_add(1, Ordering::SeqCst) as usize;
        if idx >= steps.len() {
            let _ = stream.shutdown(std::net::Shutdown::Both);
            continue;
        }
        match &steps[idx] {
            ScriptedStep::Respond { status, body } => {
                let response = format!(
                    "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.write_all(body);
                let _ = stream.flush();
            }
            ScriptedStep::Redirect { location } => {
                let response = format!(
                    "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                let _ = stream.write_all(response.as_bytes());
                state.redirect_counter.fetch_add(1, Ordering::SeqCst);
            }
            ScriptedStep::Truncate {
                status,
                announced_len,
                truncate_to,
            } => {
                let response = format!(
                    "HTTP/1.1 {status} OK\r\nContent-Length: {announced_len}\r\nConnection: close\r\n\r\n"
                );
                let _ = stream.write_all(response.as_bytes());
                let payload = vec![b'X'; truncate_to.min(*announced_len)];
                let _ = stream.write_all(&payload);
                let _ = stream.flush();
                state.truncated_observed.fetch_add(1, Ordering::SeqCst);
            }
            ScriptedStep::CloseAfter {
                status,
                body_prefix,
                close_after_bytes,
            } => {
                let response = format!(
                    "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body_prefix.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let take = close_after_bytes.min(body_prefix.len());
                let _ = stream.write_all(&body_prefix[..take]);
                let _ = stream.flush();
                let _ = stream.shutdown(std::net::Shutdown::Both);
            }
        }
    }
}

fn parse_request(stream: &mut std::net::TcpStream) -> ReceivedRequest {
    let mut buf = [0u8; 4096];
    let mut total = vec![];
    let n = stream.read(&mut buf).unwrap_or(0);
    total.extend_from_slice(&buf[..n]);
    let text = String::from_utf8_lossy(&total).to_string();

    let mut lines = text.split("\r\n");
    let request_line = lines.next().unwrap_or("").to_owned();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_owned();
    let path = parts.next().unwrap_or("").to_owned();

    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_owned(), value.trim().to_owned()));
        }
    }

    ReceivedRequest {
        method,
        path,
        headers,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn scripted_server_starts_with_zero_attempts() {
        let steps = vec![ScriptedStep::Respond {
            status: 200,
            body: b"first".to_vec(),
        }];
        let handle = start_scripted_server(steps);
        assert!(handle.base_url.starts_with("http://127.0.0.1:"));
        assert_eq!(handle.request_count(), 0);
        assert_eq!(handle.attempt_counter(), 0);
        assert_eq!(handle.redirect_counter(), 0);
        assert_eq!(handle.truncated_observed(), 0);
        assert!(handle.last_request().is_none());
    }

    #[test]
    fn scripted_server_handles_drop_without_panic() {
        let steps = vec![ScriptedStep::Respond {
            status: 200,
            body: b"first".to_vec(),
        }];
        let handle = start_scripted_server(steps);
        drop(handle);
        std::thread::sleep(Duration::from_millis(10));
    }
}
