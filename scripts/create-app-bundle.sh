#!/bin/bash
set -euo pipefail

# Create a macOS .app bundle from built binaries.
# Usage: ./scripts/create-app-bundle.sh [build-dir]

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD="${1:-$ROOT/dist}"
APP_NAME="iCloud NFS Exporter"
BUNDLE_ID="com.wizz-cmd.icloud-nfs-exporter"
VERSION="0.1.0"

APP="$BUILD/${APP_NAME}.app"
CONTENTS="$APP/Contents"

echo "Creating ${APP_NAME}.app ..."

rm -rf "$APP"
mkdir -p "$CONTENTS/MacOS"
mkdir -p "$CONTENTS/Resources/scripts/icne_lib"
mkdir -p "$CONTENTS/Resources/launchd"

# ── Info.plist ──

cat > "$CONTENTS/Info.plist" << PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleDisplayName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleIdentifier</key>
    <string>${BUNDLE_ID}</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundleExecutable</key>
    <string>MenuBarApp</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSMinimumSystemVersion</key>
    <string>13.0</string>
    <key>LSUIElement</key>
    <true/>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.utilities</string>
</dict>
</plist>
PLIST

# ── Binaries ──

# Build release if not already built
if [ ! -f "$ROOT/src/app/.build/release/MenuBarApp" ]; then
    echo "Building release binaries..."
    swift build --package-path "$ROOT/src/app" -c release
    swift build --package-path "$ROOT/src/hydration" -c release
fi

cp "$ROOT/src/app/.build/release/MenuBarApp" "$CONTENTS/MacOS/"
cp "$ROOT/src/hydration/.build/release/HydrationDaemon" "$CONTENTS/MacOS/"

# ── CLI + resources ──

cp "$ROOT/scripts/icne" "$CONTENTS/Resources/scripts/"
cp "$ROOT/scripts/icne_lib/"*.py "$CONTENTS/Resources/scripts/icne_lib/"
chmod 755 "$CONTENTS/Resources/scripts/icne"
cp "$ROOT/launchd/"*.template "$CONTENTS/Resources/launchd/"

# ── Wrapper script for CLI install ──

cat > "$CONTENTS/MacOS/install-cli" << 'WRAPPER'
#!/bin/bash
# Install the icne CLI to /usr/local/bin
set -euo pipefail
RESOURCES="$(cd "$(dirname "$0")/../Resources" && pwd)"
sudo mkdir -p /usr/local/bin
sudo ln -sf "$RESOURCES/scripts/icne" /usr/local/bin/icne
echo "Installed: /usr/local/bin/icne"
echo "Run 'icne setup' to get started."
WRAPPER
chmod 755 "$CONTENTS/MacOS/install-cli"

echo "Created: $APP"
echo "  $(du -sh "$APP" | cut -f1) total"
