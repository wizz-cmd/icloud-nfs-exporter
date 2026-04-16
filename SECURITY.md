# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability, please report it responsibly:

1. **Do not open a public issue.**
2. Use [GitHub's private security advisory](https://github.com/wizz-cmd/icloud-nfs-exporter/security/advisories/new) to report the issue.
3. Include: affected version, steps to reproduce, potential impact.

You will receive a response within 72 hours. Fixes will be released as patch versions.

## Scope

This project runs as a local macOS service with access to:
- iCloud Drive files (via FileProvider API)
- NFS server configuration (`/etc/exports`)
- Unix domain sockets (IPC)

Security-relevant areas include the IPC protocol, NFS export permissions, and file access controls.
