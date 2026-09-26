use super::*;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

trait Required<T> {
    fn required(self, message: &str) -> T;
}

impl<T, E> Required<T> for Result<T, E> {
    fn required(self, message: &str) -> T {
        assert!(self.is_ok(), "{message}");
        match self {
            Ok(value) => value,
            Err(_) => std::process::abort(),
        }
    }
}

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("ncl-pathnames-{}-{id}", std::process::id()));
        fs::create_dir(&path).required("unique temp directory");
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
    let root = Pathname::new(root.0.clone()).required("non-empty temp path");
    let nested = Pathname::new(root.as_path().join("nested")).required("nested path");
    assert!(
        ensure_directories_exist(&nested)
            .required("create nested")
            .created
    );

    let source = Pathname::new(nested.as_path().join("source.txt")).required("source path");
    fs::write(source.as_path(), b"hello").required("write source");
    assert_eq!(file_length(&source).required("length"), 5);
    assert!(probe_file(&source).required("probe").is_some());
    assert!(file_write_date(&source).required("write date").is_some());
    if let Some(author) = file_author(&source).required("author") {
        assert!(!author.is_empty());
    }

    let renamed = Pathname::new(nested.as_path().join("renamed.txt")).required("destination path");
    assert_eq!(rename_file(&source, &renamed).required("rename"), renamed);
    let entries = directory(&nested).required("directory");
    assert_eq!(entries.len(), 1);
    assert!(entries[0].as_path().ends_with("renamed.txt"));
    delete_file(&renamed).required("delete");
    assert_eq!(probe_file(&renamed).required("probe missing"), None);
}

#[test]
fn missing_path_and_existing_directory_are_typed() {
    let root = TempDirectory::new();
    let root = Pathname::new(root.0.clone()).required("non-empty temp path");
    let missing = Pathname::new(root.as_path().join("missing")).required("missing path");
    assert_eq!(probe_file(&missing).required("probe missing"), None);
    assert!(
        !ensure_directories_exist(&root)
            .required("existing directory")
            .created
    );
    assert!(matches!(
        Pathname::new(PathBuf::new()),
        Err(FsError::InvalidPath)
    ));
}
