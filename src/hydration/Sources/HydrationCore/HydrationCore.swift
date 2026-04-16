/// Top-level namespace for the iCloud hydration library.
///
/// `HydrationCore` is a caseless enum used purely as a namespace for library-wide
/// constants. It provides the library version and default configuration values
/// shared across the hydration daemon and its clients.
public enum HydrationCore {
    /// Semantic version string for the HydrationCore library.
    public static let version = "0.2.0"

    /// Default filesystem path for the IPC Unix domain socket.
    ///
    /// Both ``IPCServer`` and ``IPCClient`` use this path when no explicit
    /// socket path is provided. The socket is created by the hydration daemon
    /// and connected to by the FUSE driver.
    public static let defaultSocketPath = "/tmp/icloud-nfs-exporter.sock"
}
