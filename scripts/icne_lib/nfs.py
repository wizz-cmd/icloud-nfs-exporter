"""NFS export management for macOS nfsd."""

import ipaddress
import subprocess
from pathlib import Path

EXPORTS_FILE = Path("/etc/exports")
MARKER_BEGIN = "# BEGIN icloud-nfs-exporter"
MARKER_END = "# END icloud-nfs-exporter"


def cidr_to_network_mask(cidr: str) -> tuple[str, str]:
    """Convert CIDR notation to network + mask for /etc/exports.

    '192.168.0.0/24' → ('192.168.0.0', '255.255.255.0')
    """
    net = ipaddress.ip_network(cidr, strict=False)
    return str(net.network_address), str(net.netmask)


def generate_exports_entry(export_path: str, cidr: str) -> str:
    """Generate an /etc/exports line for a directory.

    Returns e.g.: /tmp/icne-mnt/CloudDocs -network 192.168.0.0 -mask 255.255.255.0
    """
    network, mask = cidr_to_network_mask(cidr)
    return f"{export_path} -network {network} -mask {mask}"


def read_exports() -> str:
    """Read the current /etc/exports content (may be empty)."""
    if not EXPORTS_FILE.exists():
        return ""
    return EXPORTS_FILE.read_text()


def build_managed_block(entries: list[str]) -> str:
    """Build the managed block for /etc/exports."""
    lines = [MARKER_BEGIN]
    lines.extend(entries)
    lines.append(MARKER_END)
    return "\n".join(lines) + "\n"


def update_exports(entries: list[str]) -> str:
    """Return updated /etc/exports content with our managed block.

    Preserves any user-written lines outside our markers.
    Does NOT write the file — the caller must write it (requires root).
    """
    current = read_exports()

    # Remove existing managed block
    cleaned_lines = []
    in_block = False
    for line in current.splitlines():
        if line.strip() == MARKER_BEGIN:
            in_block = True
            continue
        if line.strip() == MARKER_END:
            in_block = False
            continue
        if not in_block:
            cleaned_lines.append(line)

    # Remove trailing blank lines
    while cleaned_lines and not cleaned_lines[-1].strip():
        cleaned_lines.pop()

    # Append managed block
    result = "\n".join(cleaned_lines)
    if result and not result.endswith("\n"):
        result += "\n"
    if entries:
        result += "\n" + build_managed_block(entries)

    return result


def apply_exports(content: str) -> None:
    """Write /etc/exports and reload nfsd.  Requires root."""
    EXPORTS_FILE.write_text(content)
    restart_nfsd()


def nfsd_is_running() -> bool:
    """Check if nfsd is currently running."""
    r = subprocess.run(
        ["nfsd", "status"],
        capture_output=True, text=True,
    )
    return r.returncode == 0


def restart_nfsd() -> None:
    """Restart nfsd to pick up /etc/exports changes."""
    subprocess.run(["nfsd", "update"], check=True)


def show_exports() -> list[str]:
    """Return current NFS exports as shown by showmount."""
    r = subprocess.run(
        ["showmount", "-e", "localhost"],
        capture_output=True, text=True,
    )
    if r.returncode != 0:
        return []
    return r.stdout.strip().splitlines()
