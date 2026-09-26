//! ANSI Common Lisp pathname domain and filesystem adapter.
//!
//! Pathnames are parsed as POSIX-shaped namestrings. A leading slash marks an
//! absolute directory, `~` remains literal text, and no filesystem or
//! environment lookup occurs in the domain layer.

mod domain;
mod fs;

pub use domain::{
    Component, Device, Directory, DirectoryElement, DirectoryKind, Host, Name, Pathname,
    PathnameError, Type, Version, make_pathname, merge_pathnames, parse_namestring,
    pathname_match_p, translate_pathname, wild_pathname_p,
};

pub use fs::{
    EnsureDirectoriesResult, FsError, MetadataField, Operation, Pathname as FsPathname,
    delete_file, directory, ensure_directories_exist, file_author, file_length, file_write_date,
    probe_file, rename_file, truename,
};
