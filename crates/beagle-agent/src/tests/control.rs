//! Tests for `control`: the server + client over a socket pair and a bound
//! socket file.
#![allow(clippy::expect_used)] // panicking is the correct failure mode in tests

use std::io::{BufRead, BufReader, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::protocol::{decode_line, AgentStatus, DaemonStatus};

use super::*;

/// A handler that records the requests it handled and returns canned responses.
struct FakeHandler {
    calls: Arc<Mutex<Vec<String>>>,
}

fn sample_status() -> DaemonStatus {
    DaemonStatus {
        version: "0.1.0".to_string(),
        agents: vec![AgentStatus {
            id: "a".to_string(),
            enabled: true,
            running: true,
            last_tick: None,
            active_sessions: 0,
            last_results: Vec::new(),
        }],
    }
}

impl Handler for FakeHandler {
    fn handle(&self, request: Request) -> Response {
        let mut calls = self.calls.lock().expect("calls");
        match request {
            Request::GetStatus => Response::Status {
                status: sample_status(),
            },
            Request::ListAgents => Response::Agents {
                agents: sample_status().agents,
            },
            Request::StartAgent { id } => {
                calls.push(format!("start:{id}"));
                Response::Ok
            }
            Request::StopAgent { id } => {
                calls.push(format!("stop:{id}"));
                Response::Ok
            }
            Request::ReloadConfig | Request::Subscribe => Response::Ok,
        }
    }

    fn snapshot(&self) -> DaemonStatus {
        sample_status()
    }
}

/// Spawns a server connection thread over a socket pair, returning the client
/// end and the shared call log.
fn connected() -> (UnixStream, Arc<Mutex<Vec<String>>>) {
    let (client, server) = UnixStream::pair().expect("pair");
    let calls = Arc::new(Mutex::new(Vec::new()));
    let handler = FakeHandler {
        calls: Arc::clone(&calls),
    };
    thread::spawn(move || {
        let _ = serve_connection(server, &handler, Duration::from_millis(20));
    });
    (client, calls)
}

#[test]
fn request_response_round_trip() {
    let (client, calls) = connected();
    let mut client = Client::from_stream(client).expect("client");

    assert_eq!(
        client.request(&Request::GetStatus).expect("get-status"),
        Response::Status {
            status: sample_status()
        }
    );
    assert_eq!(
        client
            .request(&Request::StartAgent {
                id: "a".to_string()
            })
            .expect("start"),
        Response::Ok
    );
    assert_eq!(
        client
            .request(&Request::StopAgent {
                id: "a".to_string()
            })
            .expect("stop"),
        Response::Ok
    );
    assert_eq!(&*calls.lock().expect("calls"), &["start:a", "stop:a"]);
}

#[test]
fn malformed_frame_is_reported_and_survives() {
    let (client, _calls) = connected();
    let mut writer = client.try_clone().expect("clone");
    let mut reader = BufReader::new(client);

    // A garbage line gets an Error response...
    writer.write_all(b"this is not json\n").expect("write");
    writer.flush().expect("flush");
    let mut line = String::new();
    reader.read_line(&mut line).expect("read");
    assert!(matches!(
        decode_line::<Response>(&line).expect("decode"),
        Response::Error { .. }
    ));

    // ...and the connection still works afterward.
    writer
        .write_all(b"{\"type\":\"get-status\"}\n")
        .expect("write");
    writer.flush().expect("flush");
    let mut line2 = String::new();
    reader.read_line(&mut line2).expect("read");
    assert!(matches!(
        decode_line::<Response>(&line2).expect("decode"),
        Response::Status { .. }
    ));
}

#[test]
fn subscribe_streams_updates() {
    let (client, _calls) = connected();
    let mut client = Client::from_stream(client).expect("client");

    client.send(&Request::Subscribe).expect("subscribe");
    // The first update arrives immediately, a second after the interval.
    assert!(matches!(
        client.read_response().expect("first"),
        Response::Update { .. }
    ));
    assert!(matches!(
        client.read_response().expect("second"),
        Response::Update { .. }
    ));
}

#[test]
fn bind_serves_over_a_real_socket_and_clears_stale() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("agentd.sock");

    // Binding twice must succeed: the second call clears the stale socket.
    let first = bind(&path).expect("first bind");
    drop(first);
    let listener = bind(&path).expect("second bind clears stale");

    let calls = Arc::new(Mutex::new(Vec::new()));
    let server = Server::new(Arc::new(FakeHandler {
        calls: Arc::clone(&calls),
    }))
    .with_subscribe_interval(Duration::from_millis(20));
    thread::spawn(move || {
        let _ = server.serve(&listener);
    });

    let mut client = Client::connect(&path).expect("connect");
    assert_eq!(
        client.request(&Request::GetStatus).expect("status"),
        Response::Status {
            status: sample_status()
        }
    );
}
