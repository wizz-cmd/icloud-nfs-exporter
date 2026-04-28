# End-to-End Testing Guide

This guide walks you through testing icloud-nfs-exporter on your local network. You need:

- **The Mac** that has iCloud Drive (your Intel mini or M2 MacBook) — this runs the server
- **A client machine** — another Mac or a Linux box on the same LAN

---

## 1. Build

On the Mac that will run the server:

```bash
cd ~/projects/icloud-nfs-exporter

# Build hydration daemon (Swift)
swift build --package-path src/hydration -c release

# Build NFS server (Rust)
cd src/nfs && cargo build --release && cd ../..
```

After this you have two binaries:
- `src/hydration/.build/release/HydrationDaemon`
- `src/nfs/target/release/nfs-server`

---

## 2. Start the Services

You need two terminal windows on the server Mac.

**Terminal 1 — Hydration Daemon:**

```bash
src/hydration/.build/release/HydrationDaemon \
  --watch ~/Library/Mobile\ Documents/com~apple~CloudDocs \
  --socket /tmp/icloud-nfs-exporter.sock
```

You should see it start and listen. Leave it running.

**Terminal 2 — NFS Server:**

```bash
src/nfs/target/release/nfs-server serve \
  ~/Library/Mobile\ Documents/com~apple~CloudDocs \
  --port 11111 \
  --socket /tmp/icloud-nfs-exporter.sock \
  --staging-dir ~/.icne-staging \
  --promotion-delay 5
```

You should see:
```
Serving /Users/.../CloudDocs via NFSv3 on port 11111 (read-write)
Staging: /Users/.../.icne-staging
Promotion delay: 5s
```

> **Tip:** Add `RUST_LOG=info` before the command to see hydration and promotion activity:
> ```bash
> RUST_LOG=info src/nfs/target/release/nfs-server serve ...
> ```

---

## 3. Mount — macOS Client

On another Mac (or on the same Mac for a quick test):

```bash
# Create a mount point
mkdir -p /tmp/icloud-nfs

# Mount (replace SERVER_IP with the server Mac's IP address)
mount_nfs -o vers=3,tcp,port=11111,mountport=11111,nolocks SERVER_IP:/ /tmp/icloud-nfs
```

For localhost testing on the same Mac:
```bash
mount_nfs -o vers=3,tcp,port=11111,mountport=11111,nolocks 127.0.0.1:/ /tmp/icloud-nfs
```

To unmount:
```bash
umount /tmp/icloud-nfs
```

---

## 4. Mount — Linux Client

On a Linux machine on the same network:

```bash
# Install NFS client tools (Debian/Ubuntu)
sudo apt-get install nfs-common

# Create mount point
sudo mkdir -p /mnt/icloud

# Mount (replace SERVER_IP with the Mac's IP)
sudo mount.nfs -o vers=3,tcp,port=11111,mountport=11111,nolock SERVER_IP:/ /mnt/icloud
```

To unmount:
```bash
sudo umount /mnt/icloud
```

---

## 5. Test: Reading Files

These tests verify that iCloud files are served correctly, including cloud-only files.

### 5a. List directory contents

```bash
ls /tmp/icloud-nfs/
```

**Expected:** You see your iCloud Drive files and folders by their real names — no `.icloud` stub files visible.

### 5b. Read a local file

Pick a file you know is already downloaded on the Mac:

```bash
cat /tmp/icloud-nfs/some-local-file.txt
```

**Expected:** File contents appear immediately.

### 5c. Read a cloud-only file

Pick a file that shows a cloud icon in Finder (not downloaded). Try reading it:

```bash
cat /tmp/icloud-nfs/some-cloud-file.pdf > /dev/null
```

**Expected:** Brief pause while the file downloads, then it succeeds. In the server terminal (with `RUST_LOG=info`), you should see hydration activity.

### 5d. List a large directory

```bash
ls -la /tmp/icloud-nfs/Documents/
```

**Expected:** All files listed with correct sizes. Cloud-only files may show small stub sizes until read.

---

## 6. Test: Writing Files

These tests verify the new staging layer.

### 6a. Create a new file

```bash
echo "hello from NFS" > /tmp/icloud-nfs/nfs-test-file.txt
```

**Check staging** (on the server Mac):
```bash
cat ~/.icne-staging/nfs-test-file.txt
```

**Expected:** File appears in staging immediately with content "hello from NFS".

**Wait 5+ seconds**, then check iCloud:
```bash
cat ~/Library/Mobile\ Documents/com~apple~CloudDocs/nfs-test-file.txt
```

**Expected:** File has been promoted to iCloud Drive. It will eventually sync to iCloud (check on another device or icloud.com).

### 6b. Copy a larger file

```bash
cp /usr/share/dict/words /tmp/icloud-nfs/wordlist.txt
```

**Check staging:**
```bash
ls -la ~/.icne-staging/wordlist.txt
```

**Expected:** File in staging, same size as the original. After promotion delay, it moves to iCloud Drive.

### 6c. Modify an existing iCloud file

Pick a file that already exists in iCloud:

```bash
echo "appended via NFS" >> /tmp/icloud-nfs/some-existing-file.txt
```

