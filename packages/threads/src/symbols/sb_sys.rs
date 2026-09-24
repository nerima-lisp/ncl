//! `SB-SYS` symbols owned by `ncl-threads`.

use super::{SymbolKind, SymbolRow};

/// The `SB-SYS` Phase-1 symbols owned by this crate.
pub const SB_SYS: &[SymbolRow] = &[
    SymbolRow {
        package: "SB-SYS",
        name: "*ALLOW-WITH-INTERRUPTS*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "*INTERRUPT-PENDING*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "*INTERRUPTS-ENABLED*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "*PERIODIC-POLLING-FUNCTION*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "*PERIODIC-POLLING-PERIOD*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "ALLOW-WITH-INTERRUPTS",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "CANCEL-DEADLINE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "DECODE-TIMEOUT",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "DEFER-DEADLINE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "ENABLE-INTERRUPT",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "IN-INTERRUPTION",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "INVOKE-INTERRUPTION",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SIGNAL-DEADLINE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "WITH-DEADLINE",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "WITH-INTERRUPT-BINDINGS",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "WITH-INTERRUPTS",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "WITH-LOCAL-INTERRUPTS",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "WITHOUT-INTERRUPTS",
        kind: SymbolKind::Macro,
    },
];
