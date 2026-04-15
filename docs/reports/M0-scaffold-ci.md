# M0 Implementation Report — Scaffold + CI

**Milestone:** M0 (Scaffold + CI)
**Date completed:** 2026-04-15
**Commit:** `23ee8dc` on `main`
**CI run:** Passed (46s on macOS 15)

---

## Objective

Create the full project scaffold with build tooling and continuous integration so that all subsequent milestones have a working foundation to build on.

## Deliverables

### Source packages

| Package | Path | Type | Language | Targets |
|---|---|---|---|---|
| Hydration | `src/hydration/` | SPM | Swift | `HydrationCore` (lib), `HydrationDaemon` (exe), `HydrationTests` (test) |
| FUSE driver | `src/fuse/` | Cargo workspace | Rust | `fuse-core` (lib), `fuse-driver` (bin) |
| Menu bar app | `src/app/` | SPM | Swift | `MenuBarApp` (exe) |
| Privileged helper | `src/helper/` | SPM | Swift | `PrivilegedHelper` (exe) |

All targets are stubs that compile and link but contain no business logic. Test targets include a single version-check assertion to verify the build and test pipelines work end-to-end.

### CLI tool

`scripts/icne` — Python 3 CLI using `argparse` with three subcommands:

- `setup` — initial setup and configuration
- `add-folder` — add an iCloud folder to export
- `diagnose` — run diagnostics

All subcommands print "not yet implemented" and exit cleanly.

### launchd template

`launchd/com.wizz-cmd.icloud-nfs-exporter.plist.template` — a `LaunchAgent` plist with `__USERNAME__` placeholders for log paths and working directory. Configured with `RunAtLoad` and `KeepAlive` for automatic restart.

### Makefile

Four targets:

| Target | Commands |
|---|---|
| `build` | `swift build` (×3 packages) + `cargo build` |
| `test` | `swift test` (hydration) + `cargo test` |
| `lint` | `swiftlint` + `cargo clippy` + `ruff` |
| `clean` | `swift package clean` (×3) + `cargo clean` |

### CI workflow

`.github/workflows/ci.yml` — GitHub Actions workflow:

- **Runner:** `macos-15` (includes Xcode + Swift)
- **Rust:** installed via `dtolnay/rust-toolchain@stable`
- **Steps:** `make build`, `make test`
- **Triggers:** push and PR to `main`

### Other changes

- `.gitignore` updated with Swift build artifacts (`.build/`, `.swiftpm/`)

## Verification

| Check | Result |
|---|---|
| `swift build --package-path src/hydration` | Pass |
| `swift build --package-path src/app` | Pass |
| `swift build --package-path src/helper` | Pass |
| `cargo build` (src/fuse) | Pass (CI) |
| `swift test --package-path src/hydration` | Pass (CI) |
| `cargo test` (src/fuse) | Pass (CI) |
| `scripts/icne --help` | Pass |
| `scripts/icne setup` | Pass |
| GitHub Actions CI | Pass (46s) |

Local Swift builds were verified directly. Rust builds and all tests were verified via CI (Rust is not installed on the development machine).

## Architecture alignment

The scaffold follows the language assignment from ARCHITECTURE.md (ADR 004):

- **Swift** for components touching Apple APIs (hydration daemon, menu bar app, privileged helper)
- **Rust** for the FUSE driver layer
- **Python** for scripting/glue (CLI tool)

## Next milestone

**M1 — Swift hydration daemon**: implement iCloud file state detection via `NSFileProviderManager`, FSEvents watcher, and the `brctl download` hydration trigger in `src/hydration/`.
