# M6 Implementation Report — Distribution

**Milestone:** M6 (Distribution)
**Date completed:** 2026-04-15
**Commit:** `d16deca` on `main`
**CI run:** Passed (52s on macOS 15) — 62 tests

---

## Objective

Package the project for installation from source, from release tarballs, and (future) via Homebrew.

## Deliverables

### Makefile Targets

| Target | Description |
|---|---|
| `build` | Debug build (Swift + Rust) |
| `build-release` | Optimised release build |
| `test` | Run all test suites (Swift + Rust + Python) |
| `lint` | SwiftLint + Cargo Clippy + Ruff |
| `install` | Build release and install to `PREFIX` (default `/usr/local`) |
| `uninstall` | Remove installed files (preserves user config + LaunchAgent) |
| `clean` | Remove build artifacts |

**Install layout:**

```
/usr/local/
├── bin/
│   ├── HydrationDaemon          # Swift hydration daemon
│   ├── icloud-nfs-exporter-app  # Menu bar app
│   └── icne -> ../share/.../icne  # CLI symlink
└── share/icloud-nfs-exporter/
    ├── scripts/
    │   ├── icne                 # Python CLI
    │   └── icne_lib/            # CLI library modules
    └── launchd/
        └── *.plist.template     # LaunchAgent template
```

### Release Build Script (`scripts/build-release.sh`)

Builds universal (arm64 + x86_64) binaries for macOS distribution.

- **Swift packages**: uses `swift build --arch arm64 --arch x86_64` for fat binaries
- **Rust workspace**: if both targets are installed via rustup, builds each and joins with `lipo`; otherwise builds native
- **CLI + templates**: copied to `dist/share/`
- **Output**: everything in `dist/` ready for tarball or `make install`

### Homebrew Formula (`Formula/icloud-nfs-exporter.rb`)

Complete Homebrew formula including:

- Source build from tagged release tarball
- Depends on macOS, Xcode 15+, Rust, Python 3.11
- Installs HydrationDaemon, MenuBarApp, fuse-driver, and CLI
- `brew services` integration (start/stop HydrationDaemon)
- Caveats with setup instructions
- Test block verifying `--version` and `--help`

SHA256 is a placeholder — updated automatically per release.

### GitHub Release Workflow (`.github/workflows/release.yml`)

Triggered on tag push (`v*`):

1. Checkout + install Rust
2. Run `scripts/build-release.sh`
3. Run `make test` to verify release build
4. Create tarball + SHA256 checksum
5. Publish GitHub Release with auto-generated notes and artifacts

Uses `softprops/action-gh-release@v2` for release creation.

### README.md

Rewritten with:

- Installation instructions (source, tarball, uninstall)
- Quick start guide (`icne setup`)
- CLI reference table (all 6 commands)
- Component table (4 components, languages, descriptions)
- Development section (build, test, lint, release commands)

## Architecture alignment

- **Convention over Configuration** (ARCHITECTURE.md §6) — `make install` uses standard `/usr/local` prefix; `icne setup` auto-configures everything else
- **Daemon / Activator** (ARCHITECTURE.md §5) — Homebrew formula includes `brew services` support for the launchd-managed daemon

## Next milestone

**M7 — Polish**: code quality improvements, documentation refinements, and preparation for first release tag.
