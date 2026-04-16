"""NFS export management for macOS nfsd."""

import ipaddress
import subprocess
from pathlib import Path

EXPORTS_FILE = Path("/etc/exports")
MARKER_BEGIN = "# BEGIN icloud-nfs-exporter"
MARKER_END = "# END icloud-nfs-exporter"


def cidr_to_network_mask(cidr: str) -> tuple[str, str]:
    """Convert CIDR notation to a (network, netmask) tuple.

    Produce the network address and dotted-decimal subnet mask
    required by ``/etc/exports`` on macOS.

    Args:
        cidr: A CIDR string such as ``"192.168.0.0/24"``.

    Returns:
        A ``(network_address, netmask)`` tuple of strings, e.g.
        ``("192.168.0.0", "255.255.255.0")``.
    """
    net = ipaddress.ip_network(cidr, strict=False)
    return str(net.network_address), str(net.netmask)


def generate_exports_entry(export_path: str, cidr: str) -> str:
    """Generate an ``/etc/exports`` line for a single directory.

    Args:
        export_path: Absolute path to the directory being exported.
        cidr: Allowed network in CIDR notation (e.g. ``"192.168.0.0/24"``).

    Returns:
        A formatted exports line, e.g.
        ``"/tmp/icne-mnt/CloudDocs -network 192.168.0.0 -mask 255.255.255.0"``.
    """
    network, mask = cidr_to_network_mask(cidr)
    return f"{export_path} -network {network} -mask {mask}"


def read_exports() -> str:
    """Read the current ``/etc/exports`` content.

    Returns:
        The full text of ``/etc/exports``, or an empty string if the
        file does not exist.
    """
    if not EXPORTS_FILE.exists():
        return ""
    return EXPORTS_FILE.read_text()


def build_managed_block(entries: list[str]) -> str:
    """Build the managed block for ``/etc/exports``.

    Wrap *entries* between ``MARKER_BEGIN`` and ``MARKER_END`` sentinel
    comments so the block can be located and replaced on subsequent runs.

    Args:
        entries: One export line per element (without trailing newlines).

    Returns:
        The complete managed block as a single string ending with a
        newline.
    """
    lines = [MARKER_BEGIN]
    lines.extend(entries)
    lines.append(MARKER_END)
    return "\n".join(lines) + "\n"


def update_exports(entries: list[str]) -> str:
    """Return updated ``/etc/exports`` content with the managed block.

    Read the current ``/etc/exports``, strip any existing managed block,
    and append a new one built from *entries*.  Lines outside the managed
    markers are preserved verbatim.

    This function does **not** write the file -- the caller must write it
    (which typically requires root).

    Args:
        entries: Export lines to include in the managed block.

    Returns:
        The full updated ``/etc/exports`` content as a string.
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
    """Write ``/etc/exports`` and reload nfsd.

    This function requires root privileges.

    Args:
        content: The complete new ``/etc/exports`` text to write.

    Raises:
        PermissionError: If the process lacks root privileges.
        subprocess.CalledProcessError: If ``nfsd update`` fails.
    """
    EXPORTS_FILE.write_text(content)
    restart_nfsd()


def nfsd_is_running() -> bool:
    """Check whether nfsd is currently running.

    Returns:
        ``True`` if ``nfsd status`` exits with code 0, ``False``
        otherwise.
    """
    r = subprocess.run(
        ["nfsd", "status"],
        capture_output=True, text=True,
    )
    return r.returncode == 0


def restart_nfsd() -> None:
    """Restart nfsd to pick up ``/etc/exports`` changes.

    Raises:
        subprocess.CalledProcessError: If ``nfsd update`` fails.
    """
    subprocess.run(["nfsd", "update"], check=True)


def show_exports() -> list[str]:
    """Return current NFS exports as reported by ``showmount``.

    Returns:
        Lines of output from ``showmount -e localhost``.  The first
        line is typically a header.  Returns an empty list if the
        command fails.
    """
    r = subprocess.run(
        ["showmount", "-e", "localhost"],
        capture_output=True, text=True,
    )
    if r.returncode != 0:
        return []
    return r.stdout.strip().splitlines()
