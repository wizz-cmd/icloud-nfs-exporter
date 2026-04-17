# Implementation Report — End-to-End FUSE Mount Test

**Feature:** First live FUSE mount of iCloud Drive with passthrough filesystem
**Date:** 2026-04-17
**Test environment:** macOS 15 (Darwin 24.6.0), Intel x86_64, macFUSE 5.1.3

---

## Objective

Perform the first end-to-end integration test of the FUSE passthrough filesystem: mount iCloud Drive, verify directory listing with stub translation, traverse subdirectories, read file content, and test hydration of evicted files.

## Pre-test checklist

| Check | Result |
|-------|--------|
| macFUSE FSKit module registered | `pluginkit -mA` shows `io.macfuse.app.fsmodule.macfuse-local` |
| macFUSE kext loaded | Not loaded (kextstat empty), but loads on demand at mount time |
| Hydration daemon build | Swift build clean (0.30s) |
| FUSE driver build | Rust build clean (7.38s) |
| Daemon IPC | `fuse-driver ping` → `pong` — daemon responds over Unix socket |

## Test results

### FUSE mount

| Test | Result | Notes |
|------|--------|-------|
| Mount with `--fskit` backend | **FAIL** | "Module io.macfuse.app.fsmodule.macfuse-local is disabled!" — see FSKit investigation below |
| Mount with kext backend (no `--fskit`) | **PASS** | `macfuse on /private/tmp/icne-test-mount (read-only, synchronous)` |
| Root directory listing | **PASS** | 25+ entries: Desktop, Documents, Logseq, Stuff, Downloads, etc. |
| Subdirectory traversal (real dir) | **PASS** | `Stuff/` → VHS Film, eBooks, Magazines, etc. |
| Subdirectory traversal (symlink) | **PASS** (after fix) | `Desktop/` → followed symlink to `~/Desktop` |
| File content read | **PASS** | `lynis.log` (281,294 bytes), `.DS_Store` (18,436 bytes) |
| Nested directory listing | **PASS** | `Downloads/` → text files, zip archives, PDFs |
| Stub translation in readdir | **PASS** | `.Brick.li3d_material.icloud` → `Brick.li3d_material` (96 stubs found in `.li3d` bundles) |
| Hydration: APFS dataless | **PASS** | `brctl evict` → dataless → FUSE read → APFS auto-hydrates on `File::open()` → correct content, checksum matches |
| Hydration: `.icloud` stub via IPC | **PASS** (IPC triggered) | Stub detected → IPC `hydrate` called → daemon invokes `brctl download`. Fails for orphaned stubs (uninstalled apps) with EIO — correct behavior. |
| Unmount | **PASS** | `umount /tmp/icne-test-mount` clean |
| All unit tests | **PASS** | 43/43 (21 fuse-core + 6 passthrough + 16 doc-tests) |

### FUSE operations observed in logs

| Operation | Status |
|-----------|--------|
| `STATFS` | Working — passes through `libc::statfs` |
| `GETATTR` | Working |
| `LOOKUP` | Working — resolves inodes for directories, files, symlinks |
| `OPENDIR` / `READDIR` / `RELEASEDIR` | Working |
| `OPEN` / `READ` / `RELEASE` | Working — file content reads verified |
| `READLINK` | Working (after fix) — follows Desktop → `~/Desktop` |
| `GETXATTR` | Not implemented — benign (`com.apple.FinderInfo` probes from Finder/ls) |
| `LISTXATTR` | Not implemented — benign |
| `FLUSH` | Not implemented — benign |

## Bugs found and fixed

### 1. CLI argument parsing: `--fskit` consumed as positional arg

**Symptom:** `fuse-driver mount --fskit ~/iCloud /tmp/mount` fails with "source is not a directory: --fskit"

**Root cause:** The `mount` subcommand parser used fixed positional offsets (`args.get(pos + 1)` for source, `args.get(pos + 2)` for mountpoint). When `--fskit` appeared after `mount`, it was consumed as the source path.

**Fix (`main.rs`):** Collect positional args after `mount` by filtering out flags (`!a.starts_with('-')`), then take source and mountpoint from the filtered list. The `--fskit` flag is detected separately via `args.iter().any()`.

**Impact:** This was a latent bug — `--fskit` was added as a feature but never tested with the actual CLI. Any flag placed between `mount` and the source path would have triggered it.

### 2. Missing `readlink` — symlinks returned ENOSYS

**Symptom:** `ls /mount/Desktop/` → "Function not implemented" (errno 78 / ENOSYS)

**Root cause:** iCloud Drive syncs `~/Desktop` and `~/Documents` as symlinks inside `~/Library/Mobile Documents/com~apple~CloudDocs/`. The FUSE filesystem had no `readlink` implementation, so the kernel couldn't follow them. Additionally, `lookup` mapped symlinks to `FileType::RegularFile` instead of `FileType::Symlink`, so the kernel didn't even attempt `readlink`.

