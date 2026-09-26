//! Typed filesystem operations used by pathname builtins.

#![forbid(unsafe_code)]

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// An owned pathname accepted by the filesystem adapter.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Pathname(PathBuf);

impl Pathname {
    /// Construct a pathname, rejecting the empty path.
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, FsError> {
        let path = path.into();
        if path.as_os_str().is_empty() {
            return Err(FsError::InvalidPath);
        }
        Ok(Self(path))
    }

    /// Borrow the platform-native path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// Consume the pathname and return its platform-native path.
    #[must_use]
    pub fn into_path(self) -> PathBuf {
        self.0
    }
}

impl AsRef<Path> for Pathname {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

impl fmt::Display for Pathname {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.display().fmt(formatter)
    }
}

/// The filesystem operation that produced an error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operation {
    ProbeFile,
    Truename,
    Directory,
    EnsureDirectoriesExist,
    FileWriteDate,
    FileLength,
    RenameFile,
    DeleteFile,
    FileAuthor,
}

/// A metadata field that the host cannot provide through the standard library.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataField {
    Author,
    WriteDate,
}

/// Domain-facing filesystem failure.
#[derive(Debug)]
pub enum FsError {
    /// The supplied pathname is empty or otherwise invalid for this adapter.
    InvalidPath,
    /// The operation expected a directory but found another filesystem object.
    NotDirectory { operation: Operation },
    /// The operating system rejected an operation.
    Io {
        operation: Operation,
        kind: io::ErrorKind,
    },
    /// A requested metadata field is not available on this host.
    MetadataUnavailable { field: MetadataField },
}

impl fmt::Display for FsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath => formatter.write_str("pathname is empty"),
            Self::NotDirectory { operation } => {
                write!(formatter, "{operation:?} requires a directory")
            }
            Self::Io { operation, kind } => write!(formatter, "{operation:?} failed: {kind}"),
            Self::MetadataUnavailable { field } => {
                write!(formatter, "metadata is unavailable: {field:?}")
            }
        }
    }
}

impl std::error::Error for FsError {}

fn io_error(operation: Operation, error: io::Error) -> FsError {
    if error.kind() == io::ErrorKind::NotADirectory {
        FsError::NotDirectory { operation }
    } else {
        FsError::Io {
            operation,
            kind: error.kind(),
        }
    }
}

/// Return the existing pathname, or `None` when it does not exist.
pub fn probe_file(pathname: &Pathname) -> Result<Option<Pathname>, FsError> {
    match fs::metadata(pathname.as_path()) {
        Ok(_) => truename(pathname).map(Some),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io_error(Operation::ProbeFile, error)),
    }
}

/// Resolve a pathname to an existing canonical pathname.
pub fn truename(pathname: &Pathname) -> Result<Pathname, FsError> {
    fs::canonicalize(pathname.as_path())
        .map(Pathname)
        .map_err(|error| io_error(Operation::Truename, error))
}

/// Return the canonical path of every entry in a directory.
pub fn directory(pathname: &Pathname) -> Result<Vec<Pathname>, FsError> {
    let entries =
        fs::read_dir(pathname.as_path()).map_err(|error| io_error(Operation::Directory, error))?;
    entries
        .map(|entry| {
            let entry = entry.map_err(|error| io_error(Operation::Directory, error))?;
            truename(&Pathname(entry.path())).map_err(|error| match error {
                FsError::Io {
                    kind: io::ErrorKind::NotFound,
                    ..
                } => FsError::Io {
                    operation: Operation::Directory,
                    kind: io::ErrorKind::NotFound,
                },
                other => other,
            })
        })
        .collect()
}

/// The result of ensuring a directory exists.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnsureDirectoriesResult {
    /// The requested directory.
    pub pathname: Pathname,
    /// Whether this call created one or more directories.
    pub created: bool,
}

/// Create a directory and all missing parents.
pub fn ensure_directories_exist(pathname: &Pathname) -> Result<EnsureDirectoriesResult, FsError> {
    match fs::metadata(pathname.as_path()) {
        Ok(metadata) => {
            if metadata.is_dir() {
                return Ok(EnsureDirectoriesResult {
                    pathname: pathname.clone(),
                    created: false,
                });
            }
            Err(FsError::NotDirectory {
                operation: Operation::EnsureDirectoriesExist,
            })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir_all(pathname.as_path())
                .map_err(|error| io_error(Operation::EnsureDirectoriesExist, error))?;
            Ok(EnsureDirectoriesResult {
                pathname: pathname.clone(),
                created: true,
            })
        }
        Err(error) => Err(io_error(Operation::EnsureDirectoriesExist, error)),
    }
}

/// Return the last modification time, when the host supplies one.
pub fn file_write_date(pathname: &Pathname) -> Result<Option<SystemTime>, FsError> {
    let metadata = fs::metadata(pathname.as_path())
        .map_err(|error| io_error(Operation::FileWriteDate, error))?;
    Ok(metadata.modified().ok())
}

/// Return the file size in bytes.
pub fn file_length(pathname: &Pathname) -> Result<u64, FsError> {
    fs::metadata(pathname.as_path())
        .map(|metadata| metadata.len())
        .map_err(|error| io_error(Operation::FileLength, error))
}

/// Rename a filesystem entry and return its destination pathname.
pub fn rename_file(from: &Pathname, to: &Pathname) -> Result<Pathname, FsError> {
    fs::rename(from.as_path(), to.as_path())
        .map_err(|error| io_error(Operation::RenameFile, error))?;
    Ok(to.clone())
}

/// Delete a filesystem entry.
pub fn delete_file(pathname: &Pathname) -> Result<(), FsError> {
    let metadata = fs::symlink_metadata(pathname.as_path())
        .map_err(|error| io_error(Operation::DeleteFile, error))?;
    if metadata.is_dir() {
        fs::remove_dir(pathname.as_path())
    } else {
        fs::remove_file(pathname.as_path())
    }
    .map_err(|error| io_error(Operation::DeleteFile, error))
}

/// Return the file author when the host exposes a portable author name.
pub fn file_author(pathname: &Pathname) -> Result<Option<String>, FsError> {
    let metadata =
        fs::metadata(pathname.as_path()).map_err(|error| io_error(Operation::FileAuthor, error))?;
    // CLHS permits NIL when the host cannot expose an author. Unix metadata
    // exposes the owner uid, but resolving that uid is host policy and may
    // fail in containers or when passwd is unavailable.
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        let uid = metadata.uid().to_string();
        let passwd = fs::read_to_string("/etc/passwd").ok();
        return Ok(passwd.and_then(|contents| {
            contents.lines().find_map(|line| {
                let mut fields = line.split(':');
                let name = fields.next()?;
                let _password = fields.next()?;
                let user_id = fields.next()?;
                (user_id == uid).then(|| name.to_string())
            })
        }));
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        Ok(None)
    }
}

#[cfg(test)]
#[path = "fs/tests.rs"]
mod tests;