**Expected:**
1. The server hydrates the file (downloads if cloud-only)
2. Copies it to staging (copy-up)
3. Appends the text to the staging copy
4. After 5s, promotes the modified version back to iCloud

Verify:
```bash
tail -1 ~/Library/Mobile\ Documents/com~apple~CloudDocs/some-existing-file.txt
# Should show: "appended via NFS"
```

### 6d. Create a directory

```bash
mkdir /tmp/icloud-nfs/test-dir
```

**Expected:** Directory appears in both staging and iCloud Drive immediately (directories are created in both places at once).

```bash
ls -d ~/Library/Mobile\ Documents/com~apple~CloudDocs/test-dir
ls -d ~/.icne-staging/test-dir
```

### 6e. Delete a file

```bash
rm /tmp/icloud-nfs/nfs-test-file.txt
```

**Expected:** File is removed from iCloud Drive (and from staging if it was there). Verify:
```bash
ls ~/Library/Mobile\ Documents/com~apple~CloudDocs/nfs-test-file.txt
# Should say: No such file or directory
```

### 6f. Rename a file

```bash
mv /tmp/icloud-nfs/wordlist.txt /tmp/icloud-nfs/words-renamed.txt
```

**Expected:** File is renamed in iCloud Drive. Old name gone, new name present.

---

## 7. Test: Linux Client Writes

Repeat tests 6a–6f from the Linux client. The behavior should be identical.

Additionally, test a typical Linux workflow:

```bash
# Create a file with vi/nano
echo "linux test" > /mnt/icloud/from-linux.txt

# Verify it reads back
cat /mnt/icloud/from-linux.txt

# Copy a binary file
cp /bin/ls /mnt/icloud/ls-backup

# Delete it
rm /mnt/icloud/ls-backup
```

---

## 8. Test: Edge Cases

### 8a. Large file write

```bash
dd if=/dev/urandom of=/tmp/icloud-nfs/large-test.bin bs=1M count=100
```

**Expected:** 100 MB file appears in staging, then gets promoted. Check that the file size matches after promotion.

### 8b. Many small files

```bash
for i in $(seq 1 50); do echo "file $i" > /tmp/icloud-nfs/batch-$i.txt; done
```

**Expected:** All 50 files appear in staging, then get promoted one by one.

### 8c. Concurrent read + write

While writing a file from one client, try reading a different file from another client. Both should work without blocking each other.

### 8d. Symlink (should fail gracefully)

```bash
ln -s /tmp/icloud-nfs/some-file.txt /tmp/icloud-nfs/link-test
```

**Expected:** Error message (operation not supported). iCloud doesn't preserve symlinks, so we reject this cleanly.

---

## 9. Cleanup

After testing:

```bash
# On the client: unmount
umount /tmp/icloud-nfs          # macOS
sudo umount /mnt/icloud         # Linux

# On the server: stop with Ctrl+C in both terminals

# Remove test files from iCloud Drive
rm ~/Library/Mobile\ Documents/com~apple~CloudDocs/nfs-test-file.txt
rm ~/Library/Mobile\ Documents/com~apple~CloudDocs/wordlist.txt
rm ~/Library/Mobile\ Documents/com~apple~CloudDocs/words-renamed.txt
rm ~/Library/Mobile\ Documents/com~apple~CloudDocs/from-linux.txt
rm ~/Library/Mobile\ Documents/com~apple~CloudDocs/large-test.bin
rm ~/Library/Mobile\ Documents/com~apple~CloudDocs/batch-*.txt
rmdir ~/Library/Mobile\ Documents/com~apple~CloudDocs/test-dir

# Clean staging directory
rm -rf ~/.icne-staging/*
```

---

## 10. What to Watch For

When something goes wrong, check:

1. **Server terminal** — error messages, hydration failures
2. **`RUST_LOG=info`** — shows promotion activity, copy-up events, IPC calls
3. **Staging directory** — files stuck in `~/.icne-staging/` that never promoted
4. **Hydration daemon** — is it still running? Test with:
   ```bash
   src/nfs/target/release/nfs-server ping --socket /tmp/icloud-nfs-exporter.sock
   ```

### Common issues

| Symptom | Likely cause |
|---------|-------------|
| `mount: Connection refused` | NFS server not running, or wrong port |
| Files stuck in staging | Hydration daemon not running, or promotion delay not elapsed |
| `Stale file handle` | NFS server was restarted while client was mounted — remount |
| Cloud-only file hangs on read | Hydration daemon can't reach iCloud — check network |
| `Operation not permitted` on write | Mount is read-only — check mount options |
| Large file promotion slow | Expected — `cp` to iCloud folder + iCloud upload takes time |

---

## 11. Success Criteria

All tests pass when:

- [ ] macOS client can read local and cloud-only files
- [ ] macOS client can write, create, delete, rename, mkdir
- [ ] Written files appear in staging, then promote to iCloud Drive
- [ ] Promoted files sync to iCloud (visible on icloud.com or another device)
- [ ] Linux client can read files
- [ ] Linux client can write files
- [ ] Large files (100 MB+) work
- [ ] Batch operations (50+ files) work
- [ ] No crashes or hangs under normal use
