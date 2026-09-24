//! `SB-EXT` symbols owned by `ncl-threads`.

use super::{SymbolKind, SymbolRow};

/// The `SB-EXT` Phase-1 symbols owned by this crate.
pub const SB_EXT: &[SymbolRow] = &[
    SymbolRow {
        package: "SB-EXT",
        name: "*EXIT-TIMEOUT*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "ATOMIC-DECF",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "ATOMIC-INCF",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "ATOMIC-POP",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "ATOMIC-PUSH",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "ATOMIC-UPDATE",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "CALL-WITH-TIMING",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "CAS",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "COMPARE-AND-SWAP",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "GET-TIME-OF-DAY",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "LIST-ALL-TIMERS",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "MAKE-TIMER",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "PROCESS-ALIVE-P",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "PROCESS-CLOSE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "PROCESS-CORE-DUMPED",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "PROCESS-ERROR",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "PROCESS-EXIT-CODE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "PROCESS-INPUT",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "PROCESS-KILL",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "PROCESS-OUTPUT",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "PROCESS-P",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "PROCESS-PID",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "PROCESS-PLIST",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "PROCESS-PTY",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "PROCESS-STATUS",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "PROCESS-STATUS-HOOK",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "PROCESS-WAIT",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "SCHEDULE-TIMER",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "TIMER",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "TIMER-NAME",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "TIMER-SCHEDULED-P",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "UNSCHEDULE-TIMER",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "WAIT-FOR",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "WITH-TIMEOUT",
        kind: SymbolKind::Macro,
    },
];
