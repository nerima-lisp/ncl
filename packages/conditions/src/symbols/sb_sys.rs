//! `SB-SYS` symbols owned by `ncl-conditions`.

use super::{SymbolKind, SymbolRow};

/// The `SB-SYS` Phase-1 symbols owned by this crate.
pub const SB_SYS: &[SymbolRow] = &[
    SymbolRow {
        package: "SB-SYS",
        name: "BREAKPOINT-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "DEADLINE-TIMEOUT",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "INTERACTIVE-INTERRUPT",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "IO-TIMEOUT",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "MEMORY-FAULT-ERROR",
        kind: SymbolKind::ClassAndFunction,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SYSTEM-CONDITION",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SYSTEM-CONDITION-ADDRESS",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SYSTEM-CONDITION-CONTEXT",
        kind: SymbolKind::Function,
    },
];
