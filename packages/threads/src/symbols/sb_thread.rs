//! `SB-THREAD` symbols owned by `ncl-threads`.

use super::{SymbolKind, SymbolRow};

/// The `SB-THREAD` Phase-1 symbols owned by this crate.
pub const SB_THREAD: &[SymbolRow] = &[
    SymbolRow {
        package: "SB-THREAD",
        name: "%DISPOSE-THREAD-STRUCTS",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "%THREAD-LOCAL-REFERENCES",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "*CURRENT-THREAD*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "*INTERRUPT-HANDLER*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "ABORT-THREAD",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "ALLOCATOR-HISTOGRAM",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "AVL-FIND<=",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "AVL-FIND>=",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "AVLTREE-LIST",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "BARRIER",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "CLEAR-SEMAPHORE-NOTIFICATION",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "CONDITION-BROADCAST",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "CONDITION-NOTIFY",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "CONDITION-WAIT",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "CURRENT-THREAD-SAP",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "DESTROY-THREAD",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "FOREIGN-THREAD",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "FUTEX-WAKE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "GET-FOREGROUND",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "GET-MUTEX",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "GET-SPINLOCK",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "GRAB-MUTEX",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "HOLDING-MUTEX-P",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "INTERRUPT-THREAD",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "INTERRUPT-THREAD-ERROR-THREAD",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "JOIN-THREAD",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "JOIN-THREAD-ERROR-THREAD",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "JOIN-THREAD-PROBLEM",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "LIST-ALL-THREADS",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "MAIN-THREAD",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "MAIN-THREAD-P",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "MAKE-LISTENER-THREAD",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "MAKE-MUTEX",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "MAKE-RWLOCK",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "MAKE-SEMAPHORE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "MAKE-SEMAPHORE-NOTIFICATION",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "MAKE-THREAD",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "MAKE-WAITQUEUE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "MUTEX",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "MUTEX-NAME",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "MUTEX-OWNER",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "MUTEX-VALUE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "PRINT-ALLOCATOR-HISTOGRAM",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "RELEASE-FOREGROUND",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "RELEASE-MUTEX",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "RELEASE-SPINLOCK",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "RESET-ALLOCATOR-HISTOGRAM",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "RETURN-FROM-THREAD",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "RWLOCK-RDLOCK",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "RWLOCK-UNLOCK",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "RWLOCK-WRLOCK",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "SEMAPHORE",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "SEMAPHORE-COUNT",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "SEMAPHORE-NAME",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "SEMAPHORE-NOTIFICATION",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "SEMAPHORE-NOTIFICATION-STATUS",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "SIGNAL-SEMAPHORE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "SPINLOCK",
        kind: SymbolKind::Type,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "SYMBOL-VALUE-IN-THREAD",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "TERMINATE-THREAD",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "THREAD",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "THREAD-ALIVE-P",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "THREAD-DEADLOCK-CYCLE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "THREAD-EPHEMERAL-P",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "THREAD-ERROR-THREAD",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "THREAD-NAME",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "THREAD-OS-TID",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "THREAD-YIELD",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "TRY-SEMAPHORE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "WAIT-ON-SEMAPHORE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "WAITQUEUE",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "WAITQUEUE-NAME",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "WITH-MUTEX",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "WITH-NEW-SESSION",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "WITH-RECURSIVE-LOCK",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "WITH-SESSION-LOCK",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "WITH-SPINLOCK",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "WITH-TLS-LOCK",
        kind: SymbolKind::Macro,
    },
];
