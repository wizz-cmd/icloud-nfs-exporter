//! Utilities for detecting and converting iCloud stub filenames.
//!
//! When iCloud Drive evicts a file to free local storage, it replaces the
//! original file with a small placeholder whose name follows the pattern
//! `.OriginalName.icloud`. These functions detect that pattern and convert
//! between stub names and real names.
//!
//! # Examples
//!
//! ```rust
//! use fuse_core::path_utils::{is_icloud_stub, stub_to_real_name, real_to_stub_name};
//!
//! // Detect stubs
//! assert!(is_icloud_stub(".Report.pdf.icloud"));
//! assert!(!is_icloud_stub("Report.pdf"));
//!
//! // Convert between formats
//! assert_eq!(stub_to_real_name(".Report.pdf.icloud"), Some("Report.pdf".into()));
//! assert_eq!(real_to_stub_name("Report.pdf"), ".Report.pdf.icloud");
//! ```

/// Check if a filename is an iCloud stub placeholder.
///
/// iCloud stubs follow the pattern `.OriginalName.icloud` -- they start with
/// a dot, end with `.icloud`, and contain at least one character of real name
/// in between (total length > 8).
///
/// This function operates on the filename component only, not a full path.
///
/// # Examples
///
/// ```rust
/// use fuse_core::path_utils::is_icloud_stub;
///
/// assert!(is_icloud_stub(".Report.pdf.icloud"));
/// assert!(is_icloud_stub(".a.icloud"));
/// assert!(is_icloud_stub(".archive.tar.gz.icloud"));
///
/// // Not stubs
/// assert!(!is_icloud_stub("Report.pdf"));
/// assert!(!is_icloud_stub(".icloud"));       // no real name
/// assert!(!is_icloud_stub("file.icloud"));   // no leading dot
/// assert!(!is_icloud_stub(".DS_Store"));
/// ```
pub fn is_icloud_stub(name: &str) -> bool {
    name.starts_with('.') && name.ends_with(".icloud") && name.len() > 8
}

/// Convert a stub filename to the original filename.
///
/// Strips the leading `.` and trailing `.icloud` to recover the original name.
/// Returns `None` if the input is not a valid stub (as determined by
/// [`is_icloud_stub`]).
///
/// # Examples
///
/// ```rust
/// use fuse_core::path_utils::stub_to_real_name;
///
/// assert_eq!(stub_to_real_name(".Report.pdf.icloud"), Some("Report.pdf".into()));
/// assert_eq!(stub_to_real_name(".archive.tar.gz.icloud"), Some("archive.tar.gz".into()));
/// assert_eq!(stub_to_real_name(".a.icloud"), Some("a".into()));
///
/// // Non-stubs return None
/// assert_eq!(stub_to_real_name("Report.pdf"), None);
/// assert_eq!(stub_to_real_name(""), None);
/// ```
pub fn stub_to_real_name(stub_name: &str) -> Option<String> {
    if !is_icloud_stub(stub_name) {
        return None;
    }
    // Strip leading '.' and trailing '.icloud'
    let inner = &stub_name[1..stub_name.len() - 7];
    if inner.is_empty() {
        None
    } else {
        Some(inner.to_string())
    }
}

/// Convert a real filename to the corresponding iCloud stub name.
///
/// Prepends `.` and appends `.icloud` to produce the stub filename that
/// iCloud Drive would create when evicting this file.
///
/// # Examples
///
/// ```rust
/// use fuse_core::path_utils::real_to_stub_name;
///
/// assert_eq!(real_to_stub_name("Report.pdf"), ".Report.pdf.icloud");
/// assert_eq!(real_to_stub_name("a"), ".a.icloud");
/// assert_eq!(real_to_stub_name("archive.tar.gz"), ".archive.tar.gz.icloud");
/// ```
pub fn real_to_stub_name(real_name: &str) -> String {
    format!(".{real_name}.icloud")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_stub() {
        assert!(is_icloud_stub(".Report.pdf.icloud"));
        assert!(is_icloud_stub(".a.icloud"));
    }

    #[test]
    fn reject_non_stubs() {
        assert!(!is_icloud_stub("Report.pdf"));
        assert!(!is_icloud_stub(".icloud")); // just ".icloud" — no real name
        assert!(!is_icloud_stub("..icloud")); // single dot name — too short
        assert!(!is_icloud_stub(".hidden"));
        assert!(!is_icloud_stub(""));
        assert!(!is_icloud_stub(".DS_Store"));
        assert!(!is_icloud_stub("file.icloud")); // no leading dot
    }

    #[test]
    fn stub_to_real() {
        assert_eq!(
            stub_to_real_name(".Report.pdf.icloud"),
            Some("Report.pdf".to_string())
        );
        assert_eq!(
            stub_to_real_name(".Photo.heic.icloud"),
            Some("Photo.heic".to_string())
        );
        assert_eq!(
            stub_to_real_name(".a.icloud"),
            Some("a".to_string())
        );
    }

    #[test]
    fn stub_to_real_non_stub() {
        assert_eq!(stub_to_real_name("Report.pdf"), None);
        assert_eq!(stub_to_real_name(".icloud"), None);
        assert_eq!(stub_to_real_name(""), None);
    }

    #[test]
    fn real_to_stub() {
        assert_eq!(real_to_stub_name("Report.pdf"), ".Report.pdf.icloud");
        assert_eq!(real_to_stub_name("a"), ".a.icloud");
    }

    #[test]
    fn round_trip() {
        let names = ["Document.pdf", "Photo.heic", "data.tar.gz", "a"];
        for name in names {
            let stub = real_to_stub_name(name);
            let recovered = stub_to_real_name(&stub).unwrap();
            assert_eq!(recovered, name);
        }
    }

    #[test]
    fn names_with_dots() {
        assert_eq!(
            stub_to_real_name(".archive.tar.gz.icloud"),
            Some("archive.tar.gz".to_string())
        );
    }

    #[test]
    fn unicode_names() {
        let stub = real_to_stub_name("日本語.txt");
        assert_eq!(stub, ".日本語.txt.icloud");
        assert!(is_icloud_stub(&stub));
        assert_eq!(stub_to_real_name(&stub), Some("日本語.txt".to_string()));
    }
}