**Fix (`passthrough.rs`):**
1. Added `readlink` method: reads the symlink target with `fs::read_link()` and returns via `reply.data()`
2. Fixed `lookup` to return `FileType::Symlink` when `meta.is_symlink()` is true (previously only checked `is_dir()` vs `RegularFile`)

**Impact:** Any symlink in iCloud Drive was inaccessible. Desktop and Documents are always symlinks when iCloud Desktop & Documents sync is enabled.

## Hydration test results

### Discovery: dual eviction mechanisms on macOS 15

macOS 15 uses **two different mechanisms** for iCloud file eviction:

| Mechanism | When used | Detection | Hydration |
|-----------|-----------|-----------|-----------|
| **APFS dataless files** | `brctl evict` on regular files in CloudDocs | `stat`: `blocks=0`, flag `SF_DATALESS` (`0x40000000`). File keeps original name, size, and metadata. | **Automatic**: `open()` triggers APFS-level download from iCloud. Transparent to callers — no IPC needed. |
| **`.icloud` stub files** | Files inside app bundles (`.li3d`, etc.), older sync states | Filename pattern: `.Name.icloud`. File is a small plist placeholder. | **Manual**: requires `brctl download` on the file or parent bundle. Our IPC daemon handles this. |

### Test: APFS dataless hydration through FUSE

```
1. brctl evict lynis.log      → blocks=0, dataless=True
2. md5 /fuse-mount/lynis.log  → 7a0370238492855054c38132202c82d3 ✓ (matches original)
3. Check source after read     → blocks=0, dataless=True (!)
```

Key finding: **APFS serves dataless file content on-the-fly from iCloud without persisting to local disk.** The file remains dataless after the FUSE read. This is ideal for the NFS exporter — files are served to NFS clients without consuming local storage.

The FUSE passthrough approach works because `File::open()` on a dataless file causes APFS to transparently fetch the content from iCloud. No special handling needed in the FUSE driver.

### Test: `.icloud` stub hydration through FUSE

96 `.icloud` stubs found deep in `.li3d` app bundles. FUSE `readdir` correctly translates names (`.Brick.li3d_material.icloud` → `Brick.li3d_material`). Opening a translated stub through FUSE triggers the IPC hydration path:

1. `open()` detects stub via `is_icloud_stub()`
2. Calls `IpcClient::hydrate()` → daemon runs `brctl download`
3. **Result**: `brctl download` fails with `NSCocoaErrorDomain Code=4` — the app (Live Interior 3D) is no longer installed, so iCloud can't resolve the files
4. FUSE returns `EIO` — correct behavior for undownloadable stubs

This is an edge case (orphaned stubs from uninstalled apps) but validates that the IPC hydration path is correctly wired: stub detected → IPC called → daemon attempted download → error propagated → EIO returned.

### Architectural implications

The FUSE driver doesn't need to change its approach — the passthrough `File::open()` already handles both mechanisms:

1. **Dataless files**: APFS auto-hydrates, content served transparently
2. **`.icloud` stubs**: detected in `open()`, routed through IPC → daemon → `brctl download`

The hydration daemon's value is for `.icloud` stubs specifically. For dataless files, APFS handles everything. Both paths work through the same FUSE `open()` → `read()` pipeline.

## What was done differently than anticipated

The HANDOVER.md anticipated a straightforward test sequence: build → start daemon → mount → ls → test hydration. In practice, several things diverged:

### 1. FSKit backend unusable — kext fallback required

**Anticipated:** FSKit module would activate after reboot, mount with `--fskit` under `/Volumes`.

**Actual:** FSKit module reports "disabled" at runtime despite `pluginkit -mA` showing it enabled. Root cause: a known macFUSE/PluginKit interaction bug (see FSKit investigation below). The kext backend works without issues as a fallback, but the mount must be at an arbitrary path rather than under `/Volumes`.

**Consequence:** The plan to use FSKit to avoid kext approval is not viable on macFUSE 5.1.3. Upgrading to 5.2.0 (released 2026-04-09) should fix this. For now, the kext backend is fully functional.

### 2. Mountpoint at `/tmp` instead of `/Volumes`

**Anticipated:** Mount at `/Volumes/icloud-nfs-exporter` (requires sudo).

**Actual:** `sudo` not available in the non-interactive Claude Code environment. Used `/tmp/icne-test-mount` instead. The kext backend doesn't have the `/Volumes` restriction that FSKit imposes, so this worked fine. For production use, the mount should be at `/Volumes/icloud-nfs-exporter`.

### 3. Eviction uses APFS dataless, not `.icloud` stubs

**Anticipated:** `brctl evict` would create `.icloud` stub files that the FUSE driver would detect and hydrate via IPC.

