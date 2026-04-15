/// Check if a filename is an iCloud stub placeholder.
///
/// Stub format: `.OriginalName.icloud` (e.g. `.Report.pdf.icloud`).
pub fn is_icloud_stub(name: &str) -> bool {
    name.starts_with('.') && name.ends_with(".icloud") && name.len() > 8
}

/// Convert a stub filename to the original name.
///
/// `.Report.pdf.icloud` → `Some("Report.pdf")`
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

/// Convert a real filename to the corresponding stub name.
///
/// `Report.pdf` → `.Report.pdf.icloud`
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
