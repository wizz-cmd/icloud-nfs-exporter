"""Configuration file management for icloud-nfs-exporter."""

import os
import tomllib
from pathlib import Path

CONFIG_DIR = Path.home() / ".config" / "icloud-nfs-exporter"
CONFIG_FILE = CONFIG_DIR / "config.toml"
DEFAULT_SOCKET = "/tmp/icloud-nfs-exporter.sock"
DEFAULT_MOUNT_BASE = "/tmp/icne-mnt"
ICLOUD_DRIVE = Path.home() / "Library" / "Mobile Documents"


def default_config() -> dict[str, object]:
    """Return the default configuration dictionary.

    Returns:
        A dict with ``general``, ``nfs``, and ``folders`` keys populated
        with sane defaults.
    """
    return {
        "general": {
            "socket_path": DEFAULT_SOCKET,
            "mount_base": DEFAULT_MOUNT_BASE,
        },
        "nfs": {
            "server": "direct",
            "port": 11111,
            "allowed_network": "192.168.0.0/24",
        },
        "folders": [],
    }


def load_config() -> dict[str, object]:
    """Load configuration from disk, or return defaults if missing.

    Returns:
        The parsed TOML configuration as a dict, or the default
        configuration if the config file does not exist.
    """
    if not CONFIG_FILE.exists():
        return default_config()
    with open(CONFIG_FILE, "rb") as f:
        return tomllib.load(f)


def save_config(config: dict[str, object]) -> None:
    """Write configuration to disk as TOML.

    Create the config directory if it does not already exist, then
    serialise *config* into ``~/.config/icloud-nfs-exporter/config.toml``.

    Args:
        config: The full configuration dictionary (must contain
            ``general``, ``nfs``, and ``folders`` keys).
    """
    CONFIG_DIR.mkdir(parents=True, exist_ok=True)
    with open(CONFIG_FILE, "w") as f:
        # [general]
        f.write("[general]\n")
        gen = config.get("general", {})
        f.write(f'socket_path = "{gen.get("socket_path", DEFAULT_SOCKET)}"\n')
        f.write(f'mount_base = "{gen.get("mount_base", DEFAULT_MOUNT_BASE)}"\n')
        f.write("\n")

        # [nfs]
        f.write("[nfs]\n")
        nfs = config.get("nfs", {})
        f.write(f'server = "{nfs.get("server", "direct")}"\n')
        f.write(f'port = {nfs.get("port", 11111)}\n')
        f.write(f'allowed_network = "{nfs.get("allowed_network", "192.168.0.0/24")}"\n')
        f.write("\n")

        # [[folders]]
        for folder in config.get("folders", []):
            f.write("[[folders]]\n")
            f.write(f'source = "{folder["source"]}"\n')
            f.write(f'label = "{folder.get("label", "")}"\n')
            f.write("\n")


def add_folder(source: str, label: str | None = None) -> dict[str, object]:
    """Add a folder to the export list and return the updated config.

    Resolve *source* to an absolute path, verify the directory exists,
    append it to the ``folders`` list, and persist the result to disk.

    Args:
        source: Path to the directory to export (may use ``~``).
        label: Optional human-readable label.  Defaults to the directory
            basename if not provided.

    Returns:
        The updated configuration dictionary.

    Raises:
        FileNotFoundError: If *source* does not point to an existing
            directory.
        ValueError: If *source* is already in the configuration.
    """
    source = str(Path(source).expanduser().resolve())
    if not Path(source).is_dir():
        raise FileNotFoundError(f"Directory not found: {source}")

    config = load_config()
    existing = [f["source"] for f in config.get("folders", [])]
    if source in existing:
        raise ValueError(f"Already configured: {source}")

    if label is None:
        label = Path(source).name

    config.setdefault("folders", []).append({
        "source": source,
        "label": label,
    })
    save_config(config)
    return config


def remove_folder(source: str) -> dict[str, object]:
    """Remove a folder from the export list.

    Resolve *source* to an absolute path, drop the matching entry from
    ``folders``, and persist the result to disk.

    Args:
        source: Path to the directory to remove (may use ``~``).

    Returns:
        The updated configuration dictionary.
    """
    source = str(Path(source).expanduser().resolve())
    config = load_config()
    folders = config.get("folders", [])
    config["folders"] = [f for f in folders if f["source"] != source]
    save_config(config)
    return config


def mount_point_for(source: str, config: dict[str, object] | None = None) -> Path:
    """Derive the FUSE mount point for a source directory.

    Combine the configured ``mount_base`` with the basename of *source*
    to produce the path where the FUSE filesystem will be mounted.

    Args:
        source: Absolute path to the iCloud source directory.
        config: Optional pre-loaded configuration dict.  When ``None``,
            the configuration is loaded from disk.

    Returns:
        The ``Path`` to the mount point directory.
    """
    if config is None:
        config = load_config()
    base = config.get("general", {}).get("mount_base", DEFAULT_MOUNT_BASE)
    return Path(base) / Path(source).name
