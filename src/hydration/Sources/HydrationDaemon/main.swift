import Foundation
import HydrationCore

let version = HydrationCore.version
let args = CommandLine.arguments

// -- Parse arguments --

var watchPaths: [String] = []
var socketPath = HydrationCore.defaultSocketPath
var i = 1
while i < args.count {
    switch args[i] {
    case "--version", "-v":
        print("icloud-nfs-exporter hydration daemon v\(version)")
        exit(0)
    case "--help", "-h":
        printUsage()
        exit(0)
    case "--socket", "-s":
        i += 1
        guard i < args.count else {
            fputs("Error: --socket requires a path\n", stderr)
            exit(1)
        }
        socketPath = args[i]
    case "--watch", "-w":
        i += 1
        guard i < args.count else {
            fputs("Error: --watch requires a path\n", stderr)
            exit(1)
        }
        watchPaths.append(args[i])
    default:
        if args[i].hasPrefix("-") {
            fputs("Unknown option: \(args[i])\n", stderr)
            exit(1)
        }
        watchPaths.append(args[i])
    }
    i += 1
}

// Default: watch iCloud Drive
if watchPaths.isEmpty {
    let home = FileManager.default.homeDirectoryForCurrentUser.path
    let icloudDrive = "\(home)/Library/Mobile Documents"
    if FileManager.default.fileExists(atPath: icloudDrive) {
        watchPaths.append(icloudDrive)
    } else {
        fputs(
            "Error: no watch paths given and iCloud Drive not found at \(icloudDrive)\n",
            stderr)
        exit(1)
    }
}

// -- Set up components --

let manager = HydrationManager()

let ipcServer = IPCServer(socketPath: socketPath, manager: manager)

let watcher = FSEventsWatcher(paths: watchPaths) { events in
    Task {
        for event in events where event.isItemIsFile {
            _ = try? await manager.handleEvent(for: event.path)
        }
    }
}

// Graceful shutdown on SIGTERM / SIGINT
let sigSources = [SIGTERM, SIGINT].map { sig -> DispatchSourceSignal in
    signal(sig, SIG_IGN)
    let src = DispatchSource.makeSignalSource(signal: sig, queue: .main)
    src.setEventHandler {
        print("\nShutting down…")
        watcher.stop()
        ipcServer.stop()
        exit(0)
    }
    src.resume()
    return src
}
_ = sigSources  // prevent deallocation

// -- Start --

do {
    try ipcServer.start()
} catch {
    fputs("Failed to start IPC server: \(error)\n", stderr)
    exit(1)
}
watcher.start()

print("Hydration daemon v\(version) started")
print("  Socket: \(socketPath)")
print("  Watching: \(watchPaths.joined(separator: ", "))")

dispatchMain()

// MARK: - Helpers

func printUsage() {
    print("""
        Usage: HydrationDaemon [options] [paths...]

        Options:
          -w, --watch <path>   Directory to watch (repeatable)
          -s, --socket <path>  IPC socket path (default: \(HydrationCore.defaultSocketPath))
          -v, --version        Print version and exit
          -h, --help           Print this help and exit

        If no paths are given, watches ~/Library/Mobile Documents (iCloud Drive).
        """)
}
