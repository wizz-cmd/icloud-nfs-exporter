# Session Handover Document

> **Read this first at the start of every new session.**
> Last updated: 2026-04-17

---

## Project State

**icloud-nfs-exporter** — macOS service exporting iCloud Drive folders via NFS with transparent hydration. Experimental / pre-alpha.

### What Works
- **Hydration daemon** (Swift) — FileState machine, FSEvents watcher, IPC server over Unix socket. Compiles and tests pass (17 tests).
- **Direct NFSv3 server** (Rust, `nfsserve` crate) — replaces FUSE+nfsd approach. Implements `NFSFileSystem` trait with inode table, stub translation in readdir, hydration interception in read (via `spawn_blocking` IPC), symlink readlink. 9 tests pass. Reuses `fuse-core` for IPC client/protocol/path_utils. **Verified cross-machine**: M2 MacBook (macOS 26.3.1) successfully mounted iCloud Drive from Intel mini over LAN (NFSv3, port 11111).
- **FUSE driver** (Rust, retained) — passthrough filesystem still works via macFUSE kext. 43 tests pass (21 fuse-core + 6 passthrough + 16 doc-tests). No longer the primary NFS path.
- **Hydration verified end-to-end** — two mechanisms work: (1) APFS dataless files auto-hydrate on `File::open()` (content served without persisting to disk — ideal for NFS), (2) `.icloud` stubs hydrated via IPC→daemon→`brctl download`. Orphaned stubs correctly return EIO.
- **CLI tool** `icne` (Python) — setup wizard, add-folder, diagnose, exports (start/stop NFS server), list. 28 tests pass.
- **Menu bar app** (Swift/SwiftUI) — `@main App` with `MenuBarExtra`, `Settings` TabView (4 tabs), `@Observable AppState`, VoiceOver labels. Compiles clean.
- **CI** — GitHub Actions on macOS 15: builds Swift + Rust (FUSE + NFS), runs all tests, clippy lint. macFUSE installed via brew.
- **Distribution** — `.dmg` built by release workflow on tag push, Homebrew formula, Makefile install/uninstall.

### What Does NOT Work Yet
- **macFUSE FSKit backend** — Intel mini (dev machine): `fskitd` never starts, FSKit mount segfaults. Confirmed Intel-specific — tested on M2 MacBook (macOS 26.3.1) where `fskitd` runs fine (PID visible). M2 FSKit mount still failed due to module mismatch (fuse-t registered, not macFUSE). Kext works on Intel mini; kext on Apple Silicon requires recovery-mode boot. **Decision: use kext backend on Intel mini, defer FSKit until Apple Silicon becomes primary dev machine.** See `docs/internal/reports/e2e-fuse-mount-test.md` and `scripts/fskit-test.sh`.
- **Code signing** — app is unsigned, requires `xattr -cr` workaround. Needs Apple Developer account ($99/year).

### Immediate Next Step
**Test from a Linux NFS client.** macOS-to-macOS verified. Next:
1. ~~Start hydration daemon + NFS server against iCloud Drive~~ — done
2. ~~Mount via `mount_nfs` on localhost~~ — done (Intel mini)
3. ~~Mount from M2 MacBook over LAN~~ — done (192.168.21.130:11111)
4. Test from a Linux NFS client on the LAN
5. Stress test: large files, many concurrent reads, long-running mount

### Architecture (Key Files)

```
src/hydration/                    # Swift (SPM), macOS 14+
├── Sources/HydrationCore/        # Library: FileState, Detector, Manager, FSEvents, IPC
├── Sources/HydrationDaemon/      # Executable: CLI daemon with --watch/--socket
└── Tests/HydrationTests/         # 17 XCTest tests

src/nfs/                          # Rust (Cargo workspace)
└── nfs-server/src/               # Binary: direct NFSv3 server (icloud_nfs.rs)

src/fuse/                         # Rust (Cargo workspace), retained
├── fuse-core/src/                # Library: IPC client/protocol, path_utils (shared with nfs-server)
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
- Rust: 1.94.1 (installed via rustup, `~/.cargo/env` sourced in `.zshrc`). 52 Rust tests pass locally (21 fuse-core + 6 passthrough + 16 doc-tests + 9 nfs-server). Clippy clean on both workspaces.
- Python 3.14
- macFUSE: 5.2.0 installed (Homebrew cask). Kext backend works (Intel mini). FSKit broken on this Intel Mac (`fskitd` never starts, mount segfaults). Verified FSKit infra works on M2/macOS 26 but mount needs module fix. Kext mount at `/Volumes` requires sudo; `/tmp` works without. libfuse + headers at `/usr/local/lib`, `/usr/local/include/fuse`

### Design References
- `docs/internal/research/macos-app-design-rules.md` — comprehensive HIG/SwiftUI/distribution guide
- `docs/internal/research/v0.2-implementation-plan.md` — completed v0.2 plan
- `ARCHITECTURE.md` — design patterns (Proxy, Lazy Init, State Machine, Facade, Strategy)

### Completed Roadmap
M0 Scaffold → M1 Hydration Daemon → M2 FUSE Core → M3 NFS Wiring → M4 Menu Bar App → M5 Setup Wizard → M6 Distribution → M7 Polish → v0.2 SwiftUI Migration → API Documentation → Direct NFSv3 Server (read-only)

### Future Roadmap
- **Read-write NFS** — implement write-back through iCloud: `write()`/`create()`/`mkdir()`/`rename()`/`remove()` in NFS server → upload via iCloud APIs. Requires: iCloud upload mechanism in hydration daemon, conflict resolution with iCloud sync, file locking strategy.
- **FSKit migration** — revisit when macFUSE ships macOS 26-compatible FSKit module (for local FUSE mount without kext, if needed alongside NFS)
