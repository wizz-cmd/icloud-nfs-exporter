"""iCloud Drive container discovery."""

from pathlib import Path

from . import config as cfg

# Well-known Apple container IDs → friendly names
_APPLE_LABELS = {
    "com~apple~CloudDocs": "iCloud Drive",
    "com~apple~Numbers": "Numbers",
    "com~apple~Pages": "Pages",
    "com~apple~Keynote": "Keynote",
    "com~apple~Preview": "Preview",
    "com~apple~TextEdit": "TextEdit",
    "com~apple~QuickTimePlayerX": "QuickTime Player",
    "com~apple~Automator": "Automator",
    "com~apple~ScriptEditor2": "Script Editor",
    "com~apple~mail": "Mail",
}


def label_for_container(name: str) -> str:
    """Derive a human-readable label from a container directory name.

    Examples:
        com~apple~CloudDocs        → "iCloud Drive"
        iCloud~com~example~MyApp   → "MyApp"
        com~apple~Numbers          → "Numbers"
        SomeOtherDir               → "SomeOtherDir"
    """
    if name in _APPLE_LABELS:
        return _APPLE_LABELS[name]

    # iCloud~com~vendor~AppName → "AppName"
    if name.startswith("iCloud~"):
        parts = name.split("~")
        if len(parts) >= 2:
            return parts[-1]

    return name


def discover_containers(
    base: Path | None = None,
) -> list[dict]:
    """List available iCloud containers.

    Returns a list of dicts with keys: path, name, label.
    Sorted with iCloud Drive first, then alphabetically by label.
    """
    if base is None:
        base = cfg.ICLOUD_DRIVE
    if not base.is_dir():
        return []

    containers = []
    for child in sorted(base.iterdir()):
        if not child.is_dir():
            continue
        name = child.name
        # Skip hidden directories
        if name.startswith("."):
            continue
        containers.append({
            "path": str(child),
            "name": name,
            "label": label_for_container(name),
        })

    # Sort: iCloud Drive first, then alphabetical
    def sort_key(c):
        if c["name"] == "com~apple~CloudDocs":
            return (0, "")
        return (1, c["label"].lower())

    containers.sort(key=sort_key)
    return containers
