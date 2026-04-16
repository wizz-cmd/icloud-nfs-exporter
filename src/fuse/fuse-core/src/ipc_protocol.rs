//! IPC wire protocol for communication between the FUSE driver and the hydration daemon.
//!
//! Messages are exchanged over a Unix domain socket using a simple length-prefixed
//! JSON format: a 4-byte big-endian length prefix followed by a JSON payload.
//! This module defines the [`Request`] and [`Response`] enums, the [`FileState`]
//! enum, and the wire encoding/decoding functions.
//!
//! The JSON shapes are designed for cross-language compatibility with the Swift
//! hydration daemon (see `cross_compat_with_swift_daemon` test).
//!
//! # Wire format
//!
//! ```text
//! +---------+--------------------+
//! | 4 bytes |   N bytes          |
//! | (len N) |   (JSON payload)   |
//! +---------+--------------------+
//! ```
//!
//! # Examples
//!
//! ```rust
//! use fuse_core::ipc_protocol::{wire_encode, wire_read_length, wire_decode, Request, Response};
//!
//! // Encode a Ping request
//! let wire = wire_encode(&Request::Ping).unwrap();
//!
//! // Read the length prefix
//! let len = wire_read_length(&wire).unwrap() as usize;
//! assert_eq!(len, wire.len() - 4);
//!
//! // Decode the JSON payload back into a Request
//! let decoded: Request = wire_decode(&wire[4..]).unwrap();
//! assert!(matches!(decoded, Request::Ping));
//! ```

use serde::{Deserialize, Serialize};

/// Hydration state of a file in iCloud Drive.
///
/// Mirrors the Swift `FileState` enum in the hydration daemon. Serialized as a
/// lowercase string (e.g. `"evicted"`, `"local"`) for JSON compatibility.
///
/// # Examples
///
/// ```rust
/// use fuse_core::FileState;
///
/// // Deserialize from a JSON string (as sent by the Swift daemon)
/// let state: FileState = serde_json::from_str("\"evicted\"").unwrap();
/// assert_eq!(state, FileState::Evicted);
///
/// // Serialize back to JSON
/// let json = serde_json::to_string(&state).unwrap();
/// assert_eq!(json, "\"evicted\"");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileState {
    /// File state could not be determined.
    Unknown,
    /// File is evicted (only a stub/placeholder exists on disk).
    Evicted,
    /// File is currently being downloaded from iCloud.
    Downloading,
    /// File is fully materialized on local disk.
    Local,
    /// An error occurred while querying or downloading the file.
    Error,
}

/// Request sent from the FUSE driver to the hydration daemon.
///
/// Serialized as tagged JSON with a `"type"` discriminator field. For example,
/// `Request::Ping` serializes to `{"type":"ping"}` and
/// `Request::QueryState { path: "/a".into() }` serializes to
/// `{"type":"query_state","path":"/a"}`.
///
/// # Examples
///
/// ```rust
/// use fuse_core::Request;
///
/// let req = Request::Hydrate { path: "/tmp/file.pdf".into() };
/// let json = serde_json::to_string(&req).unwrap();
/// assert!(json.contains("\"type\":\"hydrate\""));
/// assert!(json.contains("\"path\":\"/tmp/file.pdf\""));
/// ```
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Request {
    /// Health-check ping; the daemon should reply with [`Response::Pong`].
    #[serde(rename = "ping")]
    Ping,
    /// Query the hydration state of the file at `path`.
    ///
    /// The daemon responds with [`Response::State`].
    #[serde(rename = "query_state")]
    QueryState {
        /// Absolute path to the file within iCloud Drive.
        path: String,
    },
    /// Request on-demand hydration (download) of the file at `path`.
    ///
    /// The daemon responds with [`Response::HydrationResult`] once the
    /// download completes or fails.
    #[serde(rename = "hydrate")]
    Hydrate {
        /// Absolute path to the evicted file to hydrate.
        path: String,
    },
}

/// Response received from the hydration daemon.
///
/// Serialized as tagged JSON with a `"type"` discriminator field, matching
/// the format produced by the Swift daemon.
///
/// # Examples
///
/// ```rust
/// use fuse_core::{Response, FileState};
///
/// // Deserialize a state response as the Swift daemon would send it
/// let json = r#"{"type":"state","path":"/test","state":"local"}"#;
/// let resp: Response = serde_json::from_str(json).unwrap();
/// assert!(matches!(resp, Response::State { state: FileState::Local, .. }));
/// ```
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Response {
    /// Reply to [`Request::Ping`], confirming the daemon is alive.
    #[serde(rename = "pong")]
    Pong,
    /// Current hydration state of a queried file.
    #[serde(rename = "state")]
    State {
        /// The path that was queried.
        path: String,
        /// Current hydration state of the file.
        state: FileState,
    },
    /// Result of a hydration (download) request.
    #[serde(rename = "hydration_result")]
    HydrationResult {
        /// The path that was requested for hydration.
        path: String,
        /// `true` if the file was successfully hydrated.
        success: bool,
        /// Human-readable error message, present only when `success` is `false`.
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}

/// Encode a value to the IPC wire format (4-byte big-endian length prefix + JSON).
///
/// The returned buffer contains the complete frame ready to write to the socket.
///
/// # Errors
///
/// Returns [`serde_json::Error`] if `value` cannot be serialized to JSON.
///
/// # Examples
///
/// ```rust
/// use fuse_core::ipc_protocol::{wire_encode, Request};
///
/// let wire = wire_encode(&Request::Ping).unwrap();
/// // First 4 bytes are the big-endian length of the JSON payload
/// assert!(wire.len() > 4);
/// ```
pub fn wire_encode<T: Serialize>(value: &T) -> Result<Vec<u8>, serde_json::Error> {
    let json = serde_json::to_vec(value)?;
    let len = (json.len() as u32).to_be_bytes();
    let mut buf = Vec::with_capacity(4 + json.len());
    buf.extend_from_slice(&len);
    buf.extend_from_slice(&json);
    Ok(buf)
}

/// Read the 4-byte big-endian length prefix from the beginning of a buffer.
///
/// Returns `None` if `buf` is shorter than 4 bytes.
///
/// # Examples
///
/// ```rust
/// use fuse_core::ipc_protocol::wire_read_length;
///
/// // A buffer starting with [0, 0, 0, 15] means a 15-byte payload follows
/// let buf = [0u8, 0, 0, 15, /* ...payload bytes... */];
/// assert_eq!(wire_read_length(&buf), Some(15));
///
/// // Too-short buffers return None
/// assert_eq!(wire_read_length(&[0, 1]), None);
/// ```
pub fn wire_read_length(buf: &[u8]) -> Option<u32> {
    if buf.len() < 4 {
        return None;
    }
    Some(u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]))
}

/// Decode a JSON byte slice into a value of type `T`.
///
/// Typically used to deserialize the payload portion of a wire frame
/// (the bytes after the 4-byte length prefix).
///
/// # Errors
///
/// Returns [`serde_json::Error`] if `data` is not valid JSON or does not
/// match the structure of `T`.
///
/// # Examples
///
/// ```rust
/// use fuse_core::ipc_protocol::{wire_decode, Response};
///
/// let json = br#"{"type":"pong"}"#;
/// let resp: Response = wire_decode(json).unwrap();
/// assert!(matches!(resp, Response::Pong));
/// ```
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
