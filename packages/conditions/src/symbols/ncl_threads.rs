//! NCL-THREADS condition types owned by ncl-conditions.

use super::{SymbolKind, SymbolRow};

/// Condition types in NCL-THREADS.
pub const NCL_THREADS: &[SymbolRow] = &[
    SymbolRow {
        package: "NCL-THREADS",
        name: "INTERRUPT-THREAD-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "JOIN-THREAD-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "SYMBOL-VALUE-IN-THREAD-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "THREAD-DEADLOCK",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "THREAD-ERROR",
        kind: SymbolKind::Class,
    },
];
