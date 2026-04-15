# M5 Implementation Report — Setup Wizard

**Milestone:** M5 (Setup wizard)
**Date completed:** 2026-04-15
**Commit:** `0baaed7` on `main`
**CI run:** Passed (54s on macOS 15) — 62 tests (17 Swift + 21 Rust + 24 Python)

---

## Objective

Replace the old non-interactive `icne setup` with a guided 5-step wizard that walks users through prerequisites, iCloud folder selection, network configuration, and service installation.

## Components

### iCloud Container Discovery (`scripts/icne_lib/icloud.py`)

Discovers available iCloud containers in `~/Library/Mobile Documents/`.

**Container ID mapping:**

| Container ID | Label |
|---|---|
| `com~apple~CloudDocs` | iCloud Drive |
| `com~apple~Numbers` | Numbers |
| `com~apple~Pages` | Pages |
| `com~apple~Keynote` | Keynote |
| `iCloud~com~vendor~AppName` | AppName |
| Other | directory name as-is |

- 10 pre-mapped Apple container IDs
- Third-party apps (`iCloud~...`) extract the app name from the last tilde-delimited segment
- Hidden directories and files are filtered out
- iCloud Drive is always sorted first in the list

### Interactive Wizard (`scripts/icne_lib/wizard.py`)

5-step guided setup:

| Step | Action |
|---|---|
| 1. Prerequisites | Runs 4 diagnostic checks (iCloud Drive, macFUSE, nfsd, Rust). Warns if any are missing; asks to continue. |
| 2. Folder selection | Lists discovered iCloud containers with numbered selection. Pre-marks already-configured folders with `*`. Accepts comma-separated numbers. |
| 3. Network config | Prompts for NFS allowed network in CIDR notation. Validates with `ipaddress.ip_network()`. |
| 4. Write config | Generates `config.toml`, creates mount point directories. Merges with existing config (unless `--force`). |
| 5. LaunchAgent | Optionally installs the launchd plist with `__USERNAME__` replaced. |

**Modes:**
- **Interactive** (default): prompts at each step with defaults in brackets
- **Non-interactive** (`--non-interactive`): auto-selects iCloud Drive, uses all defaults, no prompts — suitable for CI/scripting

### CLI Changes

`icne setup` now accepts:
- `--force` — overwrite existing config
- `--non-interactive` — skip all prompts, use defaults

## Tests

6 new tests in `TestIcloud`:

| Test | Coverage |
|---|---|
| `test_label_apple_containers` | Maps 4 known Apple container IDs to friendly names |
| `test_label_third_party` | Extracts app name from `iCloud~com~vendor~App` format |
| `test_label_unknown` | Unknown container names pass through unchanged |
| `test_discover_containers` | Discovery with mock directory: finds dirs, skips hidden/files |
| `test_discover_missing_dir` | Returns empty list for non-existent base path |
| `test_icloud_drive_sorted_first` | iCloud Drive always appears first regardless of sort |

## Verified locally

```
$ icne setup --non-interactive --force
  Step 1: Checking prerequisites
    [  OK]  iCloud Drive — ~/Library/Mobile Documents (227 items)
    [FAIL]  macFUSE — not installed
    [  OK]  NFS server (nfsd) — 0 export(s)
    [FAIL]  Rust toolchain — cargo not found
  Step 2: Select iCloud folders to export
    Auto-selected: iCloud Drive (com~apple~CloudDocs)
  Step 3: Network configuration
    NFS allowed network: 192.168.0.0/24
  Step 4: Writing configuration
    Config: ~/.config/icloud-nfs-exporter/config.toml
    Folders: 1 configured
  Step 5: LaunchAgent installation
    Installed: ~/Library/LaunchAgents/com.wizz-cmd...plist
  Setup complete!
```

## Architecture alignment

- **Convention over Configuration** (ARCHITECTURE.md §6) — wizard uses sensible defaults at every step; zero-input setup works via `--non-interactive`
- **Builder pattern** (ARCHITECTURE.md §6) — config is constructed step-by-step, each field either from user input or a default

## Next milestone

**M6 — Distribution**: packaging the project for installation via Homebrew formula or direct download (DMG/pkg), including code signing considerations.
