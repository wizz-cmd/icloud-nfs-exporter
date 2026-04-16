//! Blocking IPC client for the hydration daemon.
//!
//! [`IpcClient`] connects to the Swift hydration daemon over a Unix domain
//! socket, sends [`Request`]s, and reads [`Response`]s using the length-prefixed
//! JSON wire format defined in [`crate::ipc_protocol`].
//!
//! Each call to [`IpcClient::send`] opens a new connection, sends exactly one
//! request, reads the response, and closes the connection. This makes the
//! client safe to use from multiple threads (each call is independent).
//!
//! # Examples
//!
//! ```rust,no_run
//! use fuse_core::IpcClient;
//! use std::time::Duration;
//!
//! let client = IpcClient::new("/var/run/hydrated.sock")
//!     .with_timeout(Duration::from_secs(60));
//!
//! // Health check
//! client.ping().expect("daemon not responding");
//!
//! // Query a file's state
//! let state = client.query_state("/Users/me/iCloud/doc.pdf").unwrap();
//! println!("file state: {:?}", state);
//!
//! // Hydrate (download) an evicted file
//! client.hydrate("/Users/me/iCloud/doc.pdf").unwrap();
//! ```

use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use crate::ipc_protocol::{self, FileState, Request, Response};

/// Blocking client for communicating with the hydration daemon over a Unix domain socket.
///
/// The client is configured once (socket path + timeout) and can then be used
/// to send individual requests. Each call opens a fresh connection, so there is
/// no persistent socket state to manage.
///
/// # Examples
///
/// ```rust
/// use fuse_core::IpcClient;
///
/// let client = IpcClient::new("/tmp/hydrated.sock");
/// assert_eq!(client.socket_path(), "/tmp/hydrated.sock");
/// ```
pub struct IpcClient {
    socket_path: String,
    timeout: Duration,
}

/// Errors that can occur during IPC communication with the hydration daemon.
///
/// Each variant wraps the underlying cause so that callers can distinguish
/// between connection failures, I/O errors, serialization issues, and
/// application-level errors.
#[derive(Debug)]
pub enum IpcError {
    /// Failed to connect to the Unix domain socket (daemon may not be running).
    Connect(io::Error),
    /// An I/O error occurred during read or write on an established connection.
    Io(io::Error),
    /// Failed to serialize the request to JSON.
    Encode(serde_json::Error),
    /// Failed to deserialize the response from JSON.
    Decode(serde_json::Error),
    /// The daemon reported a response larger than the 1 MiB safety limit.
    ResponseTooLarge(u32),
    /// The daemon returned a response type that does not match the request
    /// (e.g., a `Pong` in reply to a `QueryState`).
    UnexpectedResponse,
    /// The daemon reported that hydration failed, with an optional error message.
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
    /// Create a new client that will connect to the given Unix socket path.
    ///
    /// No connection is established until [`send`](Self::send) (or a
    /// convenience method like [`ping`](Self::ping)) is called.
    ///
    /// The default read timeout is 300 seconds (5 minutes), which accommodates
    /// large-file hydrations. Use [`with_timeout`](Self::with_timeout) to
    /// override.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fuse_core::IpcClient;
    ///
    /// let client = IpcClient::new("/var/run/hydrated.sock");
    /// assert_eq!(client.socket_path(), "/var/run/hydrated.sock");
    /// ```
    pub fn new(socket_path: impl Into<String>) -> Self {
        Self {
            socket_path: socket_path.into(),
            timeout: Duration::from_secs(300),
        }
    }

    /// Set the read timeout for responses from the daemon.
    ///
    /// This is a builder-style method that consumes and returns `self`.
    /// The timeout applies to the entire response read (length prefix + payload).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use fuse_core::IpcClient;
    /// use std::time::Duration;
    ///
    /// let client = IpcClient::new("/tmp/test.sock")
    ///     .with_timeout(Duration::from_secs(60));
    /// ```
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Return the Unix socket path this client is configured to connect to.
    pub fn socket_path(&self) -> &str {
        &self.socket_path
    }

    /// Send a request to the daemon and return its response.
    ///
    /// Opens a new Unix domain socket connection, writes the wire-encoded
    /// request, reads the length-prefixed response, and deserializes it.
    /// The connection is closed when this method returns.
    ///
    /// Responses larger than 1 MiB are rejected with [`IpcError::ResponseTooLarge`]
    /// as a safety measure.
    ///
    /// # Errors
    ///
    /// - [`IpcError::Connect`] -- the daemon socket is unreachable.
    /// - [`IpcError::Io`] -- read/write error on the connection.
    /// - [`IpcError::Encode`] -- the request could not be serialized.
    /// - [`IpcError::Decode`] -- the response could not be deserialized.
    /// - [`IpcError::ResponseTooLarge`] -- the response exceeds the 1 MiB limit.
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

    /// Send a ping and verify the daemon replies with a pong.
    ///
    /// This is the simplest health check: it confirms the daemon is running,
    /// listening on the socket, and able to process requests.
    ///
    /// # Errors
    ///
    /// - Any [`IpcError`] variant from [`send`](Self::send).
    /// - [`IpcError::UnexpectedResponse`] if the daemon replies with something
    ///   other than [`Response::Pong`].
    pub fn ping(&self) -> Result<(), IpcError> {
        match self.send(&Request::Ping)? {
            Response::Pong => Ok(()),
            _ => Err(IpcError::UnexpectedResponse),
        }
    }

    /// Query the hydration state of a file in iCloud Drive.
    ///
    /// Returns the current [`FileState`] for the file at `path`. This is a
    /// non-mutating operation -- it does not trigger a download.
    ///
    /// # Errors
    ///
    /// - Any [`IpcError`] variant from [`send`](Self::send).
    /// - [`IpcError::UnexpectedResponse`] if the daemon does not reply with
    ///   [`Response::State`].
    pub fn query_state(&self, path: &str) -> Result<FileState, IpcError> {
        match self.send(&Request::QueryState {
            path: path.to_string(),
        })? {
            Response::State { state, .. } => Ok(state),
            _ => Err(IpcError::UnexpectedResponse),
        }
    }

    /// Request hydration of an evicted file and block until it completes.
    ///
    /// If the file is already local, the daemon typically returns success
    /// immediately. For evicted files, this call blocks until the download
    /// finishes or fails (subject to the client's read timeout).
    ///
    /// # Errors
    ///
    /// - Any [`IpcError`] variant from [`send`](Self::send).
    /// - [`IpcError::HydrationFailed`] if the daemon reports a download failure.
    /// - [`IpcError::UnexpectedResponse`] if the daemon does not reply with
    ///   [`Response::HydrationResult`].
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
