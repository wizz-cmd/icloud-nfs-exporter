# M2 Implementation Report — Rust FUSE Driver Core

**Milestone:** M2 (Rust FUSE driver)
**Date completed:** 2026-04-15
**Commit:** `3eba75d` on `main`
**CI run:** Passed (1m10s on macOS 15) — 21 Rust tests + 17 Swift tests

---

## Objective

Implement the Rust-side bridge between the FUSE driver and the Swift hydration daemon: IPC client, wire protocol, and iCloud stub path translation. The actual `fuser::Filesystem` mount is deferred until macFUSE is installed.

## Components

### IPC Protocol (`fuse-core/src/ipc_protocol.rs`)

Rust types that mirror the Swift daemon's IPC format exactly.

**Types:**

| Type | Variants |
|---|---|
| `FileState` | `Unknown`, `Evicted`, `Downloading`, `Local`, `Error` |
| `Request` | `Ping`, `QueryState { path }`, `Hydrate { path }` |
| `Response` | `Pong`, `State { path, state }`, `HydrationResult { path, success, error? }` |

**Wire format:** 4-byte big-endian length prefix + JSON payload, implemented via `wire_encode()`, `wire_read_length()`, and `wire_decode()`.

**Serialization:** serde internally-tagged enums (`#[serde(tag = "type")]`) produce JSON identical to the Swift daemon's `Codable` output. Cross-compatibility is verified by a dedicated test that decodes hand-written Swift-format JSON.

### IPC Client (`fuse-core/src/ipc_client.rs`)

`IpcClient` connects to the hydration daemon's Unix domain socket and provides typed methods:

| Method | Request | Response |
|---|---|---|
| `ping()` | `Ping` | `Ok(())` on `Pong` |
| `query_state(path)` | `QueryState` | `Ok(FileState)` |
| `hydrate(path)` | `Hydrate` | `Ok(())` on success, `Err(HydrationFailed)` otherwise |
| `send(request)` | any | raw `Response` |

- Configurable timeout (default 300s for hydration, 10s write timeout).
- Max response size: 1 MB.
- Each `send()` opens a new connection (matches the daemon's per-connection model).
- Proper error type (`IpcError`) with `Display` and `Error` impls.

### Path Utilities (`fuse-core/src/path_utils.rs`)

Handles the `.icloud` stub naming convention:

| Function | Example |
|---|---|
| `is_icloud_stub(name)` | `.Report.pdf.icloud` → `true` |
| `stub_to_real_name(stub)` | `.Report.pdf.icloud` → `Some("Report.pdf")` |
| `real_to_stub_name(name)` | `Report.pdf` → `.Report.pdf.icloud` |

Edge cases handled:
- Names with multiple dots (`archive.tar.gz`)
- Unicode filenames (`日本語.txt`)
- Minimum length validation (rejects `.icloud` alone)
- Round-trip correctness for all cases

### FUSE Driver CLI (`fuse-driver/src/main.rs`)

A diagnostic CLI that exercises the IPC client:

```
Usage: fuse-driver [options] <command>

Commands:
  ping               Check if the hydration daemon is running
  query <path>       Query the hydration state of a file
  hydrate <path>     Request hydration of an evicted file
```

This enables end-to-end testing of the Swift daemon ↔ Rust client communication without requiring macFUSE.

## Tests

21 tests in `fuse-core`, all passing:

| Module | Tests | Coverage |
|---|---|---|
| `ipc_protocol` | 10 | All request/response round-trips, wire format encoding, FileState serialization, cross-compat with Swift JSON |
| `ipc_client` | 3 | Client creation, timeout configuration, connection failure to missing socket |
| `path_utils` | 8 | Stub detection, name translation, non-stub rejection, round-trip, multi-dot names, Unicode |

## What's deferred

**FUSE mount (`fuser::Filesystem` implementation)** — requires macFUSE kernel extension, which is not installed on the development machine or CI runners. The passthrough filesystem will be implemented when macFUSE is available. All supporting logic (IPC, path translation) is complete and tested.

The implementation plan for the FUSE mount:

1. Inode table mapping FUSE inode numbers to real filesystem paths
2. `lookup` / `readdir` that translate `.icloud` stubs to real names
3. `open` that detects stubs and calls `IpcClient::hydrate()` before returning the fd
4. `read` / `getattr` / `release` pass-through to the real filesystem

## Architecture alignment

- **Proxy pattern** (ADR 001) — the FUSE driver is the proxy that intercepts VFS calls and triggers hydration via IPC to the Swift daemon.
- **IPC boundary** (ADR 004) — clean language split: Rust FUSE driver communicates with Swift hydration daemon over Unix domain socket with length-prefixed JSON. Protocol types are tested for cross-language compatibility.
- **Language split** (ADR 004) — all M2 code is Rust, using only `serde`/`serde_json` as external dependencies.

## Next milestone

**M3 — NFS wiring**: configure macOS `nfsd` (or `nfs-ganesha`) to export the FUSE mountpoint over NFS. Requires macFUSE installation and the FUSE passthrough implementation.
