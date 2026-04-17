"""Interactive setup wizard for icloud-nfs-exporter."""

import ipaddress
import os
from pathlib import Path

from . import config as cfg
from . import diagnose, icloud, nfs


def _ask(prompt: str, default: str = "") -> str:
    """Prompt the user for text input, showing a default in brackets.

    Args:
        prompt: The question to display.
        default: Value returned when the user presses Enter without
            typing anything.

    Returns:
        The user's input, or *default* if the input was empty.
    """
    if default:
        raw = input(f"  {prompt} [{default}]: ").strip()
        return raw if raw else default
    return input(f"  {prompt}: ").strip()


def _ask_yn(prompt: str, default: bool = True) -> bool:
    """Prompt the user with a yes/no question.

    Args:
        prompt: The question to display.
        default: The answer used when the user presses Enter without
            typing anything.

    Returns:
        ``True`` for yes, ``False`` for no.
    """
    hint = "Y/n" if default else "y/N"
    raw = input(f"  {prompt} [{hint}] ").strip().lower()
    if not raw:
        return default
    return raw.startswith("y")


def _header(text: str) -> None:
    """Print a centred, box-framed header line to stdout.

    Args:
        text: The title text to display inside the box.
    """
    width = max(len(text) + 4, 44)
    print()
    print(f"  {'=' * width}")
    print(f"  | {text:^{width - 4}} |")
    print(f"  {'=' * width}")
    print()


def _step(n: int, title: str) -> None:
    """Print a numbered step heading to stdout.

    Args:
        n: Step number.
        title: Short description of the step.
    """
    print(f"\n  Step {n}: {title}\n")


def run(*, force: bool = False, non_interactive: bool = False) -> None:
    """Run the interactive setup wizard.

    Walk the user through prerequisite checks, folder selection,
    network configuration, config-file generation, and LaunchAgent
    installation.

    Args:
        force: When ``True``, overwrite existing configuration and
            re-install the LaunchAgent instead of merging.
        non_interactive: When ``True``, accept all defaults without
            prompting -- suitable for scripted/CI usage.
    """

    _header("icloud-nfs-exporter  —  Setup Wizard")

    # ── Step 1: Prerequisites ──
    _step(1, "Checking prerequisites")

    checks = [
        diagnose.check_icloud_drive(),
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

    # ── Step 5: LaunchAgents ──
    _step(5, "LaunchAgent installation")

    launchd_dir = Path(__file__).resolve().parent.parent.parent / "launchd"
    agents_dir = Path.home() / "Library" / "LaunchAgents"
    username = os.environ.get("USER", "")

    # Determine source and port from config for the NFS server plist
    source_dir = ""
    if config.get("folders"):
        source_dir = config["folders"][0]["source"]
    port = str(config.get("nfs", {}).get("port", 11111))
    socket_path = config.get("general", {}).get(
        "socket_path", cfg.DEFAULT_SOCKET
    )

    agents = [
        {
            "name": "Hydration daemon",
            "template": "com.wizz-cmd.icloud-nfs-exporter.plist.template",
            "plist": "com.wizz-cmd.icloud-nfs-exporter.plist",
            "replacements": {"__USERNAME__": username},
        },
        {
            "name": "NFS server",
            "template": "com.wizz-cmd.icloud-nfs-server.plist.template",
            "plist": "com.wizz-cmd.icloud-nfs-server.plist",
            "replacements": {
                "__USERNAME__": username,
                "__SOURCE__": source_dir,
                "__PORT__": port,
                "__SOCKET__": socket_path,
            },
        },
    ]

    install_agents = non_interactive or _ask_yn(
        "Install LaunchAgents (auto-start at login)?"
    )

    for agent in agents:
        src = launchd_dir / agent["template"]
        dst = agents_dir / agent["plist"]

        if dst.exists() and not force:
            print(f"  {agent['name']}: already installed")
            continue
        if not src.exists():
            print(f"  {agent['name']}: template not found ({src})")
            continue
        if not install_agents:
            print(f"  {agent['name']}: skipped")
            continue

        content = src.read_text()
        for placeholder, value in agent["replacements"].items():
            content = content.replace(placeholder, value)
        dst.parent.mkdir(parents=True, exist_ok=True)
        dst.write_text(content)
        print(f"  {agent['name']}: installed ({dst})")

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
            if "Rust" in c.name:
                print("    - Install Rust:    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh")
        print("    - Run diagnostics: icne diagnose")
    else:
        print("\n  All systems go. Run 'icne diagnose' to verify.")
    print()
