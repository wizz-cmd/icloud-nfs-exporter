"""NFS export management — direct NFS server and legacy macOS nfsd."""

import ipaddress
import os
import shutil
import signal
import subprocess
from pathlib import Path

EXPORTS_FILE = Path("/etc/exports")
MARKER_BEGIN = "# BEGIN icloud-nfs-exporter"
MARKER_END = "# END icloud-nfs-exporter"
DEFAULT_PORT = 11111


# ── Direct NFS server (nfs-server binary) ──


def nfs_server_binary() -> str | None:
    """Find the nfs-server binary.

    Search in order: $PATH, then the project build directories
    (release, then debug).

    Returns:
        Absolute path to the binary, or ``None`` if not found.
    """
    found = shutil.which("icloud-nfs-server") or shutil.which("nfs-server")
    if found:
        return found
    # Check project build dirs relative to this file
    project = Path(__file__).resolve().parent.parent.parent
    for profile in ("release", "debug"):
        p = project / "src" / "nfs" / "target" / profile / "nfs-server"
        if p.is_file() and os.access(p, os.X_OK):
            return str(p)
    return None


def nfs_server_pid() -> int | None:
    """Return the PID of a running nfs-server process, or ``None``."""
    r = subprocess.run(
        ["pgrep", "-f", "nfs-server serve"],
        capture_output=True, text=True,
    )
    if r.returncode == 0 and r.stdout.strip():
        return int(r.stdout.strip().splitlines()[0])
    return None


def nfs_server_is_running() -> bool:
    """Check whether the direct NFS server is running."""
    return nfs_server_pid() is not None


def start_nfs_server(
    source: str,
    port: int = DEFAULT_PORT,
    socket_path: str = "/tmp/icloud-nfs-exporter.sock",
) -> subprocess.Popen | None:
    """Start the direct NFS server in the background.

    Args:
        source: Absolute path to the iCloud source directory.
        port: TCP port to listen on.
        socket_path: Hydration daemon IPC socket path.

    Returns:
        The ``Popen`` object, or ``None`` if the binary was not found.
    """
    binary = nfs_server_binary()
    if binary is None:
        return None
    return subprocess.Popen(
        [binary, "serve", source, "--port", str(port), "--socket", socket_path],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


def stop_nfs_server() -> bool:
    """Stop the running NFS server.

    Returns:
        ``True`` if a process was found and signalled, ``False`` otherwise.
    """
    pid = nfs_server_pid()
    if pid is None:
        return False
    os.kill(pid, signal.SIGTERM)
    return True


def mount_command(host: str = "HOST", port: int = DEFAULT_PORT) -> dict[str, str]:
    """Return NFS mount commands for Linux and macOS clients.

    Args:
        host: Hostname or IP address of the NFS server.
        port: TCP port the server listens on.

    Returns:
        A dict with ``linux`` and ``macos`` keys containing the mount
        command strings.
    """
    return {
        "linux": (
            f"sudo mount.nfs -o vers=3,tcp,port={port},"
            f"mountport={port},nolock {host}:/ /mnt"
        ),
        "macos": (
            f"mount_nfs -o vers=3,tcp,port={port},"
            f"mountport={port},nolocks {host}:/ /mnt"
        ),
    }


# ── Legacy: macOS nfsd + /etc/exports ──


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
