//! NCL-SYS condition types owned by ncl-conditions.

use super::{SymbolKind, SymbolRow};

/// Condition types in NCL-SYS.
pub const NCL_SYS: &[SymbolRow] = &[
    SymbolRow {
        package: "NCL-SYS",
        name: "BREAKPOINT-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-SYS",
        name: "DEADLINE-TIMEOUT",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-SYS",
        name: "INTERACTIVE-INTERRUPT",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-SYS",
        name: "IO-TIMEOUT",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-SYS",
        name: "MEMORY-FAULT-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-SYS",
        name: "SYSTEM-CONDITION",
        kind: SymbolKind::Class,
    },
];

