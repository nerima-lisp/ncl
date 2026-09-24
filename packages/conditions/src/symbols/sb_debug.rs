//! `SB-DEBUG` symbols owned by `ncl-conditions`.

use super::{SymbolKind, SymbolRow};

/// The `SB-DEBUG` Phase-1 symbols owned by this crate.
pub const SB_DEBUG: &[SymbolRow] = &[
    SymbolRow {
        package: "SB-DEBUG",
        name: "*BACKTRACE-FRAME-COUNT*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "*DEBUG-BEGINNER-HELP-P*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "*DEBUG-CONDITION*",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "*DEBUG-HELP-STRING*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "*DEBUG-PRINT-VARIABLE-ALIST*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "*DEBUG-READTABLE*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "*FLUSH-DEBUG-ERRORS*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "*IN-THE-DEBUGGER*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "*MAX-TRACE-INDENTATION*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "*METHOD-FRAME-STYLE*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "*STACK-TOP-HINT*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "*TRACE-ENCAPSULATE-DEFAULT*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "*TRACE-INDENTATION-STEP*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "*TRACE-REPORT-DEFAULT*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "ARG",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "BACKTRACE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "BACKTRACE-AS-LIST",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "FRAME-HAS-DEBUG-TAG-P",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "INTERNAL-DEBUG",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "LIST-BACKTRACE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "MAP-BACKTRACE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "PRINT-BACKTRACE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "UNWIND-TO-FRAME-AND-CALL",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "VAR",
        kind: SymbolKind::Function,
    },
];
