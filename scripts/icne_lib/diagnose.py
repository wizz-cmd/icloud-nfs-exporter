"""Diagnostic checks for icloud-nfs-exporter."""

import shutil
import subprocess
from pathlib import Path

from . import config as cfg
from . import ipc, nfs


class Check:
    """Result of a single diagnostic check."""

    def __init__(self, name: str, ok: bool, detail: str = ""):
        self.name = name
        self.ok = ok
        self.detail = detail

    def __str__(self):
        status = "OK" if self.ok else "FAIL"
        line = f"  [{status:>4}]  {self.name}"
        if self.detail:
            line += f" — {self.detail}"
        return line


def check_icloud_drive() -> Check:
    """Check if iCloud Drive exists at the expected path."""
    path = cfg.ICLOUD_DRIVE
    if path.is_dir():
        count = sum(1 for _ in path.iterdir())
        return Check("iCloud Drive", True, f"{path} ({count} items)")
    return Check("iCloud Drive", False, f"not found at {path}")


def check_config() -> Check:
    """Check if the config file exists and is valid."""
    if not cfg.CONFIG_FILE.exists():
        return Check("Config file", False, f"not found — run 'icne setup'")
    try:
        c = cfg.load_config()
        n = len(c.get("folders", []))
        return Check("Config file", True, f"{cfg.CONFIG_FILE} ({n} folders)")
    except Exception as e:
        return Check("Config file", False, str(e))


def check_daemon() -> Check:
    """Check if the hydration daemon is running."""
    c = cfg.load_config()
    socket_path = c.get("general", {}).get("socket_path", cfg.DEFAULT_SOCKET)
    client = ipc.IpcClient(socket_path, timeout=3.0)
    if client.is_available():
        return Check("Hydration daemon", True, f"responding at {socket_path}")
    if Path(socket_path).exists():
        return Check("Hydration daemon", False, f"socket exists but not responding")
    return Check("Hydration daemon", False, f"not running (no socket at {socket_path})")


def check_macfuse() -> Check:
    """Check if macFUSE is installed."""
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


def check_nfsd() -> Check:
    """Check if nfsd is running."""
    if nfs.nfsd_is_running():
        exports = nfs.show_exports()
        n = max(0, len(exports) - 1)  # first line is header
        return Check("NFS server (nfsd)", True, f"{n} export(s)")
    return Check("NFS server (nfsd)", False, "not running")


def check_mount_base() -> Check:
    """Check if the mount base directory exists."""
    c = cfg.load_config()
    base = Path(c.get("general", {}).get("mount_base", cfg.DEFAULT_MOUNT_BASE))
    if base.is_dir():
        mounts = [d.name for d in base.iterdir() if d.is_dir()]
        return Check("Mount base", True, f"{base} ({len(mounts)} mounts)")
    return Check("Mount base", False, f"{base} does not exist")


def check_rust_toolchain() -> Check:
    """Check if Rust/Cargo is available."""
    if shutil.which("cargo"):
        r = subprocess.run(["cargo", "--version"], capture_output=True, text=True)
        return Check("Rust toolchain", True, r.stdout.strip())
    return Check("Rust toolchain", False, "cargo not found")


def run_all() -> list[Check]:
    """Run all diagnostic checks."""
    return [
        check_icloud_drive(),
        check_config(),
        check_daemon(),
        check_macfuse(),
        check_nfsd(),
        check_mount_base(),
        check_rust_toolchain(),
    ]
