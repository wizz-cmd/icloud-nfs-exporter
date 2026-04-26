use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::os::unix::fs::{FileExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::SystemTime;

use async_trait::async_trait;
use log::{debug, info, warn};
use nfsserve::nfs::*;
use nfsserve::vfs::{DirEntry, NFSFileSystem, ReadDirResult, VFSCapabilities};

use fuse_core::IpcClient;
use fuse_core::path_utils::{is_icloud_stub, real_to_stub_name, stub_to_real_name};

use crate::staging::StagingLayer;

const ROOT_ID: fileid3 = 1;

struct InodeData {
    /// Path in the iCloud source tree (may be a .icloud stub path before hydration).
    real_path: PathBuf,
    /// If set, the file has been copied to staging for writing.
    staged_path: Option<PathBuf>,
    #[allow(dead_code)]
    kind: ftype3,
}

pub struct IcloudNfs {
    source: PathBuf,
    socket_path: String,
    staging: StagingLayer,
    inodes: RwLock<HashMap<fileid3, InodeData>>,
    next_ino: AtomicU64,
    uid: u32,
    gid: u32,
}

impl IcloudNfs {
    pub fn new(source: PathBuf, socket_path: &str, staging: StagingLayer) -> Self {
        let uid = unsafe { libc::getuid() };
        let gid = unsafe { libc::getgid() };
        let mut inodes = HashMap::new();
        inodes.insert(
            ROOT_ID,
            InodeData {
                real_path: source.clone(),
                staged_path: None,
                kind: ftype3::NF3DIR,
            },
        );

        let nfs = Self {
            source,
            socket_path: socket_path.to_string(),
            staging,
            inodes: RwLock::new(inodes),
            next_ino: AtomicU64::new(2),
            uid,
            gid,
        };

        // Populate inodes for any files left in staging from a previous session
        if let Ok(staged_files) = nfs.staging.scan_staged_files() {
            for (icloud_path, is_dir) in staged_files {
                let kind = if is_dir {
                    ftype3::NF3DIR
                } else {
                    ftype3::NF3REG
                };
                let staged_path = nfs.staging.staging_path_for(&icloud_path);
                let ino = nfs.next_ino.fetch_add(1, Ordering::Relaxed);
                nfs.inodes.write().unwrap().insert(
                    ino,
                    InodeData {
                        real_path: icloud_path,
                        staged_path: Some(staged_path),
                        kind,
                    },
                );
            }
        }

        nfs
    }

    /// Get the iCloud path (real_path) for an inode.
    fn get_path(&self, id: fileid3) -> Result<PathBuf, nfsstat3> {
        let inodes = self.inodes.read().unwrap();
        inodes
            .get(&id)
            .map(|d| d.real_path.clone())
            .ok_or(nfsstat3::NFS3ERR_STALE)
    }

    /// Get the effective I/O path: staged copy if present, else real_path.
    fn get_effective_path(&self, id: fileid3) -> Result<PathBuf, nfsstat3> {
        let inodes = self.inodes.read().unwrap();
        let data = inodes.get(&id).ok_or(nfsstat3::NFS3ERR_STALE)?;
        Ok(data
            .staged_path
            .clone()
            .unwrap_or_else(|| data.real_path.clone()))
    }

    fn is_staged(&self, id: fileid3) -> bool {
        let inodes = self.inodes.read().unwrap();
        inodes
            .get(&id)
            .map(|d| d.staged_path.is_some())
            .unwrap_or(false)
    }

    fn set_staged_path(&self, id: fileid3, staged: PathBuf) {
        let mut inodes = self.inodes.write().unwrap();
        if let Some(data) = inodes.get_mut(&id) {
            data.staged_path = Some(staged);
        }
    }

    fn get_or_alloc_inode(&self, real_path: PathBuf, kind: ftype3) -> fileid3 {
        let mut inodes = self.inodes.write().unwrap();
        if let Some((ino, _)) = inodes.iter().find(|(_, d)| d.real_path == real_path) {
            return *ino;
        }
        let ino = self.next_ino.fetch_add(1, Ordering::Relaxed);
        inodes.insert(
            ino,
            InodeData {
                real_path,
                staged_path: None,
                kind,
            },
        );
        ino
    }

    fn remove_inode(&self, id: fileid3) {
        let mut inodes = self.inodes.write().unwrap();
        inodes.remove(&id);
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

    /// Ensure a file has a staged copy for writing.
    /// If already staged, returns the staging path.
    /// If not, hydrates (if needed), copies to staging, sets staged_path.
    async fn ensure_staged(&self, id: fileid3) -> Result<PathBuf, nfsstat3> {
        // Already staged?
        if let Some(path) = {
            let inodes = self.inodes.read().unwrap();
            inodes.get(&id).and_then(|d| d.staged_path.clone())
        } {
            return Ok(path);
        }

        // Hydrate first (no-op if already local)
        let hydrated = self.ensure_hydrated(id).await?;

        // Copy-up to staging
        let staged = self
            .staging
            .copy_up(&hydrated)
            .map_err(|e| {
                warn!("copy-up failed: {}", e);
                nfsstat3::NFS3ERR_IO
            })?;

        self.set_staged_path(id, staged.clone());
        Ok(staged)
    }

    pub fn staging(&self) -> &StagingLayer {
        &self.staging
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
        size: meta.len(),
        used: meta.blocks() * 512,
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

fn meta_to_kind(meta: &std::fs::Metadata) -> ftype3 {
    if meta.is_dir() {
        ftype3::NF3DIR
    } else if meta.is_symlink() {
        ftype3::NF3LNK
    } else {
        ftype3::NF3REG
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

/// Apply sattr3 fields to a file path.
fn apply_sattr(path: &Path, attr: &sattr3) -> Result<(), nfsstat3> {
    if let set_mode3::mode(mode) = attr.mode {
        fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
            .map_err(|_| nfsstat3::NFS3ERR_IO)?;
    }
    if let set_size3::size(size) = attr.size {
        let file = OpenOptions::new()
            .write(true)
            .open(path)
            .map_err(|_| nfsstat3::NFS3ERR_IO)?;
        file.set_len(size).map_err(|_| nfsstat3::NFS3ERR_IO)?;
    }
    // uid, gid, atime, mtime: skip for now — iCloud doesn't preserve custom ownership
    Ok(())
}

#[async_trait]
impl NFSFileSystem for IcloudNfs {
    fn root_dir(&self) -> fileid3 {
        ROOT_ID
    }

    fn capabilities(&self) -> VFSCapabilities {
        VFSCapabilities::ReadWrite
    }

    async fn lookup(&self, dirid: fileid3, filename: &filename3) -> Result<fileid3, nfsstat3> {
        let name = std::str::from_utf8(filename).map_err(|_| nfsstat3::NFS3ERR_NOENT)?;
        let parent_path = self.get_path(dirid)?;

        // Canonical iCloud path for this child
        let icloud_path = parent_path.join(name);

        // Try resolving in iCloud directory (handles stubs)
        let icloud_result = resolve_child(&parent_path, name);

        // Check staging
        let staged = self.staging.is_staged(&icloud_path);

        if let Some((real_path, _is_stub)) = icloud_result {
            // File exists in iCloud (possibly as stub)
            let meta_path = if staged {
                self.staging.staging_path_for(&icloud_path)
            } else {
                real_path.clone()
            };
            let meta = fs::symlink_metadata(&meta_path).map_err(|_| nfsstat3::NFS3ERR_NOENT)?;
            let kind = meta_to_kind(&meta);
            let ino = self.get_or_alloc_inode(real_path, kind);
            if staged {
                self.set_staged_path(ino, self.staging.staging_path_for(&icloud_path));
            }
            Ok(ino)
        } else if staged {
            // File only exists in staging (new file, not yet promoted)
            let staged_path = self.staging.staging_path_for(&icloud_path);
            let meta = fs::symlink_metadata(&staged_path).map_err(|_| nfsstat3::NFS3ERR_NOENT)?;
            let kind = meta_to_kind(&meta);
            let ino = self.get_or_alloc_inode(icloud_path, kind);
            self.set_staged_path(ino, staged_path);
            Ok(ino)
        } else {
            Err(nfsstat3::NFS3ERR_NOENT)
        }
    }

    async fn getattr(&self, id: fileid3) -> Result<fattr3, nfsstat3> {
        let path = self.get_effective_path(id)?;
        let meta = fs::symlink_metadata(&path).map_err(|_| nfsstat3::NFS3ERR_STALE)?;
        Ok(meta_to_fattr3(id, &meta, self.uid, self.gid))
    }

    async fn read(
        &self,
        id: fileid3,
        offset: u64,
        count: u32,
    ) -> Result<(Vec<u8>, bool), nfsstat3> {
        // If staged, read from staging; otherwise hydrate and read from iCloud
        let open_path = if self.is_staged(id) {
            self.get_effective_path(id)?
        } else {
            self.ensure_hydrated(id).await?
        };

        let file = File::open(&open_path).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let file_len = file.metadata().map(|m| m.len()).unwrap_or(0);

        let mut buf = vec![0u8; count as usize];
        let bytes_read = file
            .read_at(&mut buf, offset)
            .map_err(|_| nfsstat3::NFS3ERR_IO)?;
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

        // Phase 1: Collect iCloud entries (existing logic with stub translation)
        let mut entry_map: HashMap<String, DirEntry> = HashMap::new();

        if let Ok(entries_iter) = fs::read_dir(&dir_path) {
            let dir_entries: Vec<_> = entries_iter.filter_map(|e| e.ok()).collect();

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

            for entry in &dir_entries {
                let os_name = entry.file_name();
                let name = os_name.to_string_lossy().to_string();

                let meta = match fs::symlink_metadata(entry.path()) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                let kind = meta_to_kind(&meta);

                if is_icloud_stub(&name) {
                    if let Some(real_name) = stub_to_real_name(&name) {
                        if real_names.contains(&real_name) {
                            continue;
                        }
                        let ino = self.get_or_alloc_inode(entry.path(), kind);
                        let attr = meta_to_fattr3(ino, &meta, self.uid, self.gid);
                        entry_map.insert(
                            real_name.clone(),
                            DirEntry {
                                fileid: ino,
                                name: nfsstring(real_name.into_bytes()),
                                attr,
                            },
                        );
                    }
                } else {
                    let ino = self.get_or_alloc_inode(entry.path(), kind);
                    let attr = meta_to_fattr3(ino, &meta, self.uid, self.gid);
                    entry_map.insert(
                        name.clone(),
                        DirEntry {
                            fileid: ino,
                            name: nfsstring(name.into_bytes()),
                            attr,
                        },
                    );
                }
            }
        }

        // Phase 2: Overlay staging entries (staging shadows iCloud entries)
        for staged_name in self.staging.list_staged_names(&dir_path) {
            let icloud_path = dir_path.join(&staged_name);
            let staged_path = self.staging.staging_path_for(&icloud_path);
            let meta = match fs::symlink_metadata(&staged_path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let kind = meta_to_kind(&meta);
            let ino = self.get_or_alloc_inode(icloud_path, kind);
            self.set_staged_path(ino, staged_path);
            let attr = meta_to_fattr3(ino, &meta, self.uid, self.gid);
            entry_map.insert(
                staged_name.clone(),
                DirEntry {
                    fileid: ino,
                    name: nfsstring(staged_name.into_bytes()),
                    attr,
                },
            );
        }

        // Phase 3: Sort and paginate
        let mut all: Vec<DirEntry> = entry_map.into_values().collect();
        all.sort_by(|a, b| a.name.0.cmp(&b.name.0));

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
        let real_path = self.get_effective_path(id)?;
        let target = fs::read_link(&real_path).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        Ok(nfsstring(target.as_os_str().as_encoded_bytes().to_vec()))
    }

    // --- Write operations ---

    async fn setattr(&self, id: fileid3, setattr: sattr3) -> Result<fattr3, nfsstat3> {
        // Truncate requires copy-up to staging
        if matches!(setattr.size, set_size3::size(_)) {
            let staged = self.ensure_staged(id).await?;
            apply_sattr(&staged, &setattr)?;
            let meta = fs::symlink_metadata(&staged).map_err(|_| nfsstat3::NFS3ERR_IO)?;
            return Ok(meta_to_fattr3(id, &meta, self.uid, self.gid));
        }

        // Metadata-only changes apply to effective path
        let path = self.get_effective_path(id)?;
        apply_sattr(&path, &setattr)?;
        let meta = fs::symlink_metadata(&path).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        Ok(meta_to_fattr3(id, &meta, self.uid, self.gid))
    }

    async fn write(&self, id: fileid3, offset: u64, data: &[u8]) -> Result<fattr3, nfsstat3> {
        let staged = self.ensure_staged(id).await?;

        let file = OpenOptions::new()
            .write(true)
            .open(&staged)
            .map_err(|_| nfsstat3::NFS3ERR_IO)?;

        // Extend file if writing past current end
        let current_len = file.metadata().map(|m| m.len()).unwrap_or(0);
        if offset + data.len() as u64 > current_len {
            file.set_len(offset + data.len() as u64)
                .map_err(|_| nfsstat3::NFS3ERR_IO)?;
        }

        file.write_at(data, offset)
            .map_err(|_| nfsstat3::NFS3ERR_IO)?;

        let meta = file.metadata().map_err(|_| nfsstat3::NFS3ERR_IO)?;
        Ok(meta_to_fattr3(id, &meta, self.uid, self.gid))
    }

    async fn create(
        &self,
        dirid: fileid3,
        filename: &filename3,
        attr: sattr3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        let name = std::str::from_utf8(filename).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let parent_path = self.get_path(dirid)?;
        let icloud_path = parent_path.join(name);

        let staged = self
            .staging
            .create_file(&icloud_path)
            .map_err(|_| nfsstat3::NFS3ERR_IO)?;

        apply_sattr(&staged, &attr)?;

        let ino = self.get_or_alloc_inode(icloud_path, ftype3::NF3REG);
        self.set_staged_path(ino, staged.clone());

        let meta = fs::symlink_metadata(&staged).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        info!("create: {} (inode {})", name, ino);
        Ok((ino, meta_to_fattr3(ino, &meta, self.uid, self.gid)))
    }

    async fn create_exclusive(
        &self,
        dirid: fileid3,
        filename: &filename3,
    ) -> Result<fileid3, nfsstat3> {
        let name = std::str::from_utf8(filename).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let parent_path = self.get_path(dirid)?;
        let icloud_path = parent_path.join(name);

        // Must not exist in staging or iCloud
        if self.staging.is_staged(&icloud_path) || resolve_child(&parent_path, name).is_some() {
            return Err(nfsstat3::NFS3ERR_EXIST);
        }

        let staged = self
            .staging
            .create_file(&icloud_path)
            .map_err(|_| nfsstat3::NFS3ERR_IO)?;

        let ino = self.get_or_alloc_inode(icloud_path, ftype3::NF3REG);
        self.set_staged_path(ino, staged);

        info!("create_exclusive: {} (inode {})", name, ino);
        Ok(ino)
    }

    async fn mkdir(
        &self,
        dirid: fileid3,
        dirname: &filename3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        let name = std::str::from_utf8(dirname).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let parent_path = self.get_path(dirid)?;
        let icloud_path = parent_path.join(name);

        // Create in both staging and iCloud
        self.staging
            .create_dir(&icloud_path)
            .map_err(|_| nfsstat3::NFS3ERR_IO)?;

        let ino = self.get_or_alloc_inode(icloud_path.clone(), ftype3::NF3DIR);
        let meta = fs::symlink_metadata(&icloud_path).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        info!("mkdir: {} (inode {})", name, ino);
        Ok((ino, meta_to_fattr3(ino, &meta, self.uid, self.gid)))
    }

    async fn remove(&self, dirid: fileid3, filename: &filename3) -> Result<(), nfsstat3> {
        let name = std::str::from_utf8(filename).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let parent_path = self.get_path(dirid)?;
        let icloud_path = parent_path.join(name);

        // Remove staged copy if present
        let _ = self.staging.remove_staged(&icloud_path);

        // Remove from iCloud (real file or stub)
        if let Some((real_path, _)) = resolve_child(&parent_path, name) {
            if real_path.is_dir() {
                return Err(nfsstat3::NFS3ERR_ISDIR);
            }
            fs::remove_file(&real_path).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        }

        // Remove inode
        let ino = {
            let inodes = self.inodes.read().unwrap();
            inodes
                .iter()
                .find(|(_, d)| {
                    d.real_path == icloud_path
                        || d.real_path == parent_path.join(real_to_stub_name(name))
                })
                .map(|(ino, _)| *ino)
        };
        if let Some(ino) = ino {
            self.remove_inode(ino);
        }

        info!("remove: {}", name);
        Ok(())
    }

    async fn rename(
        &self,
        from_dirid: fileid3,
        from_filename: &filename3,
        to_dirid: fileid3,
        to_filename: &filename3,
    ) -> Result<(), nfsstat3> {
        let from_name =
            std::str::from_utf8(from_filename).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let to_name =
            std::str::from_utf8(to_filename).map_err(|_| nfsstat3::NFS3ERR_IO)?;

        let from_parent = self.get_path(from_dirid)?;
        let to_parent = self.get_path(to_dirid)?;
        let from_icloud = from_parent.join(from_name);
        let to_icloud = to_parent.join(to_name);

        let is_staged = self.staging.is_staged(&from_icloud);

        if is_staged {
            // Rename in staging
            self.staging
                .rename_staged(&from_icloud, &to_icloud)
                .map_err(|_| nfsstat3::NFS3ERR_IO)?;
        }

        // Rename in iCloud if the source exists there
        if let Some((real_from, _)) = resolve_child(&from_parent, from_name) {
            let real_to = to_parent.join(to_name);
            fs::rename(&real_from, &real_to).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        }

        // Update inode table
        {
            let mut inodes = self.inodes.write().unwrap();
            // Find the source inode by its real_path or stub path
            let ino = inodes
                .iter()
                .find(|(_, d)| {
                    d.real_path == from_icloud
                        || d.real_path == from_parent.join(real_to_stub_name(from_name))
                })
                .map(|(ino, _)| *ino);

            if let Some(ino) = ino {
                if let Some(data) = inodes.get_mut(&ino) {
                    data.real_path = to_icloud.clone();
                    if is_staged {
                        data.staged_path =
                            Some(self.staging.staging_path_for(&to_icloud));
                    }
                }
            }
        }

        info!("rename: {} -> {}", from_name, to_name);
        Ok(())
    }

    async fn symlink(
        &self,
        _dirid: fileid3,
        _linkname: &filename3,
        _symlink: &nfspath3,
        _attr: &sattr3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        // iCloud does not reliably preserve symlinks
        Err(nfsstat3::NFS3ERR_NOTSUPP)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create test directories and a StagingLayer
    fn setup(name: &str) -> (PathBuf, PathBuf, StagingLayer) {
        let base = std::env::temp_dir().join(format!("icne-nfs-test-{}", name));
        let _ = fs::remove_dir_all(&base);
        let source = base.join("source");
        let staging_root = base.join("staging");
        fs::create_dir_all(&source).unwrap();
        let staging = StagingLayer::new(staging_root, source.clone()).unwrap();
        (base, source, staging)
    }

    fn make_nfs(source: PathBuf, staging: StagingLayer) -> IcloudNfs {
        IcloudNfs::new(source, "/tmp/nonexistent.sock", staging)
    }

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
        let (base, source, staging) = setup("new");

        let nfs = make_nfs(source.clone(), staging);

        let inodes = nfs.inodes.read().unwrap();
        assert!(inodes.contains_key(&ROOT_ID));
        assert_eq!(inodes[&ROOT_ID].real_path, source);
        assert!(matches!(inodes[&ROOT_ID].kind, ftype3::NF3DIR));
        assert!(inodes[&ROOT_ID].staged_path.is_none());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn capabilities_read_write() {
        let (_base, source, staging) = setup("caps");
        let nfs = make_nfs(source, staging);
        assert!(matches!(nfs.capabilities(), VFSCapabilities::ReadWrite));
        let _ = fs::remove_dir_all(&_base);
    }

    #[test]
    fn write_to_new_file() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (base, source, staging) = setup("write-new");
        let nfs = make_nfs(source, staging);

        rt.block_on(async {
            // Create a file
            let filename = nfsstring(b"test.txt".to_vec());
            let (ino, _attr) = nfs.create(ROOT_ID, &filename, sattr3::default()).await.unwrap();

            // Write to it
            let attr = nfs.write(ino, 0, b"hello world").await.unwrap();
            assert_eq!(attr.size, 11);

            // Read it back
            let (data, eof) = nfs.read(ino, 0, 1024).await.unwrap();
            assert_eq!(data, b"hello world");
            assert!(eof);
        });

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn write_at_offset() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (base, source, staging) = setup("write-offset");
        let nfs = make_nfs(source, staging);

        rt.block_on(async {
            let filename = nfsstring(b"offset.txt".to_vec());
            let (ino, _) = nfs.create(ROOT_ID, &filename, sattr3::default()).await.unwrap();

            nfs.write(ino, 0, b"AAAA").await.unwrap();
            nfs.write(ino, 4, b"BBBB").await.unwrap();

            let (data, _) = nfs.read(ino, 0, 1024).await.unwrap();
            assert_eq!(data, b"AAAABBBB");
        });

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn create_and_lookup() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (base, source, staging) = setup("create-lookup");
        let nfs = make_nfs(source, staging);

        rt.block_on(async {
            let filename = nfsstring(b"newfile.txt".to_vec());
            let (ino, _) = nfs.create(ROOT_ID, &filename, sattr3::default()).await.unwrap();

            // Lookup should find the staged file
            let found = nfs.lookup(ROOT_ID, &filename).await.unwrap();
            assert_eq!(found, ino);
        });

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn create_exclusive_fails_if_exists() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (base, source, staging) = setup("create-excl");
        let nfs = make_nfs(source, staging);

        rt.block_on(async {
            let filename = nfsstring(b"excl.txt".to_vec());
            nfs.create(ROOT_ID, &filename, sattr3::default()).await.unwrap();

            // Second create_exclusive should fail
            let result = nfs.create_exclusive(ROOT_ID, &filename).await;
            assert!(matches!(result, Err(nfsstat3::NFS3ERR_EXIST)));
        });

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn mkdir_creates_in_both() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (base, source, _staging) = setup("mkdir");
        let staging_root = base.join("staging");
        let nfs = make_nfs(source.clone(), StagingLayer::new(staging_root.clone(), source.clone()).unwrap());

        rt.block_on(async {
            let dirname = nfsstring(b"newdir".to_vec());
            let (ino, attr) = nfs.mkdir(ROOT_ID, &dirname).await.unwrap();
            assert!(matches!(attr.ftype, ftype3::NF3DIR));

            // Should exist in both iCloud and staging
            assert!(source.join("newdir").is_dir());
            assert!(staging_root.join("newdir").is_dir());

            // Lookup should find it
            let found = nfs.lookup(ROOT_ID, &dirname).await.unwrap();
            assert_eq!(found, ino);
        });

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn remove_file() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (base, source, staging) = setup("remove");
        // Create a real file in iCloud dir
        fs::write(source.join("delete-me.txt"), b"gone").unwrap();
        let nfs = make_nfs(source.clone(), staging);

        rt.block_on(async {
            let filename = nfsstring(b"delete-me.txt".to_vec());
            // Ensure it exists
            nfs.lookup(ROOT_ID, &filename).await.unwrap();
            // Remove it
            nfs.remove(ROOT_ID, &filename).await.unwrap();

            assert!(!source.join("delete-me.txt").exists());
            assert!(nfs.lookup(ROOT_ID, &filename).await.is_err());
        });

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn remove_staged_file() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (base, source, staging) = setup("remove-staged");
        let nfs = make_nfs(source, staging);

        rt.block_on(async {
            let filename = nfsstring(b"staged.txt".to_vec());
            nfs.create(ROOT_ID, &filename, sattr3::default()).await.unwrap();
            nfs.write(
                nfs.lookup(ROOT_ID, &filename).await.unwrap(),
                0,
                b"data",
            )
            .await
            .unwrap();

            nfs.remove(ROOT_ID, &filename).await.unwrap();
            assert!(nfs.lookup(ROOT_ID, &filename).await.is_err());
        });

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn rename_file() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (base, source, staging) = setup("rename");
        fs::write(source.join("old.txt"), b"content").unwrap();
        let nfs = make_nfs(source, staging);

        rt.block_on(async {
            let old = nfsstring(b"old.txt".to_vec());
            let new = nfsstring(b"new.txt".to_vec());

            nfs.lookup(ROOT_ID, &old).await.unwrap();
            nfs.rename(ROOT_ID, &old, ROOT_ID, &new).await.unwrap();

            assert!(nfs.lookup(ROOT_ID, &old).await.is_err());
            nfs.lookup(ROOT_ID, &new).await.unwrap();
        });

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn setattr_truncate() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (base, source, staging) = setup("setattr");
        fs::write(source.join("big.txt"), b"hello world 12345").unwrap();
        let nfs = make_nfs(source, staging);

        rt.block_on(async {
            let filename = nfsstring(b"big.txt".to_vec());
            let ino = nfs.lookup(ROOT_ID, &filename).await.unwrap();

            let mut attr = sattr3::default();
            attr.size = set_size3::size(5);
            let result = nfs.setattr(ino, attr).await.unwrap();
            assert_eq!(result.size, 5);

            let (data, _) = nfs.read(ino, 0, 1024).await.unwrap();
            assert_eq!(data, b"hello");
        });

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn symlink_not_supported() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (base, source, staging) = setup("symlink");
        let nfs = make_nfs(source, staging);

        rt.block_on(async {
            let result = nfs
                .symlink(
                    ROOT_ID,
                    &nfsstring(b"link".to_vec()),
                    &nfsstring(b"target".to_vec()),
                    &sattr3::default(),
                )
                .await;
            assert!(matches!(result, Err(nfsstat3::NFS3ERR_NOTSUPP)));
        });

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn readdir_shows_staged_files() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (base, source, staging) = setup("readdir-staged");
        fs::write(source.join("icloud.txt"), b"from icloud").unwrap();
        let nfs = make_nfs(source, staging);

        rt.block_on(async {
            // Create a file in staging only
            let filename = nfsstring(b"staged.txt".to_vec());
            nfs.create(ROOT_ID, &filename, sattr3::default()).await.unwrap();

            let result = nfs.readdir(ROOT_ID, 0, 100).await.unwrap();
            let names: Vec<String> = result
                .entries
                .iter()
                .map(|e| String::from_utf8(e.name.0.clone()).unwrap())
                .collect();

            assert!(names.contains(&"icloud.txt".to_string()));
            assert!(names.contains(&"staged.txt".to_string()));
        });

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn readdir_stub_translation() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (base, source, staging) = setup("readdir-stubs");
        fs::write(source.join("real.txt"), b"real").unwrap();
        fs::write(source.join(".Report.pdf.icloud"), b"stub").unwrap();
        let nfs = make_nfs(source, staging);

        rt.block_on(async {
            let result = nfs.readdir(ROOT_ID, 0, 100).await.unwrap();
            let names: Vec<String> = result
                .entries
                .iter()
                .map(|e| String::from_utf8(e.name.0.clone()).unwrap())
                .collect();
            assert!(names.contains(&"real.txt".to_string()));
            assert!(names.contains(&"Report.pdf".to_string()));
            assert!(!names.contains(&".Report.pdf.icloud".to_string()));
        });

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn readdir_deduplication() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (base, source, staging) = setup("readdir-dedup");
        fs::write(source.join("doc.pdf"), b"real").unwrap();
        fs::write(source.join(".doc.pdf.icloud"), b"stub").unwrap();
        let nfs = make_nfs(source, staging);

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

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn readdir_staging_shadows_icloud() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (base, source, staging) = setup("readdir-shadow");
        // Same file in both iCloud (10 bytes) and staging (5 bytes)
        fs::write(source.join("shared.txt"), b"icloud ver").unwrap();
        let nfs = make_nfs(source, staging);

        rt.block_on(async {
            // Create + write a shorter version in staging
            let filename = nfsstring(b"shared.txt".to_vec());
            let ino = nfs.lookup(ROOT_ID, &filename).await.unwrap();
            let staged = nfs.ensure_staged(ino).await.unwrap();
            fs::write(&staged, b"staged").unwrap();

            let result = nfs.readdir(ROOT_ID, 0, 100).await.unwrap();
            let entry = result
                .entries
                .iter()
                .find(|e| e.name.0 == b"shared.txt")
                .unwrap();
            // Should show staging size (6), not iCloud size (10)
            assert_eq!(entry.attr.size, 6);
        });

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn startup_recovery_populates_inodes() {
        let base = std::env::temp_dir().join("icne-nfs-test-recovery");
        let _ = fs::remove_dir_all(&base);
        let source = base.join("source");
        let staging_root = base.join("staging");
        fs::create_dir_all(&source).unwrap();

        // Pre-populate staging with a file (simulating crash recovery)
        fs::create_dir_all(staging_root.join("docs")).unwrap();
        fs::write(staging_root.join("docs/recovered.txt"), b"data").unwrap();

        let staging = StagingLayer::new(staging_root, source.clone()).unwrap();
        let nfs = make_nfs(source, staging);

        // The inode table should contain the recovered file
        let inodes = nfs.inodes.read().unwrap();
        let has_recovered = inodes.values().any(|d| {
            d.real_path.ends_with("docs/recovered.txt") && d.staged_path.is_some()
        });
        assert!(has_recovered, "recovered file should be in inode table");

        let _ = fs::remove_dir_all(&base);
    }
}
