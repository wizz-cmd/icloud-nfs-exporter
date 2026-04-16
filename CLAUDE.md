# icloud-nfs-exporter

## Project Goal

A macOS service (Intel & Apple Silicon) that exports iCloud Drive folders via NFS, transparently handling all iCloud-specific behavior. NFS clients can access any file — whether it's already local or still in iCloud — without any manual intervention.

## Core Requirements

- **iCloud transparency**: Files may be evicted (stub/placeholder on disk), downloading, or fully local. The service must detect each state and trigger on-demand download before serving the file to NFS clients.
- **NFS export**: Standard NFS v3/v4 export, compatible with Linux, macOS, and other POSIX NFS clients.
- **macOS service**: Runs as a `launchd` agent/daemon, auto-starts on login or boot.
- **Supports Intel and Apple Silicon Macs**.

## Guiding Principles

- **FOSS & digital independence**: All dependencies must be open-source. No lock-in to proprietary tools beyond the macOS APIs that are strictly necessary.
- **Robust & fault-tolerant**: Handle iCloud errors, network outages, partial downloads, and NFS client disconnects gracefully. Never corrupt data.
- **Outstanding UX**: Clear status reporting, sane defaults, helpful error messages. Setup should take minutes, not hours.
- **Reuse existing work**: Research what already exists (FUSE, NFS servers, iCloud CLI tools, etc.) before building from scratch.
- **Document architecture**: All significant design decisions are recorded in `ARCHITECTURE.md` with pattern names, descriptions, and references.
- **Preferred languages**:
  - Prio 1 (scripting/glue): Python, Perl, bash
  - Prio 2 (macOS-native / system frameworks): Swift — preferred for any component that touches Apple APIs (FileProvider, FSEvents, launchd, entitlements)
  - Prio 3 (performance-critical / cross-platform system): Rust, C++, C — preferred for the FUSE driver layer (macFUSE exposes a C API; Rust's `fuser` crate is the best binding)

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full architecture, design patterns, and references.

## Repository Layout (planned)

```
icloud-nfs-exporter/
├── CLAUDE.md           # This file — project context for AI-assisted development
├── ARCHITECTURE.md     # Architecture decisions, patterns, references
├── README.md           # User-facing documentation
├── src/                # Source code
├── scripts/            # Install / setup scripts
├── launchd/            # launchd plist templates
└── tests/              # Test suite
```

## Key Technical Challenges

1. **iCloud file state detection** — files can be in one of several states (local, evicted, downloading, error). Must use `xattr` / `NSFileProviderManager` / `brctl` to detect and trigger downloads.
2. **On-demand hydration** — when an NFS client opens an evicted file, the service must block the open, trigger `brctl download` or equivalent, wait for completion, then serve the file.
3. **FUSE or NFS shim** — a FUSE filesystem or NFS re-export layer sits between iCloud Drive and the NFS server, intercepting VFS calls to trigger hydration.
4. **Caching & consistency** — avoid redundant downloads; keep NFS file handles stable across hydration cycles.
5. **macOS SIP / sandbox constraints** — the service must work within macOS security boundaries.

## Session Rules

- **Start of every session**: Read `HANDOVER.md` before doing anything else. It contains the current project state, what works, what's broken, and the immediate next step.
- **After every commit**: Update `HANDOVER.md` to reflect what changed — especially the "What Works", "What Does NOT Work Yet", "Immediate Next Step", and "Version" sections. Keep it concise and current.

## Development Guidelines

- Always check what already exists before writing new code (brctl, macFUSE, nfs-ganesha, etc.).
- Architecture patterns must be named, described, and sourced in `ARCHITECTURE.md`.
- Commits follow Conventional Commits format.
- All scripts must be POSIX-compatible where possible; use `#!/usr/bin/env python3` or `#!/bin/bash`.
- Test on both Intel (x86_64) and Apple Silicon (arm64).
