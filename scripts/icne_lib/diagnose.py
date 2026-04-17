"""Diagnostic checks for icloud-nfs-exporter."""

import shutil
import subprocess
from pathlib import Path

from . import config as cfg
from . import ipc, nfs


class Check:
    """Represent the result of a single diagnostic check.

    Attributes:
        name: Short human-readable label for the check.
        ok: ``True`` if the check passed.
        detail: Optional detail message shown after the status.
    """

    def __init__(self, name: str, ok: bool, detail: str = "") -> None:
        self.name = name
        self.ok = ok
        self.detail = detail

    def __str__(self) -> str:
        status = "OK" if self.ok else "FAIL"
        line = f"  [{status:>4}]  {self.name}"
        if self.detail:
            line += f" — {self.detail}"
        return line


def check_icloud_drive() -> Check:
    """Check whether iCloud Drive exists at the expected path.

    Returns:
        A ``Check`` that passes when ``~/Library/Mobile Documents``
        is a directory, including the item count in the detail.
    """
    path = cfg.ICLOUD_DRIVE
    if path.is_dir():
        count = sum(1 for _ in path.iterdir())
        return Check("iCloud Drive", True, f"{path} ({count} items)")
    return Check("iCloud Drive", False, f"not found at {path}")


def check_config() -> Check:
    """Check whether the config file exists and is valid TOML.

    Returns:
        A ``Check`` that passes when the config file can be loaded
        without errors.
    """
    if not cfg.CONFIG_FILE.exists():
        return Check("Config file", False, f"not found — run 'icne setup'")
    try:
        c = cfg.load_config()
        n = len(c.get("folders", []))
        return Check("Config file", True, f"{cfg.CONFIG_FILE} ({n} folders)")
    except Exception as e:
        return Check("Config file", False, str(e))


def check_daemon() -> Check:
    """Check whether the hydration daemon is running and responding.

    Returns:
        A ``Check`` that passes when the daemon answers a ping via
        its Unix socket.
    """
    c = cfg.load_config()
    socket_path = c.get("general", {}).get("socket_path", cfg.DEFAULT_SOCKET)
    client = ipc.IpcClient(socket_path, timeout=3.0)
    if client.is_available():
        return Check("Hydration daemon", True, f"responding at {socket_path}")
    if Path(socket_path).exists():
        return Check("Hydration daemon", False, f"socket exists but not responding")
    return Check("Hydration daemon", False, f"not running (no socket at {socket_path})")


def check_macfuse() -> Check:
    """Check whether macFUSE is installed.

    Look for ``/Library/Filesystems/macfuse.fs`` and, if found,
    attempt to read its ``CFBundleVersion``.

    Returns:
        A ``Check`` that passes when the macFUSE filesystem bundle
        exists on disk.
    """
    macfuse_fs = Path("/Library/Filesystems/macfuse.fs")
    if macfuse_fs.exists():
        # Try to get version
        version_plist = macfuse_fs / "Contents" / "Info.plist"
        detail = "installed"
        if version_plist.exists():
            try:
                r = subprocess.run(
                    ["defaults", "read", str(version_plist), "CFBundleVersion"],
                    capture_output=True, text=True,
                )
                if r.returncode == 0:
                    detail = f"v{r.stdout.strip()}"
            except Exception:
                pass
        return Check("macFUSE", True, detail)
    return Check("macFUSE", False, "not installed — https://osxfuse.github.io/")


def check_nfs_server() -> Check:
    """Check whether the direct NFS server is running.

    Returns:
        A ``Check`` that passes when the ``nfs-server`` process is
        found, including the PID in the detail.
    """
    pid = nfs.nfs_server_pid()
    if pid is not None:
        return Check("NFS server", True, f"running (PID {pid})")
    binary = nfs.nfs_server_binary()
    if binary is None:
        return Check("NFS server", False, "binary not found — run 'cd src/nfs && cargo build'")
    return Check("NFS server", False, f"not running (binary at {binary})")


def check_mount_base() -> Check:
    """Check whether the mount-base directory exists.

    Returns:
        A ``Check`` that passes when the configured ``mount_base``
        directory is present and lists how many sub-mount directories
        it contains.
    """
    c = cfg.load_config()
    base = Path(c.get("general", {}).get("mount_base", cfg.DEFAULT_MOUNT_BASE))
    if base.is_dir():
        mounts = [d.name for d in base.iterdir() if d.is_dir()]
        return Check("Mount base", True, f"{base} ({len(mounts)} mounts)")
    return Check("Mount base", False, f"{base} does not exist")


def check_rust_toolchain() -> Check:
    """Check whether the Rust toolchain (cargo) is available on ``$PATH``.

    Returns:
        A ``Check`` that passes when ``cargo --version`` succeeds,
        including the version string in the detail.
    """
    if shutil.which("cargo"):
        r = subprocess.run(["cargo", "--version"], capture_output=True, text=True)
        return Check("Rust toolchain", True, r.stdout.strip())
    return Check("Rust toolchain", False, "cargo not found")


def run_all() -> list[Check]:
    """Run all diagnostic checks and return the results.

    Returns:
        A list of ``Check`` objects, one per diagnostic, in a fixed
        order: iCloud Drive, config, daemon, macFUSE, nfsd,
        mount base, Rust toolchain.
    """
    return [
        check_icloud_drive(),
        check_config(),
        check_daemon(),
        check_nfs_server(),
        check_mount_base(),
        check_rust_toolchain(),
    ]
