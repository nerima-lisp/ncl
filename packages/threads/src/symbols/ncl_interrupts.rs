//! `NCL-THREADS` symbols owned by `ncl-threads`.

use super::{SymbolKind, SymbolRow};

/// The `NCL-THREADS` Phase-1 symbols owned by this crate.
pub const NCL_INTERRUPTS: &[SymbolRow] = &[
    SymbolRow {
        package: "NCL-THREADS",
        name: "*ALLOW-WITH-INTERRUPTS*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "*INTERRUPT-PENDING*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "*INTERRUPTS-ENABLED*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "*PERIODIC-POLLING-FUNCTION*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "*PERIODIC-POLLING-PERIOD*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "ALLOW-WITH-INTERRUPTS",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "CANCEL-DEADLINE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "DECODE-TIMEOUT",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "DEFER-DEADLINE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "ENABLE-INTERRUPT",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "IN-INTERRUPTION",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "INVOKE-INTERRUPTION",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "SIGNAL-DEADLINE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "WITH-DEADLINE",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "WITH-INTERRUPT-BINDINGS",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "WITH-INTERRUPTS",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "WITH-LOCAL-INTERRUPTS",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "WITHOUT-INTERRUPTS",
        kind: SymbolKind::Macro,
    },
];
