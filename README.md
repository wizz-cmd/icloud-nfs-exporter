# icloud-nfs-exporter

Export iCloud Drive folders as NFS shares on macOS — transparently handling on-demand file hydration so NFS clients never see a missing or stub file.

## What it does

Runs as a macOS `launchd` service that:

1. Mounts a FUSE passthrough filesystem over your iCloud Drive folder(s).
2. Intercepts `open(2)` / `read(2)` calls for evicted (cloud-only) files and triggers an on-demand download via FileProvider API.
3. Exports the hydrating FUSE mount via NFS (using macOS built-in `nfsd`).

NFS clients (Linux, macOS, BSD, ...) can read any file as if it were local — downloads happen transparently and on demand.

## Requirements

- macOS 13 Ventura or later (Intel or Apple Silicon)
- Xcode Command Line Tools (`xcode-select --install`)
- [macFUSE](https://osxfuse.github.io/) for the FUSE passthrough driver
- Python 3.11+
- iCloud Drive enabled

## Installation

### From source

```bash
git clone https://github.com/wizz-cmd/icloud-nfs-exporter.git
cd icloud-nfs-exporter
make build-release
sudo make install
```

### From a release tarball

Download the latest release from [GitHub Releases](https://github.com/wizz-cmd/icloud-nfs-exporter/releases), then:

```bash
tar xzf icloud-nfs-exporter-v*.tar.gz -C /usr/local/
```

### Uninstall

```bash
sudo make uninstall
```

## Quick Start

```bash
# Run the interactive setup wizard
icne setup

# Or non-interactive with defaults (auto-selects iCloud Drive)
icne setup --non-interactive

# Check system status
icne diagnose

# List configured folders
icne list

# Preview NFS exports without applying
icne exports --dry-run
```

## CLI Reference

| Command | Description |
|---|---|
| `icne setup` | Interactive setup wizard (prerequisites, folder selection, NFS config) |
| `icne add-folder <path>` | Add an iCloud folder to export |
| `icne remove-folder <path>` | Remove a folder from exports |
| `icne list` | List configured folders and mount points |
| `icne diagnose` | Run diagnostic checks |
| `icne exports [--dry-run]` | Show or apply NFS exports to `/etc/exports` |

## Components

| Component | Language | Description |
|---|---|---|
| HydrationDaemon | Swift | Detects iCloud file states, triggers on-demand downloads |
| fuse-driver | Rust | FUSE passthrough filesystem with hydration interception |
| MenuBarApp | Swift | Menu bar status item showing daemon health |
| icne | Python | CLI tool for setup, configuration, and diagnostics |

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full architecture, design patterns, and references.

## Development

```bash
# Build (debug)
make build

# Run tests (Swift + Rust + Python)
make test

# Lint
make lint

# Build optimised universal binaries
bash scripts/build-release.sh dist/
```

## License

MIT
