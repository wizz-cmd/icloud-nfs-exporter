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

    Look up *name* in the well-known Apple container table first.
    For third-party ``iCloud~com~...`` names, extract the last
    tilde-separated component.  Fall back to *name* unchanged.

    Args:
        name: The raw directory name inside ``~/Library/Mobile Documents``
            (e.g. ``"com~apple~CloudDocs"``, ``"iCloud~com~example~MyApp"``).

    Returns:
        A human-friendly label such as ``"iCloud Drive"`` or ``"MyApp"``.
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
) -> list[dict[str, str]]:
    """List available iCloud containers under *base*.

    Scan the iCloud ``Mobile Documents`` directory for
    sub-directories, derive labels with ``label_for_container``,
    and return them sorted with iCloud Drive first, then
    alphabetically by label.

    Args:
        base: Root directory to scan.  Defaults to
            ``~/Library/Mobile Documents`` when ``None``.

    Returns:
        A list of dicts, each with keys ``"path"`` (absolute path
        string), ``"name"`` (directory basename), and ``"label"``
        (human-readable label).  Returns an empty list if *base*
        does not exist.
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
