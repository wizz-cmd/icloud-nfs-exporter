# Session Handover Document

> **Read this first at the start of every new session.**
> Last updated: 2026-04-17

---

## Project State

**icloud-nfs-exporter** — macOS service exporting iCloud Drive folders via NFS with transparent hydration. Experimental / pre-alpha.

### What Works
- **Hydration daemon** (Swift) — FileState machine, FSEvents watcher, IPC server over Unix socket. Compiles and tests pass (17 tests).
- **FUSE driver** (Rust) — IPC client, protocol types, .icloud stub path utils, **passthrough filesystem** (`fuser::Filesystem` impl with inode table, stub translation in readdir, hydration interception in open, symlink readlink). 43 tests pass (21 fuse-core + 6 passthrough + 16 doc-tests).
- **End-to-end FUSE mount verified** — kext backend works: root listing, subdirectory traversal, symlink following (Desktop/Documents → ~/), file content reads (verified with lynis.log, .DS_Store). macFUSE v5.1.3 kext.
- **CLI tool** `icne` (Python) — setup wizard, add-folder, diagnose, exports, list. 24 tests pass.
- **Menu bar app** (Swift/SwiftUI) — `@main App` with `MenuBarExtra`, `Settings` TabView (4 tabs), `@Observable AppState`, VoiceOver labels. Compiles clean.
- **CI** — GitHub Actions on macOS 15, runs all tests on every push.
- **Distribution** — `.dmg` built by release workflow on tag push, Homebrew formula, Makefile install/uninstall.

### What Does NOT Work Yet
- **macFUSE FSKit backend** — module registration corrupted by `pluginkit -e use` (version shows `(null)` instead of `(1.5)`). Known bug: [macfuse/macfuse#1132](https://github.com/macfuse/macfuse/issues/1132). Fix: upgrade to macFUSE 5.2.0 (released 2026-04-09) or re-register via `sudo macfuse install --force` + `sudo killall fskitd`. Kext backend works as fallback. See `docs/internal/reports/e2e-fuse-mount-test.md` for full analysis.
- **Hydration end-to-end untested** — no `.icloud` stubs found in current iCloud Drive (all files local). Hydration path is implemented but needs a real evicted file to verify.
- **FUSE warnings**: `getxattr`, `listxattr`, `flush` not implemented — benign (macOS Finder probes).
- **NFS export** — not yet wired to the FUSE mount.
- **Code signing** — app is unsigned, requires `xattr -cr` workaround. Needs Apple Developer account ($99/year).

### Immediate Next Step
**Wire NFS export to the FUSE mount.** The FUSE passthrough is verified end-to-end with kext backend. Next:
1. Choose NFS server approach (nfs-ganesha, unfs3, or kernel re-export)
2. Export the FUSE mountpoint via NFS v3/v4
3. Test from a Linux NFS client

### Architecture (Key Files)

```
src/hydration/                    # Swift (SPM), macOS 14+
├── Sources/HydrationCore/        # Library: FileState, Detector, Manager, FSEvents, IPC
├── Sources/HydrationDaemon/      # Executable: CLI daemon with --watch/--socket
└── Tests/HydrationTests/         # 17 XCTest tests

src/fuse/                         # Rust (Cargo workspace)
├── fuse-core/src/                # Library: IPC client/protocol, path_utils
└── fuse-driver/src/              # Binary: CLI + FUSE mount (passthrough.rs)

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
- Rust: 1.94.1 (installed via rustup, `~/.cargo/env` sourced in `.zshrc`). 43 tests pass locally (21 fuse-core + 6 passthrough + 16 doc-tests).
- Python 3.14
- macFUSE: 5.1.3 installed. Kext backend works. FSKit module registered + enabled (`pluginkit +`) but runtime reports "disabled" — needs investigation. libfuse + headers at `/usr/local/lib`, `/usr/local/include/fuse`

### Design References
- `docs/internal/research/macos-app-design-rules.md` — comprehensive HIG/SwiftUI/distribution guide
- `docs/internal/research/v0.2-implementation-plan.md` — completed v0.2 plan
- `ARCHITECTURE.md` — design patterns (Proxy, Lazy Init, State Machine, Facade, Strategy)

### Completed Roadmap
M0 Scaffold → M1 Hydration Daemon → M2 FUSE Core → M3 NFS Wiring → M4 Menu Bar App → M5 Setup Wizard → M6 Distribution → M7 Polish → v0.2 SwiftUI Migration → API Documentation
