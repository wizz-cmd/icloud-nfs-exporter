pub mod ipc_client;
pub mod ipc_protocol;
pub mod path_utils;

pub const VERSION: &str = "0.1.0";

pub use ipc_client::IpcClient;
pub use ipc_protocol::{FileState, Request, Response};
