# icloud-nfs-exporter

Export iCloud Drive folders as NFS shares on macOS — transparently handling on-demand file hydration so NFS clients never see a missing or stub file.

## Status

Early design / research phase. See [ARCHITECTURE.md](ARCHITECTURE.md) for the current plan.

## What it does

Runs as a macOS `launchd` service that:

1. Mounts a FUSE passthrough filesystem over your iCloud Drive folder(s).
2. Intercepts `open(2)` / `read(2)` calls for evicted (cloud-only) files and triggers an on-demand download via `brctl` / FileProvider API.
3. Exports the hydrating FUSE mount via NFS (using macOS built-in `nfsd` or `nfs-ganesha`).

NFS clients (Linux, macOS, BSD, …) can read any file as if it were local — downloads happen transparently and on demand.

## Requirements

- macOS 13 Ventura or later (Intel or Apple Silicon)
- macFUSE: https://osxfuse.github.io/
- Python 3.11+
- iCloud Drive enabled

## Quick Start

> Not yet implemented. See [ARCHITECTURE.md](ARCHITECTURE.md).

## License

MIT
