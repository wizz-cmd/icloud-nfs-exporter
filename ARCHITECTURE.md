# Architecture

This document records the architectural decisions, design patterns, and references for `icloud-nfs-exporter`.

---

## High-Level Overview

```
NFS Client (Linux/macOS/etc.)
        │  NFS v3/v4 protocol
        ▼
┌───────────────────────────────────┐
│        NFS Server Layer           │  (nfs-ganesha or macOS built-in nfsd)
│  exports a virtual/FUSE mountpoint│
└──────────────┬────────────────────┘
               │ POSIX filesystem calls (open, read, stat, readdir …)
               ▼
┌───────────────────────────────────┐
│     iCloud Hydration FUSE FS      │  (macFUSE + Python/Rust driver)
│  Intercepts opens of evicted files│
│  Triggers download, blocks caller │
│  Returns real fd once local       │
└──────────────┬────────────────────┘
               │ pass-through to real path once hydrated
               ▼
┌───────────────────────────────────┐
│    iCloud Drive folder on disk    │  ~/Library/Mobile Documents/…
│  (may contain .icloud stub files) │
└───────────────────────────────────┘
               │  brctl / FileProvider API
               ▼
          iCloud servers
```

---

## Design Patterns

### 1. Virtual Filesystem / FUSE Passthrough with Intercepting Layer

**Pattern:** FUSE Passthrough Filesystem
**Description:** A FUSE driver mounts a directory and forwards all VFS calls to the underlying real path unchanged — *except* for calls that require the file to be physically present (open, read, mmap). On those calls the driver checks the iCloud eviction state (via xattr `com.apple.icloud.itemName` / presence of `.icloud` stub) and, if needed, triggers hydration before allowing the call to proceed.
**Why:** This cleanly separates the "NFS serving" concern from the "iCloud hydration" concern. The NFS server just sees a normal POSIX filesystem.
**References:**
- macFUSE (formerly OSXFUSE): https://osxfuse.github.io/
- libfuse passthrough example: https://github.com/libfuse/libfuse/blob/master/example/passthrough.c
- Python bindings — pyfuse3: https://github.com/libfuse/pyfuse3
- Tutorial (FUSE in Python): https://thepythoncorner.com/posts/2017-02-27-writing-a-fuse-filesystem-in-python/

---

### 2. On-Demand Hydration with Blocking Open (Lazy Loading)

**Pattern:** Lazy Loading / Demand Paging
**Description:** The service does not pro-actively download all iCloud files. Instead, it hydrates a file only when a client tries to open it (lazy). The open(2) call is held (blocked) until the download completes, then the real file descriptor is returned. This is the same pattern used by macOS itself for iCloud, and by Linux's `fscache`/OverlayFS for network-backed filesystems.
**Why:** Minimises disk usage and bandwidth; no need to mirror the entire iCloud Drive locally.
**References:**
- brctl(1) man page: `man brctl` on macOS (download subcommand)
- NSFileProviderManager: https://developer.apple.com/documentation/fileprovider/nsfileprovidermanager
- Apple TN: Storing documents in iCloud — https://developer.apple.com/library/archive/documentation/General/Conceptual/iCloudDesignGuide/
- Linux fscache (conceptual parallel): https://www.kernel.org/doc/html/latest/filesystems/caching/fscache.html

---

### 3. State Machine for File Hydration

**Pattern:** State Machine (Finite State Machine — FSM)
**Description:** Each file tracked by the service transitions through well-defined states:
```
UNKNOWN → EVICTED → DOWNLOADING → LOCAL → ERROR
                         ↑               │
                         └───────────────┘  (re-eviction by macOS)
```
State is derived from xattr inspection and `brctl status`. Transitions are driven by filesystem events (FSEvents / kqueue) and download completion callbacks.
**Why:** Makes the hydration lifecycle explicit and testable; avoids race conditions by enforcing that only one download is in flight per file.
**References:**
- FSM pattern (GoF behavioural): https://refactoring.guru/design-patterns/state
- Python `transitions` library: https://github.com/pytransitions/transitions
- macOS FSEvents: https://developer.apple.com/documentation/coreservices/file_system_events

