# M3 Implementation Report — NFS Wiring

**Milestone:** M3 (NFS wiring)
**Date completed:** 2026-04-15
**Commit:** `32c89f4` on `main`
**CI run:** Passed (54s on macOS 15) — 17 Swift + 21 Rust + 18 Python = 56 tests

---

## Objective

Build the configuration, NFS export management, and diagnostic tooling that wires the hydration daemon (M1) and FUSE driver (M2) to a macOS NFS server. The `icne` CLI becomes the primary interface for setting up and managing exports.

## Components

### Configuration (`scripts/icne_lib/config.py`)

TOML-based configuration at `~/.config/icloud-nfs-exporter/config.toml`.

**Structure:**

```toml
[general]
socket_path = "/tmp/icloud-nfs-exporter.sock"
mount_base = "/tmp/icne-mnt"

[nfs]
server = "nfsd"
allowed_network = "192.168.0.0/24"

[[folders]]
source = "/Users/chris/Library/Mobile Documents/com~apple~CloudDocs"
label = "iCloud Drive"
```

**Operations:**
- `load_config()` / `save_config()` — TOML read/write (uses Python 3.11+ `tomllib`)
- `add_folder()` / `remove_folder()` — manage the folder list with duplicate detection
- `mount_point_for()` — derive the FUSE mount point from a source path
- `default_config()` — sensible defaults for zero-config setup

### NFS Export Management (`scripts/icne_lib/nfs.py`)

Manages `/etc/exports` entries for macOS `nfsd`.

| Function | Purpose |
|---|---|
| `cidr_to_network_mask()` | Convert `192.168.0.0/24` → `(192.168.0.0, 255.255.255.0)` |
| `generate_exports_entry()` | Build an `/etc/exports` line for a mount point |
| `update_exports()` | Insert/replace our managed block (preserves user entries) |
| `apply_exports()` | Write `/etc/exports` and reload nfsd |
| `nfsd_is_running()` | Check nfsd status |
| `show_exports()` | Query active exports via `showmount -e` |

The managed block uses markers (`# BEGIN/END icloud-nfs-exporter`) for idempotent updates — user-written exports are never modified.

### Python IPC Client (`scripts/icne_lib/ipc.py`)

Python equivalent of the Rust IPC client, matching the Swift daemon's wire format.

- `IpcClient.ping()` — health check
- `IpcClient.query_state(path)` — file state query
- `IpcClient.hydrate(path)` — trigger hydration
- `IpcClient.is_available()` — check socket existence + ping

Used by the diagnostic system to verify daemon connectivity.

### Diagnostics (`scripts/icne_lib/diagnose.py`)

Seven checks covering the full system stack:

| Check | What it verifies |
|---|---|
| iCloud Drive | `~/Library/Mobile Documents` exists |
| Config file | TOML parses, folder count |
| Hydration daemon | IPC socket responds to ping |
| macFUSE | `/Library/Filesystems/macfuse.fs` exists, version |
| NFS server | `nfsd status`, active export count |
| Mount base | Mount point directory exists |
| Rust toolchain | `cargo` is in PATH |

### CLI (`scripts/icne`)

Expanded from 3 stub commands to 6 fully implemented commands:

| Command | Purpose |
|---|---|
| `setup [--force]` | Create config dir, default config, mount base, launchd plist |
| `add-folder <path> [--label] [--apply-nfs]` | Add a folder, optionally update exports |
| `remove-folder <path>` | Remove a folder from config |
| `list` | Show configured folders and their mount points |
| `diagnose` | Run all diagnostic checks |
| `exports [--dry-run]` | Generate or apply NFS exports |

### Makefile

Test target now runs all three test suites: `swift test` + `cargo test` + `python3 -m unittest discover`.

## Tests

18 Python tests across 3 test classes:

| Class | Tests | Coverage |
|---|---|---|
| `TestConfig` | 8 | Default config, save/load round-trip, add/remove/duplicate folder, mount point derivation |
| `TestNfs` | 5 | CIDR conversion, exports entry generation, managed block insert/replace/preserve |
| `TestIpc` | 5 | Client creation, missing socket, full IPC round-trip with mock Unix socket server |

The IPC tests use a real Unix domain socket with a threaded mock server to verify the wire format.

## Verified locally

```
$ icne setup          # Creates config, mount dir, launchd plist
$ icne add-folder ~/Library/Mobile\ Documents/com~apple~CloudDocs
$ icne list           # Shows folder + mount point
$ icne exports --dry-run   # Generates /etc/exports content (preserves existing)
$ icne diagnose       # 2/7 checks pass (iCloud Drive, nfsd present)
```

## Architecture alignment

- **Facade pattern** (ARCHITECTURE.md §4) — the CLI presents a single interface over config, NFS, daemon, and FUSE subsystems.
- **Convention over Configuration** (ARCHITECTURE.md §6) — defaults work for the common case; user only specifies which folders to export.
- **Strategy pattern** (ARCHITECTURE.md §4) — NFS backend is configurable (`nfsd` vs `ganesha`), though only `nfsd` is implemented.

## Next milestone

**M4 — Menu bar app**: implement the SwiftUI menu bar status item in `src/app/` showing daemon health, export status, and hydration activity.
