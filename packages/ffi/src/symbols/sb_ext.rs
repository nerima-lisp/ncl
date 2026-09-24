//! `SB-EXT` symbols owned by `ncl-ffi`.

use super::{SymbolKind, SymbolRow};

/// The `SB-EXT` Phase-1 symbols owned by this crate.
pub const SB_EXT: &[SymbolRow] = &[
    SymbolRow {
        package: "SB-EXT",
        name: "POSIX-ENVIRON",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "POSIX-GETENV",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "RUN-PROGRAM",
        kind: SymbolKind::Function,
    },
];
