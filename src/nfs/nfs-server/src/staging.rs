use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use log::{debug, info, warn};

/// Manages a staging directory that mirrors the iCloud source tree.
/// All NFS writes land here first; a background task promotes completed
/// files to the real iCloud folder via atomic copy+rename.
#[derive(Clone)]
pub struct StagingLayer {
    /// Root of the staging directory (e.g. ~/.icne-staging/iCloud Drive/)
    staging_root: PathBuf,
    /// Root of the real iCloud source directory
    source_root: PathBuf,
}

impl StagingLayer {
    pub fn new(staging_root: PathBuf, source_root: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&staging_root)?;
        Ok(Self {
            staging_root,
            source_root,
        })
    }

    /// Map an absolute iCloud path to its staging mirror path.
    pub fn staging_path_for(&self, icloud_path: &Path) -> PathBuf {
        let relative = icloud_path
            .strip_prefix(&self.source_root)
            .unwrap_or(icloud_path);
        self.staging_root.join(relative)
    }

    /// Map a staging path back to the corresponding iCloud path.
    pub fn icloud_path_for(&self, staged_path: &Path) -> PathBuf {
        let relative = staged_path
            .strip_prefix(&self.staging_root)
            .unwrap_or(staged_path);
        self.source_root.join(relative)
    }

    /// Check whether a file has a staged copy.
    pub fn is_staged(&self, icloud_path: &Path) -> bool {
        self.staging_path_for(icloud_path).exists()
    }

    /// Copy an existing iCloud file into the staging directory (copy-up).
    /// The iCloud file must already be hydrated (local on disk).
    /// Returns the staging path.
    pub fn copy_up(&self, icloud_path: &Path) -> io::Result<PathBuf> {
        let staged = self.staging_path_for(icloud_path);
        if let Some(parent) = staged.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(icloud_path, &staged)?;
        debug!("copy-up: {} -> {}", icloud_path.display(), staged.display());
        Ok(staged)
    }

    /// Create a new empty file in the staging directory.
    /// Returns the staging path.
    pub fn create_file(&self, icloud_path: &Path) -> io::Result<PathBuf> {
        let staged = self.staging_path_for(icloud_path);
        if let Some(parent) = staged.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::File::create(&staged)?;
        debug!("staged create: {}", staged.display());
        Ok(staged)
    }

    /// Create a directory in both staging and iCloud.
    /// Directories are zero-cost to sync so we create immediately in both.
    pub fn create_dir(&self, icloud_path: &Path) -> io::Result<()> {
        let staged = self.staging_path_for(icloud_path);
        fs::create_dir_all(&staged)?;
        fs::create_dir_all(icloud_path)?;
        debug!("mkdir: {} + {}", staged.display(), icloud_path.display());
        Ok(())
    }

    /// List filenames that exist in the staging directory for a given parent.
    pub fn list_staged_names(&self, icloud_dir: &Path) -> Vec<String> {
        let staged_dir = self.staging_path_for(icloud_dir);
        let Ok(entries) = fs::read_dir(&staged_dir) else {
            return Vec::new();
        };
        entries
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect()
    }

    /// Remove a staged copy (after promotion or delete).
    pub fn remove_staged(&self, icloud_path: &Path) -> io::Result<()> {
        let staged = self.staging_path_for(icloud_path);
        if staged.is_dir() {
            fs::remove_dir_all(&staged)
        } else if staged.exists() {
            fs::remove_file(&staged)
        } else {
            Ok(())
        }
    }

    /// Rename a file within the staging directory.
    /// Returns the new staging path.
    pub fn rename_staged(
        &self,
        old_icloud_path: &Path,
        new_icloud_path: &Path,
    ) -> io::Result<PathBuf> {
        let old_staged = self.staging_path_for(old_icloud_path);
        let new_staged = self.staging_path_for(new_icloud_path);
        if old_staged.exists() {
            if let Some(parent) = new_staged.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::rename(&old_staged, &new_staged)?;
            Ok(new_staged)
        } else {
            Err(io::Error::new(io::ErrorKind::NotFound, "not staged"))
        }
    }

    /// Promote a single file from staging to iCloud via atomic copy+rename.
    fn promote_file(&self, staged_path: &Path) -> io::Result<()> {
        let icloud_path = self.icloud_path_for(staged_path);
        if let Some(parent) = icloud_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Atomic: write to .tmp, then rename over the target
        let file_name = icloud_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        let tmp_path = icloud_path
            .parent()
            .unwrap_or(&icloud_path)
            .join(format!(".{}.icne-tmp", file_name));

        fs::copy(staged_path, &tmp_path)?;
        fs::rename(&tmp_path, &icloud_path)?;
        fs::remove_file(staged_path)?;

        info!(
            "promoted: {} -> {}",
            staged_path.display(),
            icloud_path.display()
        );
        Ok(())
    }

    /// Promote all staged files whose mtime is older than `quiescence`.
    /// Returns paths that were promoted.
    pub fn promote_if_quiesced(&self, quiescence: Duration) -> io::Result<Vec<PathBuf>> {
        let now = SystemTime::now();
        let mut promoted = Vec::new();
        self.walk_and_promote(&self.staging_root, &now, &quiescence, &mut promoted)?;
        Ok(promoted)
    }

    /// Promote all staged files unconditionally (for startup recovery).
    pub fn promote_all(&self) -> io::Result<Vec<PathBuf>> {
        let mut promoted = Vec::new();
        self.walk_and_promote(
            &self.staging_root,
            &SystemTime::now(),
            &Duration::ZERO,
            &mut promoted,
        )?;
        Ok(promoted)
    }

    /// Walk the staging tree and promote eligible files.
    fn walk_and_promote(
        &self,
        dir: &Path,
        now: &SystemTime,
        quiescence: &Duration,
        promoted: &mut Vec<PathBuf>,
    ) -> io::Result<()> {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                self.walk_and_promote(&path, now, quiescence, promoted)?;
                // Clean up empty staging directories
                if fs::read_dir(&path).map(|mut d| d.next().is_none()).unwrap_or(false) {
                    let _ = fs::remove_dir(&path);
                }
            } else {
                let meta = fs::metadata(&path)?;
                let mtime = meta.modified().unwrap_or(*now);
                let age = now.duration_since(mtime).unwrap_or(Duration::ZERO);
                if age >= *quiescence {
                    match self.promote_file(&path) {
                        Ok(()) => promoted.push(self.icloud_path_for(&path)),
                        Err(e) => warn!("promote failed for {}: {}", path.display(), e),
                    }
                }
            }
        }
        Ok(())
    }

    /// Scan the staging directory and return all staged file paths
    /// (as iCloud paths). Used for startup recovery to populate the inode table.
    pub fn scan_staged_files(&self) -> io::Result<Vec<(PathBuf, bool)>> {
        let mut results = Vec::new();
        self.walk_staged(&self.staging_root, &mut results)?;
        Ok(results)
    }

    fn walk_staged(&self, dir: &Path, results: &mut Vec<(PathBuf, bool)>) -> io::Result<()> {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let is_dir = path.is_dir();
            let icloud_path = self.icloud_path_for(&path);
            results.push((icloud_path, is_dir));
            if is_dir {
                self.walk_staged(&path, results)?;
            }
        }
        Ok(())
    }

    pub fn staging_root(&self) -> &Path {
        &self.staging_root
    }

    pub fn source_root(&self) -> &Path {
        &self.source_root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dirs(name: &str) -> (PathBuf, PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("icne-staging-test-{}", name));
        let _ = fs::remove_dir_all(&base);
        let source = base.join("source");
        let staging = base.join("staging");
        fs::create_dir_all(&source).unwrap();
        (base, source, staging)
    }

    #[test]
    fn staging_path_mapping() {
        let (_base, source, staging) = test_dirs("mapping");
        let layer = StagingLayer::new(staging.clone(), source.clone()).unwrap();

        let icloud = source.join("Documents/file.txt");
        let expected = staging.join("Documents/file.txt");
        assert_eq!(layer.staging_path_for(&icloud), expected);
        assert_eq!(layer.icloud_path_for(&expected), icloud);

        let _ = fs::remove_dir_all(&_base);
    }

    #[test]
    fn copy_up_creates_staged_copy() {
        let (_base, source, staging) = test_dirs("copyup");
        fs::create_dir_all(source.join("docs")).unwrap();
        fs::write(source.join("docs/hello.txt"), b"hello world").unwrap();

        let layer = StagingLayer::new(staging.clone(), source.clone()).unwrap();

        assert!(!layer.is_staged(&source.join("docs/hello.txt")));
        let staged = layer.copy_up(&source.join("docs/hello.txt")).unwrap();
        assert!(layer.is_staged(&source.join("docs/hello.txt")));
        assert_eq!(fs::read_to_string(&staged).unwrap(), "hello world");

        let _ = fs::remove_dir_all(&_base);
    }

    #[test]
    fn create_file_in_staging() {
        let (_base, source, staging) = test_dirs("create");
        let layer = StagingLayer::new(staging.clone(), source.clone()).unwrap();

        let icloud_path = source.join("new-file.txt");
        let staged = layer.create_file(&icloud_path).unwrap();
        assert!(staged.exists());
        assert_eq!(fs::read_to_string(&staged).unwrap(), "");

        let _ = fs::remove_dir_all(&_base);
    }

    #[test]
    fn create_dir_in_both() {
        let (_base, source, staging) = test_dirs("mkdir");
        let layer = StagingLayer::new(staging.clone(), source.clone()).unwrap();

        let icloud_path = source.join("new-dir");
        layer.create_dir(&icloud_path).unwrap();
        assert!(icloud_path.is_dir());
        assert!(staging.join("new-dir").is_dir());

        let _ = fs::remove_dir_all(&_base);
    }

    #[test]
    fn list_staged_names() {
        let (_base, source, staging) = test_dirs("list");
        let layer = StagingLayer::new(staging.clone(), source.clone()).unwrap();

        let dir = source.join("docs");
        layer.create_file(&dir.join("a.txt")).unwrap();
        layer.create_file(&dir.join("b.txt")).unwrap();

        let mut names = layer.list_staged_names(&dir);
        names.sort();
        assert_eq!(names, vec!["a.txt", "b.txt"]);

        let _ = fs::remove_dir_all(&_base);
    }

    #[test]
    fn promote_file_atomic() {
        let (_base, source, staging) = test_dirs("promote");
        fs::create_dir_all(&source).unwrap();
        let layer = StagingLayer::new(staging.clone(), source.clone()).unwrap();

        let icloud_path = source.join("upload.txt");
        let staged = layer.create_file(&icloud_path).unwrap();
        fs::write(&staged, b"uploaded content").unwrap();

        let promoted = layer.promote_all().unwrap();
        assert_eq!(promoted.len(), 1);
        assert_eq!(promoted[0], icloud_path);
        assert_eq!(fs::read_to_string(&icloud_path).unwrap(), "uploaded content");
        assert!(!staged.exists()); // staging copy removed

        let _ = fs::remove_dir_all(&_base);
    }

    #[test]
    fn promote_respects_quiescence() {
        let (_base, source, staging) = test_dirs("quiescence");
        let layer = StagingLayer::new(staging.clone(), source.clone()).unwrap();

        let icloud_path = source.join("recent.txt");
        let staged = layer.create_file(&icloud_path).unwrap();
        fs::write(&staged, b"just written").unwrap();

        // File was just written — 60s quiescence should skip it
        let promoted = layer
            .promote_if_quiesced(Duration::from_secs(60))
            .unwrap();
        assert!(promoted.is_empty());
        assert!(staged.exists()); // still in staging

        // Zero quiescence promotes immediately
        let promoted = layer
            .promote_if_quiesced(Duration::ZERO)
            .unwrap();
        assert_eq!(promoted.len(), 1);

        let _ = fs::remove_dir_all(&_base);
    }

    #[test]
    fn remove_staged_file() {
        let (_base, source, staging) = test_dirs("remove");
        let layer = StagingLayer::new(staging.clone(), source.clone()).unwrap();

        let icloud_path = source.join("deleteme.txt");
        layer.create_file(&icloud_path).unwrap();
        assert!(layer.is_staged(&icloud_path));

        layer.remove_staged(&icloud_path).unwrap();
        assert!(!layer.is_staged(&icloud_path));

        let _ = fs::remove_dir_all(&_base);
    }

    #[test]
    fn rename_staged_file() {
        let (_base, source, staging) = test_dirs("rename");
        let layer = StagingLayer::new(staging.clone(), source.clone()).unwrap();

        let old = source.join("old.txt");
        let new = source.join("new.txt");
        let staged = layer.create_file(&old).unwrap();
        fs::write(&staged, b"content").unwrap();

        let new_staged = layer.rename_staged(&old, &new).unwrap();
        assert!(!layer.is_staged(&old));
        assert!(layer.is_staged(&new));
        assert_eq!(fs::read_to_string(&new_staged).unwrap(), "content");

        let _ = fs::remove_dir_all(&_base);
    }

    #[test]
    fn scan_staged_files() {
        let (_base, source, staging) = test_dirs("scan");
        let layer = StagingLayer::new(staging.clone(), source.clone()).unwrap();

        layer.create_file(&source.join("a.txt")).unwrap();
        layer.create_dir(&source.join("subdir")).unwrap();
        layer.create_file(&source.join("subdir/b.txt")).unwrap();

        let mut files = layer.scan_staged_files().unwrap();
        files.sort_by(|a, b| a.0.cmp(&b.0));

        // Should contain: a.txt (file), subdir (dir), subdir/b.txt (file)
        assert_eq!(files.len(), 3);

        let _ = fs::remove_dir_all(&_base);
    }
}
