use serde::{Deserialize, Serialize};

/// File hydration state (mirrors Swift `FileState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileState {
    Unknown,
    Evicted,
    Downloading,
    Local,
    Error,
}

/// Request sent from the FUSE driver to the hydration daemon.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Request {
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "query_state")]
    QueryState { path: String },
    #[serde(rename = "hydrate")]
    Hydrate { path: String },
}

/// Response received from the hydration daemon.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Response {
    #[serde(rename = "pong")]
    Pong,
    #[serde(rename = "state")]
    State { path: String, state: FileState },
    #[serde(rename = "hydration_result")]
    HydrationResult {
        path: String,
        success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}

/// Encode a value to the IPC wire format (4-byte big-endian length + JSON).
pub fn wire_encode<T: Serialize>(value: &T) -> Result<Vec<u8>, serde_json::Error> {
    let json = serde_json::to_vec(value)?;
    let len = (json.len() as u32).to_be_bytes();
    let mut buf = Vec::with_capacity(4 + json.len());
    buf.extend_from_slice(&len);
    buf.extend_from_slice(&json);
    Ok(buf)
}

/// Read the 4-byte big-endian length prefix from a buffer.
pub fn wire_read_length(buf: &[u8]) -> Option<u32> {
    if buf.len() < 4 {
        return None;
    }
    Some(u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]))
}

/// Decode a JSON payload.
pub fn wire_decode<T: for<'de> Deserialize<'de>>(data: &[u8]) -> Result<T, serde_json::Error> {
    serde_json::from_slice(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_round_trip() {
        let wire = wire_encode(&Request::Ping).unwrap();
        let len = wire_read_length(&wire).unwrap() as usize;
        assert_eq!(len, wire.len() - 4);
        let decoded: Request = wire_decode(&wire[4..]).unwrap();
        assert!(matches!(decoded, Request::Ping));
    }

    #[test]
    fn query_state_round_trip() {
        let req = Request::QueryState {
            path: "/tmp/test.txt".into(),
        };
        let wire = wire_encode(&req).unwrap();
        let decoded: Request = wire_decode(&wire[4..]).unwrap();
        match decoded {
            Request::QueryState { path } => assert_eq!(path, "/tmp/test.txt"),
            _ => panic!("expected QueryState"),
        }
    }

    #[test]
    fn hydrate_round_trip() {
        let req = Request::Hydrate {
            path: "/Users/test/doc.pdf".into(),
        };
        let wire = wire_encode(&req).unwrap();
        let decoded: Request = wire_decode(&wire[4..]).unwrap();
        match decoded {
            Request::Hydrate { path } => assert_eq!(path, "/Users/test/doc.pdf"),
            _ => panic!("expected Hydrate"),
        }
    }

    #[test]
    fn pong_round_trip() {
        let wire = wire_encode(&Response::Pong).unwrap();
        let decoded: Response = wire_decode(&wire[4..]).unwrap();
        assert!(matches!(decoded, Response::Pong));
    }

    #[test]
    fn state_response_round_trip() {
        let resp = Response::State {
            path: "/a/b".into(),
            state: FileState::Evicted,
        };
        let wire = wire_encode(&resp).unwrap();
        let decoded: Response = wire_decode(&wire[4..]).unwrap();
        match decoded {
            Response::State { path, state } => {
                assert_eq!(path, "/a/b");
                assert_eq!(state, FileState::Evicted);
            }
            _ => panic!("expected State"),
        }
    }

    #[test]
    fn hydration_result_round_trip() {
        let resp = Response::HydrationResult {
            path: "/x".into(),
            success: false,
            error: Some("timeout".into()),
        };
        let wire = wire_encode(&resp).unwrap();
        let decoded: Response = wire_decode(&wire[4..]).unwrap();
        match decoded {
            Response::HydrationResult {
                path,
                success,
                error,
            } => {
                assert_eq!(path, "/x");
                assert!(!success);
                assert_eq!(error.as_deref(), Some("timeout"));
            }
            _ => panic!("expected HydrationResult"),
        }
    }

    #[test]
    fn hydration_result_no_error() {
        let resp = Response::HydrationResult {
            path: "/ok".into(),
            success: true,
            error: None,
        };
        let wire = wire_encode(&resp).unwrap();
        let decoded: Response = wire_decode(&wire[4..]).unwrap();
        match decoded {
            Response::HydrationResult {
                success, error, ..
            } => {
                assert!(success);
                assert!(error.is_none());
            }
            _ => panic!("expected HydrationResult"),
        }
    }

    #[test]
    fn wire_read_length_too_short() {
        assert!(wire_read_length(&[0, 1]).is_none());
        assert!(wire_read_length(&[]).is_none());
    }

    #[test]
    fn file_state_serialization() {
        let json = serde_json::to_string(&FileState::Evicted).unwrap();
        assert_eq!(json, "\"evicted\"");

        let decoded: FileState = serde_json::from_str("\"downloading\"").unwrap();
        assert_eq!(decoded, FileState::Downloading);
    }

    #[test]
    fn cross_compat_with_swift_daemon() {
        // Verify JSON shape matches what the Swift daemon expects/produces
        let req_json = serde_json::to_string(&Request::Hydrate {
            path: "/test".into(),
        })
        .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&req_json).unwrap();
        assert_eq!(parsed["type"], "hydrate");
        assert_eq!(parsed["path"], "/test");

        // Verify we can decode a response shaped like the Swift daemon sends
        let swift_resp = r#"{"type":"state","path":"/test","state":"local"}"#;
        let resp: Response = serde_json::from_str(swift_resp).unwrap();
        match resp {
            Response::State { path, state } => {
                assert_eq!(path, "/test");
                assert_eq!(state, FileState::Local);
            }
            _ => panic!("expected State"),
        }
    }
}
