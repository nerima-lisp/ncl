//! `NCL-THREADS` symbols owned by `ncl-threads`.

use super::{SymbolKind, SymbolRow};

/// The `NCL-THREADS` Phase-1 symbols owned by this crate.
pub const NCL_TIMING: &[SymbolRow] = &[
    SymbolRow {
        package: "NCL-THREADS",
        name: "*TIMEOUT-EXIT*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "ATOMIC-DECF",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "ATOMIC-INCF",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "ATOMIC-POP",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "ATOMIC-PUSH",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "ATOMIC-UPDATE",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "CALL-WITH-TIMING",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "CAS",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "COMPARE-AND-SWAP",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "GET-TIME-OF-DAY",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "LIST-TIMERS",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "TIMER-MAKE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "PROCESS-ALIVE-P",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "PROCESS-CLOSE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "PROCESS-CORE-DUMPED",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "PROCESS-ERROR",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "PROCESS-EXIT-CODE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "PROCESS-INPUT",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "PROCESS-KILL",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "PROCESS-OUTPUT",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "PROCESS-P",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "PROCESS-PID",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "PROCESS-PLIST",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "PROCESS-PTY",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "PROCESS-STATUS",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "PROCESS-STATUS-HOOK",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "PROCESS-WAIT",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "TIMER-SCHEDULE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "TIMER",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "TIMER-NAME",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "TIMER-SCHEDULED-P",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "TIMER-UNSCHEDULE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "WAIT-FOR",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "WITH-TIMEOUT",
        kind: SymbolKind::Macro,
    },
];
