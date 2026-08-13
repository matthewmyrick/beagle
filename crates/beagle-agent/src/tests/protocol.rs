//! Tests for `protocol`: round-trip serialization and framing.
#![allow(clippy::expect_used)] // panicking is the correct failure mode in tests

use super::*;

fn sample_status() -> DaemonStatus {
    DaemonStatus {
        version: "0.1.0".to_string(),
        agents: vec![AgentStatus {
            id: "rca-remediation".to_string(),
            enabled: true,
            running: true,
            last_tick: Some("2026-08-13T12:00:00Z".to_string()),
            active_sessions: 1,
            last_results: vec!["2026-01-01-alpha -> Published".to_string()],
        }],
    }
}

#[test]
fn requests_round_trip() {
    let requests = [
        Request::GetStatus,
        Request::ListAgents,
        Request::StartAgent {
            id: "a".to_string(),
        },
        Request::StopAgent {
            id: "a".to_string(),
        },
        Request::ReloadConfig,
        Request::Subscribe,
    ];
    for request in requests {
        let line = encode_line(&request).expect("encode");
        assert!(line.ends_with('\n'));
        let back: Request = decode_line(&line).expect("decode");
        assert_eq!(back, request);
    }
}

#[test]
fn responses_round_trip() {
    let responses = [
        Response::Status {
            status: sample_status(),
        },
        Response::Agents {
            agents: sample_status().agents,
        },
        Response::Ok,
        Response::Error {
            message: "nope".to_string(),
        },
        Response::Update {
            status: sample_status(),
        },
    ];
    for response in responses {
        let line = encode_line(&response).expect("encode");
        let back: Response = decode_line(&line).expect("decode");
        assert_eq!(back, response);
    }
}

#[test]
fn request_tag_is_kebab_case() {
    let line = encode_line(&Request::GetStatus).expect("encode");
    assert_eq!(line.trim_end(), r#"{"type":"get-status"}"#);
    let line = encode_line(&Request::StartAgent {
        id: "x".to_string(),
    })
    .expect("encode");
    assert_eq!(line.trim_end(), r#"{"type":"start-agent","id":"x"}"#);
}

#[test]
fn malformed_line_is_an_error_not_a_panic() {
    assert!(decode_line::<Request>("not json").is_err());
    assert!(decode_line::<Request>(r#"{"type":"unknown-verb"}"#).is_err());
    assert!(decode_line::<Request>("").is_err());
}
