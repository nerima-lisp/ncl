//! ANSI Common Lisp pathname domain and filesystem adapter.
#![allow(missing_docs)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value,
    clippy::needless_return,
    clippy::option_if_let_else,
    clippy::question_mark,
    clippy::redundant_closure_for_method_calls,
    clippy::type_complexity,
    clippy::unnecessary_wraps
)]
//!
//! Pathnames are parsed as POSIX-shaped namestrings. A leading slash marks an
//! absolute directory, `~` remains literal text, and no filesystem or
//! environment lookup occurs in the domain layer.

mod builtins;
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

pub use builtins::register;
