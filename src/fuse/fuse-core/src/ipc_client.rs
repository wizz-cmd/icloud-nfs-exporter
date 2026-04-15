use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use crate::ipc_protocol::{self, FileState, Request, Response};

/// Client for the hydration daemon IPC socket.
pub struct IpcClient {
    socket_path: String,
    timeout: Duration,
}

#[derive(Debug)]
pub enum IpcError {
    Connect(io::Error),
    Io(io::Error),
    Encode(serde_json::Error),
    Decode(serde_json::Error),
    ResponseTooLarge(u32),
    UnexpectedResponse,
    HydrationFailed(String),
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connect(e) => write!(f, "connect: {e}"),
            Self::Io(e) => write!(f, "io: {e}"),
            Self::Encode(e) => write!(f, "encode: {e}"),
            Self::Decode(e) => write!(f, "decode: {e}"),
            Self::ResponseTooLarge(n) => write!(f, "response too large: {n} bytes"),
            Self::UnexpectedResponse => write!(f, "unexpected response type"),
            Self::HydrationFailed(e) => write!(f, "hydration failed: {e}"),
        }
    }
}

impl std::error::Error for IpcError {}

impl IpcClient {
    pub fn new(socket_path: impl Into<String>) -> Self {
        Self {
            socket_path: socket_path.into(),
            timeout: Duration::from_secs(300),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn socket_path(&self) -> &str {
        &self.socket_path
    }

    /// Send a request and return the response.
    pub fn send(&self, request: &Request) -> Result<Response, IpcError> {
        let mut stream =
            UnixStream::connect(&self.socket_path).map_err(IpcError::Connect)?;
        stream
            .set_read_timeout(Some(self.timeout))
            .map_err(IpcError::Io)?;
        stream
            .set_write_timeout(Some(Duration::from_secs(10)))
            .map_err(IpcError::Io)?;

        // Encode and send
        let wire = ipc_protocol::wire_encode(request).map_err(IpcError::Encode)?;
        stream.write_all(&wire).map_err(IpcError::Io)?;

        // Read response length
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).map_err(IpcError::Io)?;
        let resp_len = u32::from_be_bytes(len_buf);
        if resp_len > 1_048_576 {
            return Err(IpcError::ResponseTooLarge(resp_len));
        }

        // Read response payload
        let mut resp_buf = vec![0u8; resp_len as usize];
        stream.read_exact(&mut resp_buf).map_err(IpcError::Io)?;

        ipc_protocol::wire_decode(&resp_buf).map_err(IpcError::Decode)
    }

    /// Health check — returns Ok(()) if the daemon responds with Pong.
    pub fn ping(&self) -> Result<(), IpcError> {
        match self.send(&Request::Ping)? {
            Response::Pong => Ok(()),
            _ => Err(IpcError::UnexpectedResponse),
        }
    }

    /// Query the hydration state of a file.
    pub fn query_state(&self, path: &str) -> Result<FileState, IpcError> {
        match self.send(&Request::QueryState {
            path: path.to_string(),
        })? {
            Response::State { state, .. } => Ok(state),
            _ => Err(IpcError::UnexpectedResponse),
        }
    }

    /// Request hydration of an evicted file (blocks until complete).
    pub fn hydrate(&self, path: &str) -> Result<(), IpcError> {
        match self.send(&Request::Hydrate {
            path: path.to_string(),
        })? {
            Response::HydrationResult {
                success: true, ..
            } => Ok(()),
            Response::HydrationResult { error, .. } => {
                Err(IpcError::HydrationFailed(error.unwrap_or_default()))
            }
            _ => Err(IpcError::UnexpectedResponse),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_creation() {
        let client = IpcClient::new("/tmp/test.sock");
        assert_eq!(client.socket_path(), "/tmp/test.sock");
    }

    #[test]
    fn client_with_timeout() {
        let client =
            IpcClient::new("/tmp/test.sock").with_timeout(Duration::from_secs(60));
        assert_eq!(client.timeout, Duration::from_secs(60));
    }

    #[test]
    fn connect_to_missing_socket_fails() {
        let client = IpcClient::new("/tmp/nonexistent-socket-12345.sock");
        let result = client.ping();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), IpcError::Connect(_)));
    }
}
