//! `NCL-FFI` symbols owned by `ncl-ffi`.

use super::{SymbolKind, SymbolRow};

/// The `NCL-FFI` Phase-1 symbols owned by this crate.
pub const EXTENSION: &[SymbolRow] = &[
    SymbolRow {
        package: "NCL-FFI",
        name: "POSIX-ENVIRON",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "POSIX-GETENV",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "RUN-PROGRAM",
        kind: SymbolKind::Function,
    },
];
