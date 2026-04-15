# iCloud NFS Exporter

Export iCloud Drive folders as NFS shares on macOS — transparently handling on-demand file hydration so NFS clients never see a missing or stub file.

## What it does

Runs as a macOS service that:

1. Mounts a FUSE passthrough filesystem over your iCloud Drive folder(s).
2. Intercepts `open(2)` / `read(2)` calls for evicted (cloud-only) files and triggers an on-demand download via FileProvider API.
3. Exports the hydrating FUSE mount via NFS (using macOS built-in `nfsd`).

NFS clients (Linux, macOS, BSD, ...) can read any file as if it were local — downloads happen transparently and on demand.

## Requirements

- macOS 14 Sonoma or later (Intel or Apple Silicon)
- [macFUSE](https://osxfuse.github.io/) for the FUSE passthrough driver
- Python 3.11+
- iCloud Drive enabled in System Settings

## Installation

### Download the app

1. Download **iCloud-NFS-Exporter-v*.dmg** from the [latest release](https://github.com/wizz-cmd/icloud-nfs-exporter/releases/latest).
2. Open the `.dmg` and drag **iCloud NFS Exporter** to your **Applications** folder.
3. On first launch, macOS may block the app because it is not code-signed. To allow it:
   - **Right-click** the app and choose **Open**, then click **Open** in the dialog, or
   - Run in Terminal: `xattr -cr "/Applications/iCloud NFS Exporter.app"`
4. Open the app — it will appear in your menu bar (cloud icon, top right).

### Install the command-line tool (optional)

To use the `icne` CLI for setup and diagnostics, open Terminal and run:

```bash
/Applications/iCloud\ NFS\ Exporter.app/Contents/MacOS/install-cli
```

This creates a symlink at `/usr/local/bin/icne` pointing into the app bundle.

### Build from source

```bash
git clone https://github.com/wizz-cmd/icloud-nfs-exporter.git
cd icloud-nfs-exporter

# Build the .app bundle
bash scripts/create-app-bundle.sh dist

# Or create a .dmg
bash scripts/create-dmg.sh dist
```

Requires Xcode Command Line Tools and Rust (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`).

### Uninstall

1. Quit the app from the menu bar (cloud icon > Quit).
2. Delete **iCloud NFS Exporter** from your Applications folder.
3. Optionally remove the CLI symlink: `sudo rm /usr/local/bin/icne`
4. Optionally remove config: `rm -r ~/.config/icloud-nfs-exporter`

## Getting Started

After installing, run the setup wizard:

```bash
icne setup
```

The wizard walks you through:
1. Checking prerequisites (macFUSE, iCloud Drive, NFS)
2. Selecting which iCloud folders to export
3. Configuring which network can access the NFS shares
4. Installing the LaunchAgent for auto-start

Or use `--non-interactive` to accept all defaults:

```bash
icne setup --non-interactive
```

Check system status at any time:

```bash
icne diagnose
```

## CLI Reference

| Command | Description |
|---|---|
| `icne setup` | Interactive setup wizard |
| `icne add-folder <path>` | Add an iCloud folder to export |
| `icne remove-folder <path>` | Remove a folder |
| `icne list` | List configured folders and mount points |
| `icne diagnose` | Run diagnostic checks |
| `icne exports [--dry-run]` | Show or apply NFS exports |
| `icne --version` | Print version |

## Components

| Component | Language | Description |
|---|---|---|
| iCloud NFS Exporter (app) | Swift | Menu bar status item with daemon health monitoring |
| HydrationDaemon | Swift | Detects iCloud file states, triggers on-demand downloads |
| fuse-driver | Rust | FUSE passthrough filesystem with hydration interception |
| icne | Python | CLI for setup, configuration, and diagnostics |

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full design, patterns, and references.

## Development

```bash
make build       # Debug build
make test        # Run all tests (Swift + Rust + Python)
make lint        # Lint all languages
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

## License

[MIT](LICENSE)
