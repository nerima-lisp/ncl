//! `SB-THREAD` symbols owned by `ncl-conditions`.

use super::{SymbolKind, SymbolRow};

/// The `SB-THREAD` Phase-1 symbols owned by this crate.
pub const SB_THREAD: &[SymbolRow] = &[
    SymbolRow {
        package: "SB-THREAD",
        name: "INTERRUPT-THREAD-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "JOIN-THREAD-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "SYMBOL-VALUE-IN-THREAD-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "THREAD-DEADLOCK",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "THREAD-ERROR",
        kind: SymbolKind::Class,
    },
];
