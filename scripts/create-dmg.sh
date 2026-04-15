#!/bin/bash
set -euo pipefail

# Create a .dmg disk image containing the .app bundle.
# Usage: ./scripts/create-dmg.sh [build-dir]

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD="${1:-$ROOT/dist}"
APP_NAME="iCloud NFS Exporter"
VERSION="0.1.0"
DMG_NAME="iCloud-NFS-Exporter-${VERSION}"

APP="$BUILD/${APP_NAME}.app"

if [ ! -d "$APP" ]; then
    echo "App bundle not found. Building..."
    bash "$SCRIPT_DIR/create-app-bundle.sh" "$BUILD"
fi

echo "Creating ${DMG_NAME}.dmg ..."

# Stage directory for DMG contents
STAGE="$BUILD/dmg-stage"
rm -rf "$STAGE"
mkdir -p "$STAGE"

# Copy app bundle
cp -R "$APP" "$STAGE/"

# Add Applications symlink for drag-to-install
ln -s /Applications "$STAGE/Applications"

# Add a README
cat > "$STAGE/README.txt" << EOF
iCloud NFS Exporter v${VERSION}

Installation:
  1. Drag "iCloud NFS Exporter" to the Applications folder.
  2. Open the app — it will appear in your menu bar.
  3. To install the command-line tool, open Terminal and run:
     /Applications/iCloud\ NFS\ Exporter.app/Contents/MacOS/install-cli

Getting started:
  icne setup        # Interactive setup wizard
  icne diagnose     # Check system status

More info: https://github.com/wizz-cmd/icloud-nfs-exporter
EOF

# Create DMG
DMG_PATH="$BUILD/${DMG_NAME}.dmg"
rm -f "$DMG_PATH"

hdiutil create \
    -volname "$APP_NAME" \
    -srcfolder "$STAGE" \
    -ov \
    -format UDZO \
    "$DMG_PATH"

rm -rf "$STAGE"

echo "Created: $DMG_PATH"
echo "  $(du -sh "$DMG_PATH" | cut -f1)"

# Checksum
shasum -a 256 "$DMG_PATH" | tee "$DMG_PATH.sha256"
