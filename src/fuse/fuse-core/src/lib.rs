//! Core library for the icloud-nfs-exporter FUSE layer.
//!
//! This crate provides the shared types, IPC protocol, and utility functions
//! used by the FUSE driver to communicate with the Swift hydration daemon and
//! to handle iCloud stub/placeholder files on disk.
//!
//! # Modules
//!
//! - [`ipc_protocol`] -- Wire format, request/response enums, and serialization
//!   for the Unix-domain-socket IPC between the FUSE driver and the hydration daemon.
//! - [`ipc_client`] -- A blocking client that connects to the hydration daemon,
//!   sends requests, and reads responses.
//! - [`path_utils`] -- Helpers for detecting and converting iCloud stub filenames
//!   (`.OriginalName.icloud` format).
//!
//! # Quick start
//!
//! ```rust
//! use fuse_core::{IpcClient, FileState, Request, Response};
//!
//! // Create a client (connection is deferred until send())
//! let client = IpcClient::new("/tmp/hydrated.sock");
//! assert_eq!(client.socket_path(), "/tmp/hydrated.sock");
//! ```

/// IPC client for communicating with the hydration daemon.
pub mod ipc_client;

/// Wire-format types and serialization for the FUSE-to-daemon IPC protocol.
pub mod ipc_protocol;

/// Utilities for detecting and converting iCloud stub filenames.
pub mod path_utils;

/// Crate version, following [Semantic Versioning](https://semver.org/).
///
/// This is kept in sync with the version in `Cargo.toml`.
pub const VERSION: &str = "0.2.0";

pub use ipc_client::IpcClient;
pub use ipc_protocol::{FileState, Request, Response};
