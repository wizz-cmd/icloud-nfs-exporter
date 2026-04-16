# Contributing

Thanks for your interest in icloud-nfs-exporter! This document covers the basics.

## Development Setup

```bash
git clone https://github.com/wizz-cmd/icloud-nfs-exporter.git
cd icloud-nfs-exporter
make build
make test
```

**Requirements:**
- macOS 14+ with **Xcode** (full install, not just Command Line Tools — needed for XCTest)
- Rust (via [rustup](https://rustup.rs/))
- Python 3.11+
- [macFUSE](https://osxfuse.github.io/) (optional, for FUSE driver)

Run `make help` to see all available targets.

## Project Structure

| Directory | Language | Purpose |
|---|---|---|
| `src/hydration/` | Swift (SPM) | Hydration daemon + core library |
| `src/fuse/` | Rust (Cargo) | FUSE driver + IPC client |
| `src/app/` | Swift (SPM) | Menu bar app (SwiftUI) |
| `src/helper/` | Swift (SPM) | Privileged helper (stub) |
| `scripts/` | Python | CLI tool (`icne`) + library |
| `tests/` | Python | Python test suite |
| `docs/reports/` | Markdown | Implementation reports per milestone |
| `docs/research/` | Markdown | Design research and guidelines |

## Making Changes

1. Create a branch from `main`.
2. Make your changes. Follow existing code style.
3. Run `make test` and ensure all tests pass (62 tests: Swift + Rust + Python).
4. Commit using [Conventional Commits](https://www.conventionalcommits.org/) format.
5. Open a pull request using the PR template.

## Commit Messages

Format: `type: short description`

Types: `feat`, `fix`, `docs`, `test`, `refactor`, `chore`.

## Testing

```bash
make test        # Run all test suites (Swift + Rust + Python)
make lint        # Lint all languages (swiftlint + clippy + ruff)
make help        # Show all Makefile targets
```

**Note:** Swift tests (`swift test`) require full Xcode, not just Command Line Tools. The `XCTest` framework is only available with Xcode installed. CI uses macOS 15 runners with Xcode pre-installed.

Python tests can run independently: `python3 -m unittest discover -s tests -v`

## Releases

Releases are automated via GitHub Actions:

1. Update version strings in `HydrationCore.swift`, `scripts/icne`, `Cargo.toml` files.
2. Add a CHANGELOG entry.
3. Tag: `git tag -a v0.X.Y -m "v0.X.Y — description"`
4. Push: `git push origin v0.X.Y`
5. The [Release workflow](.github/workflows/release.yml) builds a `.dmg`, runs tests, and publishes a GitHub Release.

## Code of Conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md). Be kind and constructive.
