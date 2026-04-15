# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.1.0] - 2026-04-15

### Added

- **Hydration daemon** (Swift): iCloud file state detection via URL resource
  values and `.icloud` stub files. State machine (unknown/evicted/downloading/
  local/error) with validated transitions. FSEvents directory watcher with
  file-level and xattr change events. On-demand hydration via
  `FileManager.startDownloadingUbiquitousItem`. IPC server over Unix domain
  socket with length-prefixed JSON protocol.

- **FUSE driver core** (Rust): IPC client matching the daemon's wire format.
  `.icloud` stub path translation (detection, name conversion, round-trip).
  CLI tool for testing daemon connectivity (ping, query, hydrate).

- **NFS wiring** (Python): TOML configuration management. `/etc/exports`
  generation with managed block markers (preserves user entries). CIDR to
  network+mask conversion for macOS nfsd. Python IPC client. Seven diagnostic
  checks (iCloud Drive, config, daemon, macFUSE, nfsd, mount base, Rust).

- **Menu bar app** (Swift/AppKit): NSStatusBar status item with cloud icon.
  10-second daemon health polling. Config file reader. Folder list display.
  Actions for refresh, open config, diagnostics, quit.

- **Setup wizard** (Python): 5-step interactive setup — prerequisite checks,
  iCloud container discovery with friendly name mapping, folder selection,
  CIDR network configuration, LaunchAgent installation. Non-interactive mode
  for scripting.

- **Distribution**: Makefile with build/test/lint/install/uninstall targets.
  Universal binary build script (arm64 + x86_64). Homebrew formula with brew
  services support. GitHub Actions release workflow (tag-triggered, tarball +
  SHA256).

- **CI**: GitHub Actions on macOS 15, Swift + Rust + Python test suites
  (62 tests total).

[0.1.0]: https://github.com/wizz-cmd/icloud-nfs-exporter/releases/tag/v0.1.0
