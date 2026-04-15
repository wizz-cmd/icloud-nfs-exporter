# M4 Implementation Report — Menu Bar App

**Milestone:** M4 (Menu bar app)
**Date completed:** 2026-04-15
**Commit:** `9a0a223` on `main`
**CI run:** Passed (1m26s on macOS 15) — 56 tests (17 Swift + 21 Rust + 18 Python)

---

## Objective

Build a macOS menu bar status item that displays daemon health, configured exports, and provides quick access to configuration and diagnostics.

## Components

### IPCClient (`HydrationCore/IPCClient.swift`)

Client-side counterpart to `IPCServer`, added to the shared `HydrationCore` library so both the menu bar app and any future Swift component can connect to the daemon.

| Method | Purpose |
|---|---|
| `send(_:)` | Raw request/response over Unix domain socket |
| `ping()` | Health check (returns `Bool`) |
| `queryState(path:)` | File hydration state query |
| `isAvailable` | Socket exists + responds to ping |

- Configurable timeout (default 10s)
- Sets both `SO_RCVTIMEO` and `SO_SNDTIMEO` on the socket
- Max response size: 1 MB
- `IPCClientError` with descriptive messages via `strerror`

### MenuBarApp (`src/app/`)

AppKit-based menu bar application using `NSStatusBar` and `NSStatusItem`.

**Architecture:**
- `main.swift` — creates `NSApplication`, sets `AppDelegate`, runs event loop
- `AppDelegate.swift` — manages the status item, polling, and menu
- `ConfigReader.swift` — lightweight TOML parser for the `icne` config file

**Menu structure:**

```
[cloud icon] ▾
──────────────────────
  iCloud NFS Exporter
──────────────────────
  Daemon: Running/Stopped
──────────────────────
  iCloud Drive
    ↳ /tmp/icne-mnt/CloudDocs
──────────────────────
  Refresh            ⌘R
  Open Config…       ⌘,
  Run Diagnostics…   ⌘D
──────────────────────
  Quit               ⌘Q
```

**Behaviour:**
- Runs as a menu bar accessory (no Dock icon) via `NSApp.setActivationPolicy(.accessory)`
- Icon: `cloud.fill` (SF Symbols) when daemon is running, `cloud` when stopped
- Polls daemon status every 10 seconds on a background queue
- Menu is rebuilt only when status changes (avoids flicker)
- "Open Config" opens the TOML file in the default editor
- "Run Diagnostics" launches `icne diagnose` in Terminal

### ConfigReader

Parses `~/.config/icloud-nfs-exporter/config.toml` without external dependencies.

Handles:
- `[general]` section: `socket_path`, `mount_base`
- `[[folders]]` array: `source`, `label`
- Quoted string values, comments, unknown sections
- Falls back to defaults when the config file is missing

### Package changes

- `Hydration/Package.swift`: added `.library(name: "HydrationCore", ...)` product declaration for cross-package consumption
- `MenuBarApp/Package.swift`: added `.package(path: "../hydration")` dependency on `HydrationCore`

## Architecture alignment

- **Facade** (ARCHITECTURE.md §4) — the menu bar app presents a unified view over config, daemon, and NFS state
- **Daemon / Singleton** (ARCHITECTURE.md §5) — the app monitors the launchd-managed daemon via IPC
- **Convention over Configuration** (ARCHITECTURE.md §6) — config is auto-discovered at the standard path; the app works with zero setup

## Next milestone

**M5 — Setup wizard**: guided first-run experience that walks users through prerequisites (macFUSE, iCloud Drive), folder selection, and NFS configuration.
