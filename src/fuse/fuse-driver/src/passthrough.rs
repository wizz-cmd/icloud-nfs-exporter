use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::{self, File, Metadata};
use std::os::unix::fs::{DirEntryExt, FileExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fuser::{
    Errno, FileAttr, FileHandle, FileType, Filesystem, FopenFlags, Generation, INodeNo,
    LockOwner, ReplyAttr, ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry, ReplyOpen,
    ReplyStatfs, Request,
};
use log::{debug, error, warn};

use fuse_core::IpcClient;
use fuse_core::path_utils::{is_icloud_stub, real_to_stub_name, stub_to_real_name};

const TTL: Duration = Duration::from_secs(1);
const ROOT_INO: INodeNo = INodeNo(1);

struct InodeData {
    real_path: PathBuf,
    kind: FileType,
    lookup_count: u64,
}

struct HandleData {
    file: File,
}

pub struct IcloudFs {
    source: PathBuf,
    ipc: IpcClient,
    inodes: RwLock<HashMap<u64, InodeData>>,
    next_ino: AtomicU64,
    handles: RwLock<HashMap<u64, HandleData>>,
    next_fh: AtomicU64,
    uid: u32,
    gid: u32,
}

impl IcloudFs {
    pub fn new(source: PathBuf, ipc: IpcClient) -> Self {
        let uid = unsafe { libc::getuid() };
        let gid = unsafe { libc::getgid() };
        let mut inodes = HashMap::new();
        inodes.insert(1, InodeData {
            real_path: source.clone(),
            kind: FileType::Directory,
            lookup_count: 1,
        });
        Self {
            source,
            ipc,
            inodes: RwLock::new(inodes),
            next_ino: AtomicU64::new(2),
            handles: RwLock::new(HashMap::new()),
            next_fh: AtomicU64::new(1),
            uid,
            gid,
        }
    }
}

fn meta_to_attr(ino: u64, meta: &Metadata, uid: u32, gid: u32) -> FileAttr {
    let kind = if meta.is_dir() {
        FileType::Directory
    } else if meta.is_symlink() {
        FileType::Symlink
    } else {
        FileType::RegularFile
    };
    let nlink = if meta.is_dir() { 2 } else { 1 };

    FileAttr {
        ino: INodeNo(ino),
        size: meta.len(),
        blocks: meta.blocks(),
        atime: meta.accessed().unwrap_or(UNIX_EPOCH),
        mtime: meta.modified().unwrap_or(UNIX_EPOCH),
        ctime: SystemTime::UNIX_EPOCH
            + Duration::from_secs(meta.ctime() as u64),
        crtime: meta.created().unwrap_or(UNIX_EPOCH),
        kind,
        perm: (meta.mode() & 0o7777) as u16,
        nlink,
        uid,
        gid,
        rdev: meta.rdev() as u32,
        blksize: meta.blksize() as u32,
        flags: 0,
    }
}

/// Find the real path for a child name under a parent directory.
/// Returns (path_on_disk, presented_name, is_stub).
fn resolve_child(parent: &Path, name: &OsStr) -> Option<(PathBuf, bool)> {
    let name_str = name.to_str().unwrap_or("");

    // Try the literal name first
    let direct = parent.join(name);
    if direct.symlink_metadata().is_ok() {
        return Some((direct, false));
    }

    // Try the stub form: name -> .name.icloud
    if !name_str.is_empty() {
        let stub_name = real_to_stub_name(name_str);
        let stub_path = parent.join(&stub_name);
        if stub_path.symlink_metadata().is_ok() {
            return Some((stub_path, true));
        }
    }

    None
}

impl Filesystem for IcloudFs {
    fn init(
        &mut self,
        _req: &Request,
        _config: &mut fuser::KernelConfig,
    ) -> std::io::Result<()> {
        log::info!("icloud-nfs-exporter FUSE mounted: {}", self.source.display());
        Ok(())
    }

    fn destroy(&mut self) {
        log::info!("icloud-nfs-exporter FUSE unmounted");
    }

    fn lookup(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
        let parent_path = {
            let inodes = self.inodes.read().unwrap();
            match inodes.get(&parent.0) {
                Some(data) => data.real_path.clone(),
                None => {
                    reply.error(Errno::ENOENT);
                    return;
                }
            }
        };

        let (real_path, _is_stub) = match resolve_child(&parent_path, name) {
            Some(v) => v,
            None => {
                reply.error(Errno::ENOENT);
                return;
            }
        };

        let meta = match fs::symlink_metadata(&real_path) {
            Ok(m) => m,
            Err(_) => {
                reply.error(Errno::ENOENT);
                return;
            }
        };

        let kind = if meta.is_dir() {
            FileType::Directory
        } else {
            FileType::RegularFile
        };

        // Check if this path already has an inode
        let mut inodes = self.inodes.write().unwrap();
        let ino = inodes
            .iter()
            .find(|(_, d)| d.real_path == real_path)
            .map(|(k, _)| *k);

        let ino = match ino {
            Some(existing) => {
                inodes.get_mut(&existing).unwrap().lookup_count += 1;
                existing
            }
            None => {
                let new_ino = self.next_ino.fetch_add(1, Ordering::Relaxed);
                inodes.insert(new_ino, InodeData {
                    real_path: real_path.clone(),
                    kind,
                    lookup_count: 1,
                });
                new_ino
            }
        };

        let attr = meta_to_attr(ino, &meta, self.uid, self.gid);
        reply.entry(&TTL, &attr, Generation(0));
    }

    fn forget(&self, _req: &Request, ino: INodeNo, nlookup: u64) {
        if ino == ROOT_INO {
            return;
        }
        let mut inodes = self.inodes.write().unwrap();
        if let Some(data) = inodes.get_mut(&ino.0) {
            data.lookup_count = data.lookup_count.saturating_sub(nlookup);
            if data.lookup_count == 0 {
                inodes.remove(&ino.0);
            }
        }
    }

    fn getattr(&self, _req: &Request, ino: INodeNo, _fh: Option<FileHandle>, reply: ReplyAttr) {
        let real_path = {
            let inodes = self.inodes.read().unwrap();
            match inodes.get(&ino.0) {
                Some(data) => data.real_path.clone(),
                None => {
                    reply.error(Errno::ENOENT);
                    return;
                }
            }
        };

        match fs::symlink_metadata(&real_path) {
            Ok(meta) => {
                let attr = meta_to_attr(ino.0, &meta, self.uid, self.gid);
                reply.attr(&TTL, &attr);
            }
            Err(_) => reply.error(Errno::ENOENT),
        }
    }

    fn opendir(&self, _req: &Request, ino: INodeNo, _flags: fuser::OpenFlags, reply: ReplyOpen) {
        let inodes = self.inodes.read().unwrap();
        match inodes.get(&ino.0) {
            Some(data) if data.kind == FileType::Directory => {
                reply.opened(FileHandle(0), FopenFlags::empty());
            }
            _ => reply.error(Errno::ENOENT),
        }
    }

    fn readdir(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        mut reply: ReplyDirectory,
    ) {
        let dir_path = {
            let inodes = self.inodes.read().unwrap();
            match inodes.get(&ino.0) {
                Some(data) => data.real_path.clone(),
                None => {
                    reply.error(Errno::ENOENT);
                    return;
                }
            }
        };

        let entries = match fs::read_dir(&dir_path) {
            Ok(rd) => rd,
            Err(_) => {
                reply.error(Errno::EIO);
                return;
            }
        };

        // Collect and sort entries for deterministic offsets
        let mut all_entries: Vec<(u64, INodeNo, FileType, String)> = Vec::new();

        // . and ..
        all_entries.push((1, ino, FileType::Directory, ".".to_string()));
        let parent_ino = if ino == ROOT_INO { ROOT_INO } else {
            // Find parent by checking if any inode's path is the parent of dir_path
            let inodes = self.inodes.read().unwrap();
            let parent = dir_path.parent();
            parent
                .and_then(|p| inodes.iter().find(|(_, d)| d.real_path == p).map(|(k, _)| INodeNo(*k)))
                .unwrap_or(ROOT_INO)
        };
        all_entries.push((2, parent_ino, FileType::Directory, "..".to_string()));

        // Build a set of real file names present in the directory (non-stub names)
        // so we can skip stubs whose real counterpart already exists.
        let dir_entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
        let real_names: std::collections::HashSet<String> = dir_entries
            .iter()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if !is_icloud_stub(&name) {
                    Some(name)
                } else {
                    None
                }
            })
            .collect();

        for entry in &dir_entries {
            let os_name = entry.file_name();
            let name = os_name.to_string_lossy().to_string();
            let ft = entry.file_type().map(|ft| {
                if ft.is_dir() { FileType::Directory }
                else { FileType::RegularFile }
            }).unwrap_or(FileType::RegularFile);

            let entry_ino = INodeNo(entry.ino());

            if is_icloud_stub(&name) {
                // Translate stub to real name
                if let Some(real_name) = stub_to_real_name(&name) {
                    // Skip if the real file already exists (no duplication)
                    if real_names.contains(&real_name) {
                        continue;
                    }
                    all_entries.push((
                        all_entries.len() as u64 + 1,
                        entry_ino,
                        ft,
                        real_name,
                    ));
                }
            } else {
                all_entries.push((
                    all_entries.len() as u64 + 1,
                    entry_ino,
                    ft,
                    name,
                ));
            }
        }

        // Reply starting from offset
        for (i, (_, entry_ino, kind, name)) in all_entries.iter().enumerate() {
            let entry_offset = (i + 1) as u64;
            if entry_offset <= offset {
                continue;
            }
            if reply.add(*entry_ino, entry_offset, *kind, name) {
                // Buffer full
                break;
            }
        }
        reply.ok();
    }

    fn releasedir(
        &self,
        _req: &Request,
        _ino: INodeNo,
        _fh: FileHandle,
        _flags: fuser::OpenFlags,
        reply: ReplyEmpty,
    ) {
        reply.ok();
    }

    fn open(&self, _req: &Request, ino: INodeNo, flags: fuser::OpenFlags, reply: ReplyOpen) {
        // Read-only filesystem
        if (flags.0 & libc::O_WRONLY) != 0 || (flags.0 & libc::O_RDWR) != 0 {
            reply.error(Errno::EROFS);
            return;
        }

        let real_path = {
            let inodes = self.inodes.read().unwrap();
            match inodes.get(&ino.0) {
                Some(data) => data.real_path.clone(),
                None => {
                    reply.error(Errno::ENOENT);
                    return;
                }
            }
        };

        let file_name = real_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // Hydration: if this is an iCloud stub, hydrate before opening
        let open_path = if is_icloud_stub(file_name) {
            let stub_path_str = match real_path.to_str() {
                Some(s) => s.to_string(),
                None => {
                    error!("non-UTF-8 path: {}", real_path.display());
                    reply.error(Errno::EIO);
                    return;
                }
            };

            debug!("hydrating stub: {}", stub_path_str);

            match self.ipc.hydrate(&stub_path_str) {
                Ok(()) => {
                    // Hydration succeeded — the real file now exists
                    let real_name = match stub_to_real_name(file_name) {
                        Some(n) => n,
                        None => {
                            reply.error(Errno::EIO);
                            return;
                        }
                    };
                    let hydrated_path = real_path.parent().unwrap().join(&real_name);

                    // Update the inode table to point to the hydrated file
                    {
                        let mut inodes = self.inodes.write().unwrap();
                        if let Some(data) = inodes.get_mut(&ino.0) {
                            data.real_path = hydrated_path.clone();
                        }
                    }

                    hydrated_path
                }
                Err(e) => {
                    warn!("hydration failed for {}: {}", stub_path_str, e);
                    reply.error(Errno::EIO);
                    return;
                }
            }
        } else {
            real_path
        };

        // Open the file
        match File::open(&open_path) {
            Ok(file) => {
                let fh = self.next_fh.fetch_add(1, Ordering::Relaxed);
                self.handles.write().unwrap().insert(fh, HandleData { file });
                reply.opened(FileHandle(fh), FopenFlags::empty());
            }
            Err(e) => {
                reply.error(Errno::from(e));
            }
        }
    }

    fn read(
        &self,
        _req: &Request,
        _ino: INodeNo,
        fh: FileHandle,
        offset: u64,
        size: u32,
        _flags: fuser::OpenFlags,
        _lock_owner: Option<LockOwner>,
        reply: ReplyData,
    ) {
        let handles = self.handles.read().unwrap();
        let handle = match handles.get(&fh.0) {
            Some(h) => h,
            None => {
                reply.error(Errno::EBADF);
                return;
            }
        };

        let mut buf = vec![0u8; size as usize];
        match handle.file.read_at(&mut buf, offset) {
            Ok(n) => reply.data(&buf[..n]),
            Err(e) => {
                reply.error(Errno::from(e));
            }
        }
    }

    fn release(
        &self,
        _req: &Request,
        _ino: INodeNo,
        fh: FileHandle,
        _flags: fuser::OpenFlags,
        _lock_owner: Option<LockOwner>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        self.handles.write().unwrap().remove(&fh.0);
        reply.ok();
    }

    fn statfs(&self, _req: &Request, _ino: INodeNo, reply: ReplyStatfs) {
        let c_path = match std::ffi::CString::new(self.source.to_string_lossy().as_bytes()) {
            Ok(p) => p,
            Err(_) => {
                reply.error(Errno::EIO);
                return;
            }
        };

        unsafe {
            let mut stat: libc::statfs = std::mem::zeroed();
            if libc::statfs(c_path.as_ptr(), &mut stat) == 0 {
                reply.statfs(
                    stat.f_blocks,
                    stat.f_bfree,
                    stat.f_bavail,
                    stat.f_files,
                    stat.f_ffree,
                    stat.f_bsize as u32,
                    255, // namelen
                    stat.f_bsize as u32, // frsize
                );
            } else {
                reply.error(Errno::EIO);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn resolve_child_real_file() {
        let dir = std::env::temp_dir().join("icne-test-resolve-real");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("hello.txt"), b"hi").unwrap();

        let (path, is_stub) = resolve_child(&dir, OsStr::new("hello.txt")).unwrap();
        assert_eq!(path, dir.join("hello.txt"));
        assert!(!is_stub);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_child_stub_file() {
        let dir = std::env::temp_dir().join("icne-test-resolve-stub");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(".Report.pdf.icloud"), b"stub").unwrap();

        // Asking for "Report.pdf" finds the stub
        let (path, is_stub) = resolve_child(&dir, OsStr::new("Report.pdf")).unwrap();
        assert_eq!(path, dir.join(".Report.pdf.icloud"));
        assert!(is_stub);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_child_not_found() {
        let dir = std::env::temp_dir().join("icne-test-resolve-none");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        assert!(resolve_child(&dir, OsStr::new("nope.txt")).is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn meta_to_attr_file() {
        let dir = std::env::temp_dir().join("icne-test-attr");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let f = dir.join("test.txt");
        fs::write(&f, b"hello world").unwrap();

        let meta = fs::metadata(&f).unwrap();
        let attr = meta_to_attr(42, &meta, 501, 20);
        assert_eq!(attr.ino, INodeNo(42));
        assert_eq!(attr.size, 11);
        assert_eq!(attr.kind, FileType::RegularFile);
        assert_eq!(attr.nlink, 1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn meta_to_attr_dir() {
        let dir = std::env::temp_dir().join("icne-test-attr-dir");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let meta = fs::metadata(&dir).unwrap();
        let attr = meta_to_attr(1, &meta, 501, 20);
        assert_eq!(attr.kind, FileType::Directory);
        assert_eq!(attr.nlink, 2);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn icloud_fs_new_seeds_root() {
        let dir = std::env::temp_dir().join("icne-test-new");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let ipc = IpcClient::new("/tmp/nonexistent.sock");
        let fs = IcloudFs::new(dir.clone(), ipc);

        let inodes = fs.inodes.read().unwrap();
        assert!(inodes.contains_key(&1));
        assert_eq!(inodes[&1].real_path, dir);
        assert_eq!(inodes[&1].kind, FileType::Directory);
        assert_eq!(fs.next_ino.load(Ordering::Relaxed), 2);

        let _ = fs::remove_dir_all(&dir);
    }
}
