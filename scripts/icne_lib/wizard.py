"""Interactive setup wizard for icloud-nfs-exporter."""

import ipaddress
import os
from pathlib import Path

from . import config as cfg
from . import diagnose, icloud, nfs


def _ask(prompt: str, default: str = "") -> str:
    """Prompt the user, showing a default in brackets."""
    if default:
        raw = input(f"  {prompt} [{default}]: ").strip()
        return raw if raw else default
    return input(f"  {prompt}: ").strip()


def _ask_yn(prompt: str, default: bool = True) -> bool:
    """Yes/no prompt."""
    hint = "Y/n" if default else "y/N"
    raw = input(f"  {prompt} [{hint}] ").strip().lower()
    if not raw:
        return default
    return raw.startswith("y")


def _header(text: str) -> None:
    width = max(len(text) + 4, 44)
    print()
    print(f"  {'=' * width}")
    print(f"  | {text:^{width - 4}} |")
    print(f"  {'=' * width}")
    print()


def _step(n: int, title: str) -> None:
    print(f"\n  Step {n}: {title}\n")


def run(*, force: bool = False, non_interactive: bool = False) -> None:
    """Run the interactive setup wizard."""

    _header("icloud-nfs-exporter  —  Setup Wizard")

    # ── Step 1: Prerequisites ──
    _step(1, "Checking prerequisites")

    checks = [
        diagnose.check_icloud_drive(),
        diagnose.check_macfuse(),
        diagnose.check_nfsd(),
        diagnose.check_rust_toolchain(),
    ]
    for c in checks:
        print(f"  {c}")

    missing = [c for c in checks if not c.ok]
    if missing:
        print()
        print("  Some prerequisites are missing. The service will have")
        print("  limited functionality until they are installed.")
        if not non_interactive:
            if not _ask_yn("Continue anyway?"):
                print("\n  Setup cancelled.")
                return
    else:
        print("\n  All prerequisites met.")

    # ── Step 2: Folder selection ──
    _step(2, "Select iCloud folders to export")

    existing_config = cfg.load_config() if cfg.CONFIG_FILE.exists() else None
    already = set()
    if existing_config and not force:
        already = {f["source"] for f in existing_config.get("folders", [])}

    containers = icloud.discover_containers()
    if not containers:
        print("  No iCloud containers found.")
        print("  Make sure iCloud Drive is enabled in System Settings.")
        selected_folders = []
    elif non_interactive:
        # Auto-select iCloud Drive
        selected_folders = [
            c for c in containers
            if c["name"] == "com~apple~CloudDocs"
        ]
        if not selected_folders:
            selected_folders = containers[:1]
        for c in selected_folders:
            print(f"  Auto-selected: {c['label']} ({c['name']})")
    else:
        print("  Available iCloud containers:\n")
        for i, c in enumerate(containers, 1):
            marker = " *" if c["path"] in already else ""
            print(f"    {i:2}. {c['label']:30s} ({c['name']}){marker}")
        if already:
            print("\n  (* = already configured)")

        print()
        default_sel = "1"
        raw = _ask(
            "Enter numbers to export (comma-separated)", default_sel
        )
        try:
            indices = [int(x.strip()) for x in raw.split(",") if x.strip()]
        except ValueError:
            indices = [1]

        selected_folders = []
        for idx in indices:
            if 1 <= idx <= len(containers):
                selected_folders.append(containers[idx - 1])

    # ── Step 3: Network configuration ──
    _step(3, "Network configuration")

    default_network = "192.168.0.0/24"
    if existing_config and not force:
        default_network = existing_config.get("nfs", {}).get(
            "allowed_network", default_network
        )

    if non_interactive:
        network = default_network
        print(f"  NFS allowed network: {network}")
    else:
        print("  Which network should be allowed to access NFS exports?")
        print("  Use CIDR notation (e.g. 192.168.1.0/24, 10.0.0.0/8).\n")
        while True:
            network = _ask("Allowed network", default_network)
            try:
                ipaddress.ip_network(network, strict=False)
                break
            except ValueError:
                print(f"  Invalid network: {network}")

    # ── Step 4: Write configuration ──
    _step(4, "Writing configuration")

    config = cfg.default_config()
    config["nfs"]["allowed_network"] = network

    # Merge existing folders if not forcing
    if existing_config and not force:
        config["folders"] = list(existing_config.get("folders", []))

    # Add newly selected folders (skip duplicates)
    existing_sources = {f["source"] for f in config.get("folders", [])}
    for c in selected_folders:
        if c["path"] not in existing_sources:
            config["folders"].append({
                "source": c["path"],
                "label": c["label"],
            })

    cfg.save_config(config)
    print(f"  Config: {cfg.CONFIG_FILE}")

    # Create mount points
    mount_base = config["general"]["mount_base"]
    os.makedirs(mount_base, exist_ok=True)
    for folder in config["folders"]:
        mp = cfg.mount_point_for(folder["source"], config)
        os.makedirs(mp, exist_ok=True)
    print(f"  Mount base: {mount_base}")
    print(f"  Folders: {len(config['folders'])} configured")

    # ── Step 5: LaunchAgent ──
    _step(5, "LaunchAgent installation")

    plist_src = (
        Path(__file__).resolve().parent.parent.parent
        / "launchd"
        / "com.wizz-cmd.icloud-nfs-exporter.plist.template"
    )
    plist_dst = Path.home() / "Library" / "LaunchAgents" / \
        "com.wizz-cmd.icloud-nfs-exporter.plist"

    if plist_dst.exists() and not force:
        print(f"  LaunchAgent already installed: {plist_dst}")
    elif plist_src.exists():
        install_agent = non_interactive or _ask_yn(
            "Install LaunchAgent (auto-start daemon at login)?"
        )
        if install_agent:
            content = plist_src.read_text()
            content = content.replace("__USERNAME__", os.environ.get("USER", ""))
            plist_dst.parent.mkdir(parents=True, exist_ok=True)
            plist_dst.write_text(content)
            print(f"  Installed: {plist_dst}")
        else:
            print("  Skipped.")
    else:
        print(f"  Template not found: {plist_src}")

    # ── Summary ──
    _header("Setup complete!")

    print("  Configured folders:")
    for f in config["folders"]:
        mp = cfg.mount_point_for(f["source"], config)
        print(f"    {f.get('label', '?'):24s} -> {mp}")

    print(f"\n  NFS network: {network}")

    if missing:
        print("\n  Recommended next steps:")
        for c in missing:
            if "macFUSE" in c.name:
                print("    - Install macFUSE: https://osxfuse.github.io/")
            elif "Rust" in c.name:
                print("    - Install Rust:    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh")
        print("    - Run diagnostics: icne diagnose")
    else:
        print("\n  All systems go. Run 'icne diagnose' to verify.")
    print()
