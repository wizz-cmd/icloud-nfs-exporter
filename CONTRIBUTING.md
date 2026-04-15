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
- macOS 13+ with Xcode Command Line Tools
- Rust (via [rustup](https://rustup.rs/))
- Python 3.11+
- [macFUSE](https://osxfuse.github.io/) (optional, for FUSE driver)

## Project Structure

| Directory | Language | Purpose |
|---|---|---|
| `src/hydration/` | Swift (SPM) | Hydration daemon + core library |
| `src/fuse/` | Rust (Cargo) | FUSE driver + IPC client |
| `src/app/` | Swift (SPM) | Menu bar app |
| `src/helper/` | Swift (SPM) | Privileged helper |
| `scripts/` | Python | CLI tool (`icne`) |
| `tests/` | Python | Python test suite |

## Making Changes

1. Create a branch from `main`.
2. Make your changes. Follow existing code style.
3. Run `make test` and ensure all tests pass.
4. Commit using [Conventional Commits](https://www.conventionalcommits.org/) format.
5. Open a pull request.

## Commit Messages

Use the format: `type: short description`

Types: `feat`, `fix`, `docs`, `test`, `refactor`, `chore`.

## Testing

```bash
make test        # Run all test suites
make lint        # Lint all languages
```

Swift tests require Xcode (not just command-line tools) for XCTest support.

## Code of Conduct

Be kind and constructive. We're all here to build something useful.
