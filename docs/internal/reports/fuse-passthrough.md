# Implementation Report — FUSE Passthrough Filesystem

**Feature:** `fuser::Filesystem` passthrough with stub translation and hydration interception
**Date completed:** 2026-04-16
**CI run:** Pending (local: 43 tests pass — 21 fuse-core + 6 passthrough + 16 doc tests)

---

## Objective

Implement the missing FUSE mount that sits between iCloud Drive and NFS clients. The passthrough filesystem mirrors an iCloud Drive folder, translates `.icloud` stub filenames to real names in directory listings, and triggers on-demand hydration when evicted files are opened.

## Prerequisites resolved this session

- **Rust toolchain** installed (1.94.1 via rustup, `~/.cargo/env` sourced in `.zshrc`)
- **macFUSE 5.1.3** confirmed installed; kext approval prompt not appearing on macOS 15.7, so using **FSKit backend** (`-o backend=fskit`) instead — runs in user space, no kext needed, mounts restricted to `/Volumes`

## Files changed

| File | Lines | Change |
|------|-------|--------|
| `src/fuse/fuse-driver/src/passthrough.rs` | 602 | **NEW** — Core filesystem implementation |
| `src/fuse/fuse-driver/src/main.rs` | 151 | Added `mount` subcommand (+49 lines) |
| `src/fuse/fuse-driver/Cargo.toml` | 11 | Added fuser, log, env_logger, libc |

No changes to `fuse-core` — all existing types and utilities reused as-is.

## Architecture

```
fuse-driver mount <source> [mountpoint]
       |
       v
+------------------------------------------+
|  IcloudFs (fuser::Filesystem)            |
|                                          |
|  inodes: RwLock<HashMap<u64, InodeData>> |  <- inode table (path + refcount)
|  handles: RwLock<HashMap<u64, File>>     |  <- open file descriptors
|  ipc: IpcClient                          |  <- hydration daemon connection
|  next_ino / next_fh: AtomicU64           |  <- lock-free allocators
+----------+-----------+-------------------+
           |           |
      readdir()     open()
           |           |
           v           v
    Stub translation   Hydration interception
    .Report.pdf.icloud   IpcClient::hydrate()
      -> Report.pdf      blocks until download
                         then opens real file
```

## Components

### IcloudFs struct (`passthrough.rs`)

| Field | Type | Purpose |
|-------|------|---------|
| `source` | `PathBuf` | Root iCloud Drive folder being mirrored |
| `ipc` | `IpcClient` | Blocking IPC to hydration daemon |
| `inodes` | `RwLock<HashMap<u64, InodeData>>` | Inode-to-path mapping (read-heavy) |
| `handles` | `RwLock<HashMap<u64, HandleData>>` | Open file descriptor table |
| `next_ino` / `next_fh` | `AtomicU64` | Lock-free ID allocation |
| `uid` / `gid` | `u32` | Mounting user (from `getuid`/`getgid`) |

Constructor seeds inode 1 as root mapped to the source directory.

### Filesystem trait methods implemented

| Method | Purpose |
|--------|---------|
| `init` / `destroy` | Lifecycle logging |
| `lookup` | Resolves child name under parent; tries literal name, then `.name.icloud` stub |
| `forget` | Decrements lookup refcount, evicts inode when it hits 0 |
| `getattr` | Stats the real path, returns `FileAttr` |
| `opendir` / `releasedir` | No-op handles (readdir re-reads each call for freshness) |
| `readdir` | Lists directory with stub translation and deduplication |
| `open` | **Hydration interception point** — detects stubs, calls `IpcClient::hydrate()`, updates inode path |
| `read` | `pread` via `File::read_at` (thread-safe, no seeking) |
| `release` | Closes file descriptor, removes from handle table |
| `statfs` | Passes through `libc::statfs` from source filesystem |

### Stub translation (`readdir` + `lookup`)

- `readdir`: `.Name.icloud` entries appear as `Name`. If both a stub and real file exist for the same name, only the real file is shown.
- `lookup`: When resolving a name, tries the literal path first. If not found, tries the `.name.icloud` stub form. The inode's `real_path` records whichever path actually exists on disk.

### Hydration interception (`open`)

1. Check write flags — return `EROFS` (read-only mount)
2. If the inode's path is a stub (`is_icloud_stub`): call `IpcClient::hydrate()`, which blocks until the Swift daemon downloads the file (up to 300s)
3. On success: update the inode table to point to the hydrated path, open the real file
4. On failure (daemon down, timeout, error): return `EIO` with warning log

### Helper functions

| Function | Purpose |
|----------|---------|
| `meta_to_attr` | Converts `std::fs::Metadata` to `fuser::FileAttr` with correct uid/gid/nlink |
| `resolve_child` | Finds a child path under a parent, checking both literal and stub forms |

## Dependencies added

| Crate | Version | Purpose |
|-------|---------|---------|
| `fuser` | 0.17 (`macfuse-4-compat`) | FUSE filesystem trait + mount API |
| `log` | 0.4 | Structured logging (fuser also uses this) |
| `env_logger` | 0.11 | `RUST_LOG=debug` for hydration events |
| `libc` | 0.2 | POSIX constants, `statfs`, `getuid`/`getgid` |

## Reused from fuse-core (unchanged)

- `IpcClient` — blocking IPC to hydration daemon
- `is_icloud_stub()` — stub detection in readdir + open
- `stub_to_real_name()` — name translation in readdir
- `real_to_stub_name()` — reverse lookup in resolve_child
- `FileState` enum

## Design decisions

**TTL = 1 second**: Conservative attribute caching since iCloud can change file state at any time (eviction, sync).

**RwLock over Mutex for tables**: Directory browsing is read-heavy (many concurrent `getattr`/`lookup`/`readdir`). Writes (inode allocation, file open/close) are infrequent.

**No directory handle caching**: `readdir` re-reads the real directory each call. Simpler and avoids stale entries from iCloud sync changes.

**FSKit backend via mount option**: `MountOption::CUSTOM("backend=fskit")` bypasses the macFUSE kext entirely. Limitation: mounts must be under `/Volumes`.

**Blocking hydration in open**: NFS clients expect `open` to block until the file is ready. The 300s timeout matches the daemon's own timeout.

## Test results

```
43 tests pass (21 fuse-core + 6 passthrough + 16 doc tests)

New tests:
  resolve_child_real_file    - finds literal file
  resolve_child_stub_file    - finds .icloud stub when asked for real name
  resolve_child_not_found    - returns None
  meta_to_attr_file          - correct FileAttr for regular file
  meta_to_attr_dir           - correct FileAttr for directory
  icloud_fs_new_seeds_root   - constructor seeds inode 1
```

## CLI usage

```bash
# Mount iCloud Drive (blocks until unmount)
fuse-driver mount ~/Library/Mobile\ Documents/com~apple~CloudDocs

# Custom socket and mountpoint
fuse-driver mount -s /tmp/custom.sock /path/to/source /Volumes/custom-mount

# Debug logging
RUST_LOG=debug fuse-driver mount ~/Library/Mobile\ Documents/com~apple~CloudDocs

# Unmount
umount /Volumes/icloud-nfs-exporter
```

## Not implemented (intentional)

- **Write support** — RO mount, write operations return `ENOSYS`/`EROFS`
- **Symlink readlink** — returns `ENOSYS`
- **Extended attributes** — not passed through
- **NFS export wiring** — next step per HANDOVER.md

## Next steps

1. Manual end-to-end test: mount, ls, open evicted file, verify hydration
2. Wire NFS export to the FUSE mountpoint
3. Update CI to build with fuser dependency