---

### 4. NFS Re-Export

**Pattern:** Proxy / Re-Export
**Description:** Rather than implementing a full NFS server from scratch, the service delegates NFS protocol handling to an existing, battle-tested NFS server (`nfs-ganesha` or macOS built-in `nfsd`). The service only provides the FUSE-backed mount point that the NFS server exports. This follows the Unix philosophy of composing small tools.
**Why:** Implementing NFS from scratch is complex and error-prone. Existing servers handle edge cases, security, and performance.
**Options evaluated:**
- **macOS built-in `nfsd`** — zero additional install; limited configurability; v3/v4 support.
- **nfs-ganesha** — highly configurable, v4.1/pNFS support, FSAL plugin architecture; available via Homebrew or source.
**References:**
- macOS nfsd(8): `man nfsd`
- nfs-ganesha: https://github.com/nfs-ganesha/nfs-ganesha
- nfs-ganesha FSAL VFS plugin: https://github.com/nfs-ganesha/nfs-ganesha/tree/next/src/FSAL/FSAL_VFS

---

### 5. macOS Service Management via launchd

**Pattern:** Service Locator / Daemon pattern
**Description:** The exporter runs as a `launchd` LaunchAgent (per-user, for iCloud access) or LaunchDaemon (system-wide, requires granting iCloud access to the daemon user). A `.plist` file in `~/Library/LaunchAgents/` ensures the service starts at login and is restarted on failure.
**Why:** `launchd` is the canonical macOS service manager. It handles restart-on-crash, throttling, log routing, and environment setup.
**References:**
- launchd(8) man page: `man launchd`
- launchd.plist(5): `man launchd.plist`
- Lingon app (GUI helper, optional): https://www.peterborgapps.com/lingon/
- Tutorial: https://www.launchd.info/

---

### 6. Configuration via Single File (Convention over Configuration)

**Pattern:** Convention over Configuration
**Description:** A single TOML/YAML config file (`~/.config/icloud-nfs-exporter/config.toml`) defines which iCloud folders to export, NFS export options, and hydration settings. Sensible defaults mean zero config is needed for the common case.
**Why:** Reduces onboarding friction; single source of truth; easy to version-control.
**References:**
- TOML spec: https://toml.io/
- Convention over Configuration (Rails origin, broadly applicable): https://en.wikipedia.org/wiki/Convention_over_configuration

---

## Technology Choices (Candidates)

| Concern | Candidate | Notes |
|---|---|---|
| FUSE driver | macFUSE + pyfuse3 (Python) or fuse-rs (Rust) | Python first for rapid iteration |
| iCloud state | `xattr`, `brctl`, `NSFileProviderManager` | `brctl` is scriptable; NSPM needs ObjC/Swift bridge |
| NFS server | macOS `nfsd` (default) / `nfs-ganesha` (advanced) | Start with nfsd |
| Service manager | `launchd` LaunchAgent | Native macOS |
| Config format | TOML | Human-friendly, no ambiguity |
| FSEvents watcher | `watchdog` (Python) | Cross-arch, wraps FSEvents & kqueue |
| Testing | `pytest` + macOS VM snapshots | Snapshot pre/post iCloud eviction states |

---

## Open Questions

- [ ] Can a LaunchAgent (per-user) export NFS on privileged ports (<1024)? If not, use port 2049 mapped via `pf` or use `nfsd` which runs as root.
- [ ] Does macFUSE work under macOS System Integrity Protection on Sequoia? (Requires kernel extension approval.)
- [ ] Is `brctl download` reliable enough, or must we use the FileProvider API directly?
- [ ] Re-eviction: macOS may evict a file that was recently hydrated if disk pressure occurs. Need to handle this gracefully (re-enter DOWNLOADING state).

---

## ADR Log

| # | Decision | Date | Status |
|---|---|---|---|
| 001 | Use FUSE passthrough as hydration shim | 2026-04-05 | Proposed |
| 002 | Start with macOS nfsd, migrate to ganesha if needed | 2026-04-05 | Proposed |
| 003 | Python for initial implementation (pyfuse3) | 2026-04-05 | Proposed |
