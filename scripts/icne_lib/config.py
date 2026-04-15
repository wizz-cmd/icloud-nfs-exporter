"""Configuration file management for icloud-nfs-exporter."""

import os
import tomllib
from pathlib import Path

CONFIG_DIR = Path.home() / ".config" / "icloud-nfs-exporter"
CONFIG_FILE = CONFIG_DIR / "config.toml"
DEFAULT_SOCKET = "/tmp/icloud-nfs-exporter.sock"
DEFAULT_MOUNT_BASE = "/tmp/icne-mnt"
ICLOUD_DRIVE = Path.home() / "Library" / "Mobile Documents"


def default_config() -> dict:
    """Return the default configuration."""
    return {
        "general": {
            "socket_path": DEFAULT_SOCKET,
            "mount_base": DEFAULT_MOUNT_BASE,
        },
        "nfs": {
            "server": "nfsd",
            "allowed_network": "192.168.0.0/24",
        },
        "folders": [],
    }


def load_config() -> dict:
    """Load configuration from disk, or return defaults if missing."""
    if not CONFIG_FILE.exists():
        return default_config()
    with open(CONFIG_FILE, "rb") as f:
        return tomllib.load(f)


def save_config(config: dict) -> None:
    """Write configuration to disk as TOML."""
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
        f.write(f'server = "{nfs.get("server", "nfsd")}"\n')
        f.write(f'allowed_network = "{nfs.get("allowed_network", "192.168.0.0/24")}"\n')
        f.write("\n")

        # [[folders]]
        for folder in config.get("folders", []):
            f.write("[[folders]]\n")
            f.write(f'source = "{folder["source"]}"\n')
            f.write(f'label = "{folder.get("label", "")}"\n')
            f.write("\n")


def add_folder(source: str, label: str | None = None) -> dict:
    """Add a folder to the export list and return the updated config."""
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


def remove_folder(source: str) -> dict:
    """Remove a folder from the export list."""
    source = str(Path(source).expanduser().resolve())
    config = load_config()
    folders = config.get("folders", [])
    config["folders"] = [f for f in folders if f["source"] != source]
    save_config(config)
    return config


def mount_point_for(source: str, config: dict | None = None) -> Path:
    """Derive the FUSE mount point for a source directory."""
    if config is None:
        config = load_config()
    base = config.get("general", {}).get("mount_base", DEFAULT_MOUNT_BASE)
    return Path(base) / Path(source).name
