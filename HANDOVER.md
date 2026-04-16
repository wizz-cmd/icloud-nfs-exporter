# Session Handover Document

> **Read this first at the start of every new session.**
> Last updated: 2026-04-16 (commit `1255256`)

---

## Project State

**icloud-nfs-exporter** — macOS service exporting iCloud Drive folders via NFS with transparent hydration. Experimental / pre-alpha.

### What Works
- **Hydration daemon** (Swift) — FileState machine, FSEvents watcher, IPC server over Unix socket. Compiles and tests pass (17 tests).
- **FUSE driver core** (Rust) — IPC client, protocol types (cross-compat with Swift daemon), .icloud stub path utils. 21 tests pass.
- **CLI tool** `icne` (Python) — setup wizard, add-folder, diagnose, exports, list. 24 tests pass.
- **Menu bar app** (Swift/SwiftUI) — `@main App` with `MenuBarExtra`, `Settings` TabView (4 tabs), `@Observable AppState`, VoiceOver labels. Compiles clean.
- **CI** — GitHub Actions on macOS 15, runs all 62 tests on every push.
- **Distribution** — `.dmg` built by release workflow on tag push, Homebrew formula, Makefile install/uninstall.

### What Does NOT Work Yet
- **End-to-end pipeline is not functional.** No FUSE mount exists → NFS has nothing to export → files can't be served.
- **macFUSE** — user is installing it now (`brew install --cask macfuse`). Reboot required. After reboot, verify with `ls /Library/Filesystems/macfuse.fs`.
- **Rust toolchain** — not installed on dev machine. All Rust builds verified in CI only.
- **Code signing** — app is unsigned, requires `xattr -cr` workaround. Needs Apple Developer account ($99/year).

### Immediate Next Step
**Implement the `fuser::Filesystem` passthrough in `src/fuse/fuse-driver/`.** This is the critical missing piece:
1. Verify macFUSE is installed and Rust is available
2. Add `fuser` crate dependency to `src/fuse/fuse-driver/Cargo.toml`
3. Implement passthrough filesystem: inode table, lookup, getattr, readdir (translating .icloud stubs), open (triggering hydration via IPC), read, release
4. Wire into `main.rs` as a CLI that mounts FUSE at a given path
5. Test end-to-end: mount over iCloud Drive → open evicted file → file hydrates → content served

### Architecture (Key Files)

```
src/hydration/                    # Swift (SPM), macOS 14+
├── Sources/HydrationCore/        # Library: FileState, Detector, Manager, FSEvents, IPC
├── Sources/HydrationDaemon/      # Executable: CLI daemon with --watch/--socket
└── Tests/HydrationTests/         # 17 XCTest tests

src/fuse/                         # Rust (Cargo workspace)
├── fuse-core/src/                # Library: IPC client/protocol, path_utils
└── fuse-driver/src/              # Binary: CLI tool (FUSE mount TODO)

src/app/                          # Swift (SPM), macOS 14+
└── Sources/MenuBarApp/           # @main App, AppState, StatusPanel, Settings tabs

scripts/
├── icne                          # Python CLI entry point
├── icne_lib/                     # config, nfs, ipc, diagnose, icloud, wizard
├── build-release.sh              # Universal binary builder
├── create-app-bundle.sh          # .app bundle creator
└── create-dmg.sh                 # .dmg disk image creator
```

### IPC Protocol
Length-prefixed JSON over Unix domain socket (`/tmp/icloud-nfs-exporter.sock`):
- **Request**: `{"type":"ping"}`, `{"type":"query_state","path":"..."}`, `{"type":"hydrate","path":"..."}`
- **Response**: `{"type":"pong"}`, `{"type":"state","path":"...","state":"local"}`, `{"type":"hydration_result","path":"...","success":true}`
- Wire: 4-byte big-endian length + JSON payload

### Config
TOML at `~/.config/icloud-nfs-exporter/config.toml`. Shared between Python CLI and Swift app. Format:
```toml
[general]
socket_path = "/tmp/icloud-nfs-exporter.sock"
mount_base = "/tmp/icne-mnt"
[nfs]
allowed_network = "192.168.0.0/24"
[[folders]]
source = "/Users/chris/Library/Mobile Documents/com~apple~CloudDocs"
label = "iCloud Drive"
```

### Version
All components at `0.2.0`. Released as v0.2.0 on GitHub.

### Dev Environment
- macOS 15 (Darwin 24.6.0), Intel (x86_64)
- Swift 6.2.3 (Xcode CLI tools only — no full Xcode, so `swift test` fails locally for XCTest)
- Rust: not installed locally, CI-only
- Python 3.14
- macFUSE: installing (pending reboot)

### Design References
- `docs/internal/research/macos-app-design-rules.md` — comprehensive HIG/SwiftUI/distribution guide
- `docs/internal/research/v0.2-implementation-plan.md` — completed v0.2 plan
- `ARCHITECTURE.md` — design patterns (Proxy, Lazy Init, State Machine, Facade, Strategy)

### Completed Roadmap
M0 Scaffold → M1 Hydration Daemon → M2 FUSE Core → M3 NFS Wiring → M4 Menu Bar App → M5 Setup Wizard → M6 Distribution → M7 Polish → v0.2 SwiftUI Migration → API Documentation
