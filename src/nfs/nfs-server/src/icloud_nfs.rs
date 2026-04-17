use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::os::unix::fs::{FileExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::SystemTime;

use async_trait::async_trait;
use log::{debug, warn};
use nfsserve::nfs::*;
use nfsserve::vfs::{DirEntry, NFSFileSystem, ReadDirResult, VFSCapabilities};

use fuse_core::IpcClient;
use fuse_core::path_utils::{is_icloud_stub, real_to_stub_name, stub_to_real_name};

const ROOT_ID: fileid3 = 1;

struct InodeData {
    real_path: PathBuf,
    kind: ftype3,
}

pub struct IcloudNfs {
    source: PathBuf,
    socket_path: String,
    inodes: RwLock<HashMap<fileid3, InodeData>>,
    next_ino: AtomicU64,
    uid: u32,
    gid: u32,
}

impl IcloudNfs {
    pub fn new(source: PathBuf, socket_path: &str) -> Self {
        let uid = unsafe { libc::getuid() };
        let gid = unsafe { libc::getgid() };
        let mut inodes = HashMap::new();
        inodes.insert(ROOT_ID, InodeData {
            real_path: source.clone(),
            kind: ftype3::NF3DIR,
        });
        Self {
            source,
            socket_path: socket_path.to_string(),
            inodes: RwLock::new(inodes),
            next_ino: AtomicU64::new(2),
            uid,
            gid,
        }
    }

    fn get_path(&self, id: fileid3) -> Result<PathBuf, nfsstat3> {
        let inodes = self.inodes.read().unwrap();
        inodes
            .get(&id)
            .map(|d| d.real_path.clone())
            .ok_or(nfsstat3::NFS3ERR_STALE)
    }

    fn get_or_alloc_inode(&self, real_path: PathBuf, kind: ftype3) -> fileid3 {
        let mut inodes = self.inodes.write().unwrap();
        if let Some((ino, _)) = inodes.iter().find(|(_, d)| d.real_path == real_path) {
            return *ino;
        }
        let ino = self.next_ino.fetch_add(1, Ordering::Relaxed);
        inodes.insert(ino, InodeData { real_path, kind });
        ino
    }

    /// If the inode points to a .icloud stub, hydrate via IPC and update the
    /// inode table to point at the hydrated file.  Returns the path to read.
    async fn ensure_hydrated(&self, id: fileid3) -> Result<PathBuf, nfsstat3> {
        let real_path = self.get_path(id)?;

        let file_name = real_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if !is_icloud_stub(file_name) {
            return Ok(real_path);
        }

        let stub_path_str = real_path
            .to_str()
            .ok_or(nfsstat3::NFS3ERR_IO)?
            .to_string();
        let socket = self.socket_path.clone();

        debug!("hydrating stub: {}", stub_path_str);

        let result = tokio::task::spawn_blocking(move || {
            let client = IpcClient::new(&socket);
            client.hydrate(&stub_path_str)
        })
        .await
        .map_err(|_| nfsstat3::NFS3ERR_IO)?;

        result.map_err(|e| {
            warn!("hydration failed: {}", e);
            nfsstat3::NFS3ERR_IO
        })?;

        let real_name = stub_to_real_name(file_name).ok_or(nfsstat3::NFS3ERR_IO)?;
        let hydrated_path = real_path.parent().unwrap().join(&real_name);

        {
            let mut inodes = self.inodes.write().unwrap();
            if let Some(data) = inodes.get_mut(&id) {
                data.real_path = hydrated_path.clone();
            }
        }

        Ok(hydrated_path)
    }
}

fn meta_to_fattr3(ino: fileid3, meta: &std::fs::Metadata, uid: u32, gid: u32) -> fattr3 {
    let ftype = if meta.is_dir() {
        ftype3::NF3DIR
    } else if meta.is_symlink() {
        ftype3::NF3LNK
    } else {
        ftype3::NF3REG
    };
    let nlink = if meta.is_dir() { 2 } else { 1 };

    fattr3 {
        ftype,
        mode: meta.mode() & 0o7777,
        nlink,
        uid,
        gid,
        size: meta.len() as u64,
        used: meta.blocks() as u64 * 512,
        rdev: specdata3 {
            specdata1: 0,
            specdata2: 0,
        },
        fsid: 0,
        fileid: ino,
        atime: system_time_to_nfstime(meta.accessed()),
        mtime: system_time_to_nfstime(meta.modified()),
        ctime: nfstime3 {
            seconds: meta.ctime() as u32,
            nseconds: meta.ctime_nsec() as u32,
        },
    }
}

fn system_time_to_nfstime(t: std::io::Result<SystemTime>) -> nfstime3 {
    match t {
        Ok(t) => {
            let d = t
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            nfstime3 {
                seconds: d.as_secs() as u32,
                nseconds: d.subsec_nanos(),
            }
        }
        Err(_) => nfstime3 {
            seconds: 0,
            nseconds: 0,
        },
    }
}

/// Find the real path for a child name under a parent directory.
/// Returns (path_on_disk, is_stub).
fn resolve_child(parent: &Path, name: &str) -> Option<(PathBuf, bool)> {
    // Try the literal name first
    let direct = parent.join(name);
    if direct.symlink_metadata().is_ok() {
        return Some((direct, false));
    }

    // Try the stub form: name -> .name.icloud
    if !name.is_empty() {
        let stub_name = real_to_stub_name(name);
        let stub_path = parent.join(&stub_name);
        if stub_path.symlink_metadata().is_ok() {
            return Some((stub_path, true));
        }
    }

    None
}

#[async_trait]
impl NFSFileSystem for IcloudNfs {
    fn root_dir(&self) -> fileid3 {
        ROOT_ID
    }

    fn capabilities(&self) -> VFSCapabilities {
        VFSCapabilities::ReadOnly
    }

    async fn lookup(&self, dirid: fileid3, filename: &filename3) -> Result<fileid3, nfsstat3> {
        let name = std::str::from_utf8(filename).map_err(|_| nfsstat3::NFS3ERR_NOENT)?;
        let parent_path = self.get_path(dirid)?;

        let (real_path, _is_stub) =
            resolve_child(&parent_path, name).ok_or(nfsstat3::NFS3ERR_NOENT)?;

        let meta =
            fs::symlink_metadata(&real_path).map_err(|_| nfsstat3::NFS3ERR_NOENT)?;

        let kind = if meta.is_dir() {
            ftype3::NF3DIR
        } else if meta.is_symlink() {
            ftype3::NF3LNK
        } else {
            ftype3::NF3REG
        };

        Ok(self.get_or_alloc_inode(real_path, kind))
    }

    async fn getattr(&self, id: fileid3) -> Result<fattr3, nfsstat3> {
        let real_path = self.get_path(id)?;
        let meta = fs::symlink_metadata(&real_path).map_err(|_| nfsstat3::NFS3ERR_STALE)?;
        Ok(meta_to_fattr3(id, &meta, self.uid, self.gid))
    }

    async fn read(
        &self,
        id: fileid3,
        offset: u64,
        count: u32,
    ) -> Result<(Vec<u8>, bool), nfsstat3> {
        let open_path = self.ensure_hydrated(id).await?;

        let file = File::open(&open_path).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let file_len = file.metadata().map(|m| m.len()).unwrap_or(0);

        let mut buf = vec![0u8; count as usize];
        let bytes_read = file.read_at(&mut buf, offset).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        buf.truncate(bytes_read);

        let eof = offset + bytes_read as u64 >= file_len;
        Ok((buf, eof))
    }

    async fn readdir(
        &self,
        dirid: fileid3,
        start_after: fileid3,
        max_entries: usize,
    ) -> Result<ReadDirResult, nfsstat3> {
        let dir_path = self.get_path(dirid)?;

        let entries_iter = fs::read_dir(&dir_path).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let dir_entries: Vec<_> = entries_iter.filter_map(|e| e.ok()).collect();

        // Build set of real (non-stub) names for deduplication
        let real_names: HashSet<String> = dir_entries
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

        let mut all: Vec<DirEntry> = Vec::new();

        for entry in &dir_entries {
            let os_name = entry.file_name();
            let name = os_name.to_string_lossy().to_string();

            let meta = match fs::symlink_metadata(entry.path()) {
                Ok(m) => m,
                Err(_) => continue,
            };

            let kind = if meta.is_dir() {
                ftype3::NF3DIR
            } else if meta.is_symlink() {
                ftype3::NF3LNK
            } else {
                ftype3::NF3REG
            };

            if is_icloud_stub(&name) {
                if let Some(real_name) = stub_to_real_name(&name) {
                    if real_names.contains(&real_name) {
                        continue;
                    }
                    let ino = self.get_or_alloc_inode(entry.path(), kind);
                    let attr = meta_to_fattr3(ino, &meta, self.uid, self.gid);
                    all.push(DirEntry {
                        fileid: ino,
                        name: nfsstring(real_name.into_bytes()),
                        attr,
                    });
                }
            } else {
                let ino = self.get_or_alloc_inode(entry.path(), kind);
                let attr = meta_to_fattr3(ino, &meta, self.uid, self.gid);
                all.push(DirEntry {
                    fileid: ino,
                    name: nfsstring(name.into_bytes()),
                    attr,
                });
            }
        }

        // Sort for deterministic ordering
        all.sort_by(|a, b| a.name.0.cmp(&b.name.0));

        // Pagination: skip entries until we pass start_after
        let start_idx = if start_after == 0 {
            0
        } else {
            all.iter()
                .position(|e| e.fileid == start_after)
                .map(|i| i + 1)
                .unwrap_or(0)
        };

        let page = &all[start_idx..];
        let end = page.len() <= max_entries;
        let entries: Vec<DirEntry> = page
            .iter()
            .take(max_entries)
            .map(|e| DirEntry {
                fileid: e.fileid,
                name: nfsstring(e.name.0.clone()),
                attr: e.attr,
            })
            .collect();

        Ok(ReadDirResult { entries, end })
    }

    async fn readlink(&self, id: fileid3) -> Result<nfspath3, nfsstat3> {
        let real_path = self.get_path(id)?;
        let target = fs::read_link(&real_path).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        Ok(nfsstring(target.as_os_str().as_encoded_bytes().to_vec()))
    }

    // --- Read-only: all write operations return NFS3ERR_ROFS ---

    async fn setattr(&self, _id: fileid3, _setattr: sattr3) -> Result<fattr3, nfsstat3> {
        Err(nfsstat3::NFS3ERR_ROFS)
    }

    async fn write(&self, _id: fileid3, _offset: u64, _data: &[u8]) -> Result<fattr3, nfsstat3> {
        Err(nfsstat3::NFS3ERR_ROFS)
    }

    async fn create(
        &self,
        _dirid: fileid3,
        _filename: &filename3,
        _attr: sattr3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        Err(nfsstat3::NFS3ERR_ROFS)
    }

    async fn create_exclusive(
        &self,
        _dirid: fileid3,
        _filename: &filename3,
    ) -> Result<fileid3, nfsstat3> {
        Err(nfsstat3::NFS3ERR_ROFS)
    }

    async fn mkdir(
        &self,
        _dirid: fileid3,
        _dirname: &filename3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        Err(nfsstat3::NFS3ERR_ROFS)
    }

    async fn remove(&self, _dirid: fileid3, _filename: &filename3) -> Result<(), nfsstat3> {
        Err(nfsstat3::NFS3ERR_ROFS)
    }

    async fn rename(
        &self,
        _from_dirid: fileid3,
        _from_filename: &filename3,
        _to_dirid: fileid3,
        _to_filename: &filename3,
    ) -> Result<(), nfsstat3> {
        Err(nfsstat3::NFS3ERR_ROFS)
    }

    async fn symlink(
        &self,
        _dirid: fileid3,
        _linkname: &filename3,
        _symlink: &nfspath3,
        _attr: &sattr3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        Err(nfsstat3::NFS3ERR_ROFS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_child_real_file() {
        let dir = std::env::temp_dir().join("icne-nfs-test-resolve-real");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("hello.txt"), b"hi").unwrap();

        let (path, is_stub) = resolve_child(&dir, "hello.txt").unwrap();
        assert_eq!(path, dir.join("hello.txt"));
        assert!(!is_stub);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_child_stub_file() {
        let dir = std::env::temp_dir().join("icne-nfs-test-resolve-stub");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(".Report.pdf.icloud"), b"stub").unwrap();

        let (path, is_stub) = resolve_child(&dir, "Report.pdf").unwrap();
        assert_eq!(path, dir.join(".Report.pdf.icloud"));
        assert!(is_stub);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_child_not_found() {
        let dir = std::env::temp_dir().join("icne-nfs-test-resolve-none");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        assert!(resolve_child(&dir, "nope.txt").is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn meta_to_fattr3_file() {
        let dir = std::env::temp_dir().join("icne-nfs-test-attr");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let f = dir.join("test.txt");
        fs::write(&f, b"hello world").unwrap();

        let meta = fs::metadata(&f).unwrap();
        let attr = meta_to_fattr3(42, &meta, 501, 20);
        assert_eq!(attr.fileid, 42);
        assert_eq!(attr.size, 11);
        assert!(matches!(attr.ftype, ftype3::NF3REG));
        assert_eq!(attr.nlink, 1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn meta_to_fattr3_dir() {
        let dir = std::env::temp_dir().join("icne-nfs-test-attr-dir");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let meta = fs::metadata(&dir).unwrap();
        let attr = meta_to_fattr3(1, &meta, 501, 20);
        assert!(matches!(attr.ftype, ftype3::NF3DIR));
        assert_eq!(attr.nlink, 2);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn icloud_nfs_new_seeds_root() {
        let dir = std::env::temp_dir().join("icne-nfs-test-new");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let nfs = IcloudNfs::new(dir.clone(), "/tmp/nonexistent.sock");

        let inodes = nfs.inodes.read().unwrap();
        assert!(inodes.contains_key(&ROOT_ID));
        assert_eq!(inodes[&ROOT_ID].real_path, dir);
        assert!(matches!(inodes[&ROOT_ID].kind, ftype3::NF3DIR));
        assert_eq!(nfs.next_ino.load(Ordering::Relaxed), 2);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_ops_return_rofs() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let dir = std::env::temp_dir().join("icne-nfs-test-rofs");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let nfs = IcloudNfs::new(dir.clone(), "/tmp/nonexistent.sock");
        let f = nfsstring(b"f".to_vec());
        let d = nfsstring(b"d".to_vec());
        let a = nfsstring(b"a".to_vec());
        let b = nfsstring(b"b".to_vec());

        rt.block_on(async {
            assert!(nfs.setattr(ROOT_ID, sattr3::default()).await.is_err());
            assert!(nfs.write(ROOT_ID, 0, b"x").await.is_err());
            assert!(nfs.create(ROOT_ID, &f, sattr3::default()).await.is_err());
            assert!(nfs.remove(ROOT_ID, &f).await.is_err());
            assert!(nfs.mkdir(ROOT_ID, &d).await.is_err());
            assert!(nfs.rename(ROOT_ID, &a, ROOT_ID, &b).await.is_err());
        });

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn readdir_stub_translation() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let dir = std::env::temp_dir().join("icne-nfs-test-readdir");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("real.txt"), b"real").unwrap();
        fs::write(dir.join(".Report.pdf.icloud"), b"stub").unwrap();

        let nfs = IcloudNfs::new(dir.clone(), "/tmp/nonexistent.sock");

        rt.block_on(async {
            let result = nfs.readdir(ROOT_ID, 0, 100).await.unwrap();
            let names: Vec<String> = result
                .entries
                .iter()
                .map(|e| String::from_utf8(e.name.0.clone()).unwrap())
                .collect();
            assert!(names.contains(&"real.txt".to_string()));
            assert!(names.contains(&"Report.pdf".to_string()));
            // Stub name should NOT appear
            assert!(!names.contains(&".Report.pdf.icloud".to_string()));
        });

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn readdir_deduplication() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let dir = std::env::temp_dir().join("icne-nfs-test-dedup");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        // Both real file and stub exist — should only show once
        fs::write(dir.join("doc.pdf"), b"real").unwrap();
        fs::write(dir.join(".doc.pdf.icloud"), b"stub").unwrap();

        let nfs = IcloudNfs::new(dir.clone(), "/tmp/nonexistent.sock");

        rt.block_on(async {
            let result = nfs.readdir(ROOT_ID, 0, 100).await.unwrap();
            let names: Vec<String> = result
                .entries
                .iter()
                .map(|e| String::from_utf8(e.name.0.clone()).unwrap())
                .collect();
            let count = names.iter().filter(|n| *n == "doc.pdf").count();
            assert_eq!(count, 1, "doc.pdf should appear exactly once");
            assert!(!names.contains(&".doc.pdf.icloud".to_string()));
        });

        let _ = fs::remove_dir_all(&dir);
    }
}
