#!/bin/bash
set -euo pipefail

# Build universal (arm64 + x86_64) release binaries for macOS.
# Usage: ./scripts/build-release.sh [output-dir]

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
OUT="${1:-$ROOT/dist}"

echo "=== icloud-nfs-exporter release build ==="
echo "Output: $OUT"
echo

mkdir -p "$OUT"

# ── Swift packages (universal binary via --arch) ──

for pkg in hydration app helper; do
    echo "Building src/$pkg (Swift, universal)..."
    swift build \
        --package-path "$ROOT/src/$pkg" \
        -c release \
        --arch arm64 --arch x86_64 2>&1 | tail -1
done

# Copy Swift binaries
cp "$ROOT/src/hydration/.build/apple/Products/Release/HydrationDaemon" "$OUT/"
cp "$ROOT/src/app/.build/apple/Products/Release/MenuBarApp" "$OUT/icloud-nfs-exporter-app"
cp "$ROOT/src/helper/.build/apple/Products/Release/PrivilegedHelper" "$OUT/"

echo "  -> HydrationDaemon ($(file -b "$OUT/HydrationDaemon" | grep -o 'arm64\|x86_64' | tr '\n' '+' | sed 's/+$//'))"
echo "  -> icloud-nfs-exporter-app"
echo "  -> PrivilegedHelper"

# ── Rust workspace ──

if command -v cargo &>/dev/null; then
    echo "Building src/fuse (Rust)..."
    cd "$ROOT/src/fuse"

    # Try universal build
    if rustup target list --installed 2>/dev/null | grep -q aarch64-apple-darwin && \
       rustup target list --installed 2>/dev/null | grep -q x86_64-apple-darwin; then
        cargo build --release --target aarch64-apple-darwin 2>&1 | tail -1
        cargo build --release --target x86_64-apple-darwin 2>&1 | tail -1
        lipo -create \
            "target/aarch64-apple-darwin/release/fuse-driver" \
            "target/x86_64-apple-darwin/release/fuse-driver" \
            -output "$OUT/fuse-driver"
        echo "  -> fuse-driver (universal)"
    else
        cargo build --release 2>&1 | tail -1
        cp "target/release/fuse-driver" "$OUT/" 2>/dev/null || true
        echo "  -> fuse-driver (native)"
    fi
    cd "$ROOT"
else
    echo "Rust not installed — skipping fuse-driver."
fi

# ── Python CLI ──

echo "Packaging CLI scripts..."
mkdir -p "$OUT/share/scripts/icne_lib"
cp "$ROOT/scripts/icne" "$OUT/share/scripts/"
cp "$ROOT/scripts/icne_lib/"*.py "$OUT/share/scripts/icne_lib/"
chmod 755 "$OUT/share/scripts/icne"

# ── Launchd template ──

mkdir -p "$OUT/share/launchd"
cp "$ROOT/launchd/"*.template "$OUT/share/launchd/"

# ── Summary ──

echo
echo "=== Build complete ==="
echo "Artifacts in $OUT:"
ls -lh "$OUT"/ 2>/dev/null | grep -v '^total'
echo
echo "Install:  sudo make install PREFIX=/usr/local"
echo "  — or —  cp $OUT/HydrationDaemon /usr/local/bin/"
