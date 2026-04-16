# M1 Implementation Report — Swift Hydration Daemon

**Milestone:** M1 (Swift hydration daemon)
**Date completed:** 2026-04-15
**Commits:** `38cab28`, `8daab68` on `main`
**CI run:** Passed (48s on macOS 15)

---

## Objective

Implement the core iCloud hydration logic in Swift: detect file states, watch directories for changes, trigger on-demand downloads, and expose an IPC interface for the FUSE driver (M2) to request hydration.

## Components

### FileState (`FileState.swift`)

State machine enum encoding the hydration lifecycle from ARCHITECTURE.md:

```
UNKNOWN → EVICTED → DOWNLOADING → LOCAL
                        ↑            │
                        └────────────┘  (re-eviction)
Any state → ERROR; ERROR → EVICTED / DOWNLOADING / UNKNOWN
```

- Self-transitions are invalid (prevents no-op state changes).
- `canTransition(to:)` enforces valid transitions before any state update.
- `Codable` + `Sendable` for IPC serialisation and actor safety.

### FileStateDetector (`FileStateDetector.swift`)

Protocol `FileStateDetecting` with a production implementation `FileStateDetector`.

Detection strategy (ordered):

1. **Stub file check** — filenames matching `.*.icloud` pattern are evicted placeholders.
2. **Existence check** — non-existent paths return `.unknown`.
3. **URL resource values** — queries `isUbiquitousItemKey`, `ubiquitousItemDownloadingStatusKey`, and `ubiquitousItemIsDownloadingKey` for iCloud-managed files.
4. **Non-iCloud fallback** — files not managed by iCloud are treated as `.local`.

The protocol allows test code to inject a `MockDetector` without touching the filesystem.

### HydrationManager (`HydrationManager.swift`)

Swift `actor` that owns per-file state and coordinates hydration.

Key operations:

| Method | Behaviour |
|---|---|
| `refreshState(for:)` | Detect on-disk state and record it |
| `hydrate(path:)` | If evicted: transition to downloading, call `FileManager.startDownloadingUbiquitousItem(at:)`, poll until local or timeout |
| `handleEvent(for:)` | Re-detect state from an FSEvents notification |
| `stopTracking(_:)` | Remove a file from the tracked set |

- Hydration of an already-local file returns immediately.
- Hydration of a currently-downloading file joins the existing poll loop.
- Timeout defaults to 300 s; poll interval defaults to 0.5 s — both configurable via init.
- Invalid transitions throw `HydrationError` with a descriptive message.

### FSEventsWatcher (`FSEventsWatcher.swift`)

Wraps the CoreServices FSEvents C API in a Swift class.

- Watches one or more directory trees with file-level granularity (`kFSEventStreamCreateFlagFileEvents`).
- Reports xattr changes (`kFSEventStreamEventFlagItemXattrMod`) — critical for detecting iCloud state transitions that modify `com.apple.icloud#` extended attributes.
- Uses `kFSEventStreamCreateFlagNoDefer` for low-latency event delivery.
- Events dispatched on a dedicated `DispatchQueue`; handler receives `[Event]` batches.
- `@unchecked Sendable` with lifecycle managed via `start()` / `stop()` / `deinit`.

### IPC Protocol (`IPCProtocol.swift`)

Length-prefixed JSON wire format (4-byte big-endian length + JSON payload) for communication between the FUSE driver and the hydration daemon.

**Requests** (FUSE driver → daemon):

| Type | Payload | Purpose |
|---|---|---|
| `ping` | — | Health check |
| `query_state` | `path` | Ask current hydration state |
| `hydrate` | `path` | Request on-demand hydration (blocks until done) |

**Responses** (daemon → FUSE driver):

| Type | Payload | Purpose |
|---|---|---|
| `pong` | — | Ping reply |
| `state` | `path`, `state` | Current file state |
| `hydration_result` | `path`, `success`, `error?` | Hydration outcome |

### IPC Server (`IPCServer.swift`)

Unix domain socket server accepting connections from the FUSE driver.

- Binds to a configurable path (default: `/tmp/icloud-nfs-exporter.sock`).
- Socket permissions set to `0600` (owner-only).
- Accept loop via `DispatchSource.makeReadSource`; each client handled in a `Task`.
- Max message size: 1 MB.
- Read/write helpers (`readExact`, `writeAll`) handle partial I/O.

### HydrationDaemon (`main.swift`)

CLI entry point that wires all components together.

```
Usage: HydrationDaemon [options] [paths...]

Options:
  -w, --watch <path>   Directory to watch (repeatable)
  -s, --socket <path>  IPC socket path
  -v, --version        Print version and exit
  -h, --help           Print this help and exit
```

- Auto-discovers `~/Library/Mobile Documents` (iCloud Drive) when no paths given.
- Registers `SIGTERM` / `SIGINT` handlers for graceful shutdown.
- Runs indefinitely via `dispatchMain()`.

## Tests

17 tests across 4 test classes, all passing in CI:

| Class | Tests | What's covered |
|---|---|---|
| `FileStateTests` | 3 | All valid/invalid transitions, self-transition rejection, Codable round-trip |
| `IPCProtocolTests` | 7 | Wire format encode/decode for every request and response variant, short-buffer edge case |
| `HydrationManagerTests` | 6 | State refresh, tracking count, stop tracking, hydrate-already-local, event handling, unknown path |
| `VersionTests` | 2 | Version string, default socket path |

The `MockDetector` (conforming to `FileStateDetecting`) enables testing the manager without iCloud or filesystem access.

## Issues encountered

1. **FSEvents callback types** — Swift 6 on the CI runner (Xcode 16 / macOS 15) treats `eventFlags` and `eventIds` as non-optional `UnsafePointer` types, unlike Swift 5.x which allowed optional force-unwrap. Fixed by removing the `!` operators.

2. **XCTest autoclosure + await** — `XCTAssertNotNil(await expr)` is invalid under Swift 6 strict concurrency because `await` cannot appear inside an autoclosure. Fixed by extracting the `await` into a local `let` binding.

## Architecture alignment

- **Proxy / Virtual Proxy** (ADR 001) — FileStateDetector + HydrationManager form the hydration shim that the FUSE driver will proxy through.
- **State pattern** (ARCHITECTURE.md §3) — FileState enum with transition validation.
- **Lazy Initialization** (ARCHITECTURE.md §2) — hydration is triggered only on demand via IPC, not proactively.
- **Language split** (ADR 004) — all M1 code is Swift, using Foundation and CoreServices frameworks.

## Next milestone

**M2 — Rust FUSE driver**: implement the macFUSE passthrough filesystem in `src/fuse/`, connecting to the hydration daemon via the IPC socket to trigger on-demand hydration when evicted files are opened.
