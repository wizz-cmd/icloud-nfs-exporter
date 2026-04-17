#!/bin/bash
# fskit-test.sh — Self-contained FSKit diagnostic for macOS
# Download and run: curl -sL <raw-url> | bash
# Or: bash fskit-test.sh
set -euo pipefail

PASS="\033[32mPASS\033[0m"
FAIL="\033[31mFAIL\033[0m"
WARN="\033[33mWARN\033[0m"
BOLD="\033[1m"
RESET="\033[0m"

echo ""
echo -e "${BOLD}=== FSKit Diagnostic ===${RESET}"
echo ""

# 1. System info
echo -e "${BOLD}[1/7] System${RESET}"
macos_ver=$(sw_vers -productVersion)
arch=$(uname -m)
echo "  macOS $macos_ver ($arch), $(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo 'unknown CPU')"

# FSKit requires macOS 15.4+
major=$(echo "$macos_ver" | cut -d. -f1)
minor=$(echo "$macos_ver" | cut -d. -f2)
if [ "$major" -gt 15 ] || { [ "$major" -eq 15 ] && [ "$minor" -ge 4 ]; }; then
    echo -e "  macOS >= 15.4: $PASS"
else
    echo -e "  macOS >= 15.4: $FAIL (FSKit requires 15.4+, you have $macos_ver)"
    exit 1
fi

# 2. macFUSE installed?
echo ""
echo -e "${BOLD}[2/7] macFUSE${RESET}"
if [ -f /Library/Filesystems/macfuse.fs/Contents/Info.plist ]; then
    fuse_ver=$(/usr/libexec/PlistBuddy -c "Print :CFBundleVersion" /Library/Filesystems/macfuse.fs/Contents/Info.plist 2>/dev/null || echo "unknown")
    echo "  macFUSE installed: version $fuse_ver"
    # Need >= 5.0.0 for FSKit
    fuse_major=$(echo "$fuse_ver" | cut -d. -f1)
    if [ "$fuse_major" -ge 5 ]; then
        echo -e "  macFUSE >= 5.0: $PASS"
    else
        echo -e "  macFUSE >= 5.0: $FAIL (FSKit needs macFUSE 5+)"
        exit 1
    fi
else
    echo -e "  macFUSE installed: $FAIL"
    echo ""
    echo "  Install with:  brew install --cask macfuse"
    echo "  Then re-run this script."
    exit 1
fi

# 3. PluginKit registration
echo ""
echo -e "${BOLD}[3/7] PluginKit registration${RESET}"
pluginkit_out=$(pluginkit -mA 2>/dev/null | grep -i fuse || true)
if [ -n "$pluginkit_out" ]; then
    echo "  $pluginkit_out"
    if echo "$pluginkit_out" | grep -q "^+"; then
        echo -e "  Module enabled (+): $PASS"
    else
        echo -e "  Module enabled (+): $FAIL (shows '-', enable in System Settings > Privacy & Security > File System Extensions)"
    fi
else
    echo -e "  FSKit module registered: $FAIL (not found in pluginkit)"
fi

# 4. fskitd running?
echo ""
echo -e "${BOLD}[4/7] fskitd process${RESET}"
if pgrep -q fskitd 2>/dev/null; then
    fskitd_pid=$(pgrep fskitd)
    echo -e "  fskitd running (PID $fskitd_pid): $PASS"
else
    echo -e "  fskitd running: $FAIL"
    echo "  (fskitd should auto-launch on demand via launchd)"
    # Try to check if the plist exists
    if [ -f /System/Library/LaunchDaemons/com.apple.fskitd.plist ] || \
       launchctl list 2>/dev/null | grep -q fskitd; then
        echo -e "  fskitd launchd entry exists: $PASS (but not running)"
    else
        echo -e "  fskitd launchd entry: $WARN (not found)"
    fi
fi

# 5. Compile a minimal FUSE hello-world
echo ""
echo -e "${BOLD}[5/7] Build minimal FUSE test binary${RESET}"

TMPDIR=$(mktemp -d /tmp/fskit-test.XXXXXX)
trap "rm -rf '$TMPDIR'; umount /Volumes/fskit-test 2>/dev/null || true" EXIT

cat > "$TMPDIR/hello.c" << 'CEOF'
#define FUSE_USE_VERSION 26
#include <fuse.h>
#include <string.h>
#include <errno.h>

static const char *hello_path = "/hello.txt";
static const char *hello_str  = "FSKit works!\n";