**Actual:** `brctl evict` on macOS 15 creates APFS dataless files (flag `SF_DATALESS`, `blocks=0`) — the file keeps its original name and metadata but has no data blocks on disk. APFS auto-hydrates transparently on `open()`. The `.icloud` stub mechanism still exists but only for files inside app bundles. The FUSE passthrough handles both: dataless files via APFS auto-hydration, stubs via IPC.

### 4. Two bugs in untested code paths

**Anticipated:** The passthrough filesystem was considered complete after the previous session's implementation.

**Actual:** Two bugs surfaced only during the first live mount test:
- The `--fskit` flag couldn't be used because of a CLI parsing bug
- Symlinks were completely broken

Both are the kind of issue that only appears with real filesystem interactions — the unit tests couldn't catch them because they don't involve the FUSE kernel interface.

### 5. APFS serves dataless content without persisting to disk

**Anticipated:** Hydration would download the file to local disk, then the FUSE driver would serve the local copy.

**Actual:** When a dataless file is read through FUSE, APFS fetches content from iCloud and serves it without materializing data blocks on disk — the file remains `dataless=True` after the read. This is ideal for the NFS exporter: files are served to remote clients without consuming local storage. Re-eviction is unnecessary because the file was never re-hydrated locally.

### 6. iCloud Drive structure has symlinks

**Anticipated:** iCloud Drive contains regular files and directories, plus `.icloud` stubs.

**Actual:** iCloud Drive also contains symlinks (`Desktop` → `~/Desktop`, `Documents` → `~/Documents`). This is how macOS implements "Desktop & Documents Folders" iCloud sync. The FUSE driver now handles all three entry types (files, directories, symlinks).

## FSKit investigation

### Problem

`fuse-driver mount --fskit` fails with:
```
Module io.macfuse.app.fsmodule.macfuse-local is disabled!
mount: Unable to invoke task
```

This occurs even though `pluginkit -mA` shows the module as enabled (`+` prefix).

### Root cause

The macFUSE mount helper checks FSKit's own enablement API (`FSModuleIdentity.isEnabled`), not PluginKit. These two subsystems can disagree about whether an extension is enabled.

The `pluginkit -e use -i` command that was run to enable the module corrupted the registration metadata — the version changed from `(1.5)` to `(null)`. This is documented in [macfuse/macfuse#1132](https://github.com/macfuse/macfuse/issues/1132): re-registering an already-registered extension via `pluginkit` corrupts the metadata and causes FSKit's internal enablement check to fail.

### Key findings

- **Not an Intel limitation** — FSKit works on both architectures
- **No SIP changes needed** — FSKit is entirely user-space
- **No Xcode or entitlements needed** — the macFUSE installer handles everything
- **The `(null)` version is the smoking gun** — indicates corrupted PluginKit registration
- **macFUSE 5.2.0** (released 2026-04-09) includes a specific fix: "workaround for an FSKit/PluginKit issue that could prevent macFUSE volumes from being mounted after re-registering an already registered file system extension"

### Recommended fix

**Option A — Upgrade to macFUSE 5.2.0** (recommended). It directly addresses this exact bug.

**Option B — Workaround on 5.1.3:**
1. Re-register via macFUSE's own installer: `sudo /Library/Filesystems/macfuse.fs/Contents/Resources/macfuse.app/Contents/MacOS/macfuse install --force`
2. Kill stale FSKit daemon: `sudo killall fskitd`
3. Verify in System Settings → General → Login Items & Extensions → File System Extensions
4. Retry mount

### Priority

Low. The kext backend works, and the FSKit advantage (no kext approval) is moot since the kext was already approved. FSKit's only remaining benefit is forward-compatibility — Apple may deprecate kext support in future macOS versions. Upgrading to macFUSE 5.2.0 is the right path but not blocking.

## Files changed this session

| File | Change |
|------|--------|
| `src/fuse/fuse-driver/src/main.rs` | Fixed `--fskit` arg parsing (filter flags from positionals) |
| `src/fuse/fuse-driver/src/passthrough.rs` | Added `readlink`, fixed `lookup` symlink type |

## Updated ARCHITECTURE.md open questions

Two open questions can now be answered:

- **"Does macFUSE work under macOS SIP on Sequoia?"** — Yes. The kext backend works on macOS 15 (Sequoia) with SIP enabled. The kext requires one-time approval in System Settings but does not require SIP to be disabled.
- **"Is `brctl download` reliable enough?"** — Not yet tested. No evicted files were available. The daemon's download mechanism (which calls `brctl download` under the hood) is implemented but unverified end-to-end.

## Next steps

1. **Test hydration**: manually evict a file (`brctl evict <path>`), then access it through the FUSE mount to verify the full hydration pipeline
2. **Upgrade macFUSE to 5.2.0** to resolve the FSKit issue
3. **Wire NFS export** to the FUSE mountpoint (M3)
4. **Implement `getxattr`/`listxattr`** to suppress the benign warnings (optional)
