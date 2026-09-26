use super::*;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("ncl-pathnames-{}-{id}", std::process::id()));
        fs::create_dir(&path).expect("unique temp directory");
        Self(path)
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn file_lifecycle_and_directory_listing() {
    let root = TempDirectory::new();
    let root = Pathname::new(root.0.clone()).expect("non-empty temp path");
    let nested = Pathname::new(root.as_path().join("nested")).expect("nested path");
    assert!(
        ensure_directories_exist(&nested)
            .expect("create nested")
            .created
    );

    let source = Pathname::new(nested.as_path().join("source.txt")).expect("source path");
    fs::write(source.as_path(), b"hello").expect("write source");
    assert_eq!(file_length(&source).expect("length"), 5);
    assert!(probe_file(&source).expect("probe").is_some());
    assert!(file_write_date(&source).expect("write date").is_some());
    assert_eq!(file_author(&source).expect("author"), None);

    let renamed = Pathname::new(nested.as_path().join("renamed.txt")).expect("destination path");
    assert_eq!(rename_file(&source, &renamed).expect("rename"), renamed);
    let entries = directory(&nested).expect("directory");
    assert_eq!(entries.len(), 1);
    assert!(entries[0].as_path().ends_with("renamed.txt"));
    delete_file(&renamed).expect("delete");
    assert_eq!(probe_file(&renamed).expect("probe missing"), None);
}

#[test]
fn missing_path_and_existing_directory_are_typed() {
    let root = TempDirectory::new();
    let root = Pathname::new(root.0.clone()).expect("non-empty temp path");
    let missing = Pathname::new(root.as_path().join("missing")).expect("missing path");
    assert_eq!(probe_file(&missing).expect("probe missing"), None);
    assert!(
        !ensure_directories_exist(&root)
            .expect("existing directory")
            .created
    );
    assert!(matches!(
        Pathname::new(PathBuf::new()),
        Err(FsError::InvalidPath)
    ));
}