static int hello_getattr(const char *path, struct stat *stbuf) {
    memset(stbuf, 0, sizeof(struct stat));
    if (strcmp(path, "/") == 0) {
        stbuf->st_mode = S_IFDIR | 0755; stbuf->st_nlink = 2; return 0;
    } else if (strcmp(path, hello_path) == 0) {
        stbuf->st_mode = S_IFREG | 0444; stbuf->st_nlink = 1;
        stbuf->st_size = strlen(hello_str); return 0;
    }
    return -ENOENT;
}
static int hello_readdir(const char *path, void *buf, fuse_fill_dir_t filler,
                         off_t offset, struct fuse_file_info *fi) {
    (void)offset; (void)fi;
    if (strcmp(path, "/") != 0) return -ENOENT;
    filler(buf, ".", NULL, 0); filler(buf, "..", NULL, 0);
    filler(buf, hello_path + 1, NULL, 0); return 0;
}
static int hello_open(const char *path, struct fuse_file_info *fi) {
    if (strcmp(path, hello_path) != 0) return -ENOENT;
    return 0;
}
static int hello_read(const char *path, char *buf, size_t size, off_t offset,
                      struct fuse_file_info *fi) {
    (void)fi;
    if (strcmp(path, hello_path) != 0) return -ENOENT;
    size_t len = strlen(hello_str);
    if ((size_t)offset >= len) return 0;
    if (offset + size > len) size = len - offset;
    memcpy(buf, hello_str + offset, size); return size;
}
static struct fuse_operations hello_oper = {
    .getattr  = hello_getattr,
    .readdir  = hello_readdir,
    .open     = hello_open,
    .read     = hello_read,
};
int main(int argc, char *argv[]) {
    return fuse_main(argc, argv, &hello_oper, NULL);
}
CEOF

if cc -Wall -o "$TMPDIR/hello_fuse" "$TMPDIR/hello.c" \
    -I/usr/local/include/fuse -L/usr/local/lib -lfuse -D_FILE_OFFSET_BITS=64 2>"$TMPDIR/cc.log"; then
    echo -e "  Compiled hello_fuse: $PASS"
else
    echo -e "  Compiled hello_fuse: $FAIL"
    cat "$TMPDIR/cc.log"
    echo "  (Need Xcode CLI tools: xcode-select --install)"
    exit 1
fi

# 6. Attempt FSKit mount
echo ""
echo -e "${BOLD}[6/7] FSKit mount test${RESET}"
mkdir -p /Volumes/fskit-test 2>/dev/null || true

# Launch hello_fuse with FSKit backend
"$TMPDIR/hello_fuse" /Volumes/fskit-test -o backend=fskit -f 2>"$TMPDIR/mount.log" &
FUSE_PID=$!
sleep 3

if mount | grep -q "fskit-test"; then
    echo -e "  FSKit mount: $PASS"

    # 7. Read test
    echo ""
    echo -e "${BOLD}[7/7] Read test${RESET}"
    if content=$(cat /Volumes/fskit-test/hello.txt 2>/dev/null); then
        echo -e "  Read /hello.txt: $PASS — \"$(echo "$content" | tr -d '\n')\""
    else
        echo -e "  Read /hello.txt: $FAIL"
    fi
    umount /Volumes/fskit-test 2>/dev/null || true
    kill $FUSE_PID 2>/dev/null || true
    wait $FUSE_PID 2>/dev/null || true
else
    echo -e "  FSKit mount: $FAIL"
    echo ""
    echo "  Mount log:"
    cat "$TMPDIR/mount.log" 2>/dev/null | sed 's/^/    /'
    kill $FUSE_PID 2>/dev/null || true
    wait $FUSE_PID 2>/dev/null || true

    # 7. Kext fallback
    echo ""
    echo -e "${BOLD}[7/7] Kext fallback test${RESET}"
    mkdir -p /tmp/fskit-test-kext 2>/dev/null
    "$TMPDIR/hello_fuse" /tmp/fskit-test-kext -f 2>"$TMPDIR/kext.log" &
    KEXT_PID=$!
    sleep 2
    if mount | grep -q "fskit-test-kext"; then
        content=$(cat /tmp/fskit-test-kext/hello.txt 2>/dev/null || echo "(read failed)")
        echo -e "  Kext mount: $PASS — \"$(echo "$content" | tr -d '\n')\""
        echo "  Conclusion: kext works, FSKit does not."
        umount /tmp/fskit-test-kext 2>/dev/null || true
    else
        echo -e "  Kext mount: $FAIL"
        cat "$TMPDIR/kext.log" 2>/dev/null | sed 's/^/    /'
    fi
    kill $KEXT_PID 2>/dev/null || true
    wait $KEXT_PID 2>/dev/null || true
fi

echo ""
echo -e "${BOLD}=== Done ===${RESET}"
