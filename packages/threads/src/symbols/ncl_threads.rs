//! `NCL-THREADS` symbols owned by `ncl-threads`.

use super::{SymbolKind, SymbolRow};

/// The `NCL-THREADS` Phase-1 symbols owned by this crate.
pub const NCL_THREADS: &[SymbolRow] = &[
    SymbolRow {
        package: "NCL-THREADS",
        name: "%DISPOSE-THREAD-STRUCTS",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "%THREAD-LOCAL-REFERENCES",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "*CURRENT-THREAD*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "*INTERRUPT-HANDLER*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "THREAD-ABORT",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "ALLOCATOR-HISTOGRAM",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "AVL-FIND<=",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "AVL-FIND>=",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "AVLTREE-LIST",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "BARRIER",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "SEMAPHORE-NOTIFICATION-CLEAR",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "CONDITION-BROADCAST",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "CONDITION-NOTIFY",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "CONDITION-WAIT",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "CURRENT-THREAD-SAP",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "DESTROY-THREAD",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "FOREIGN-THREAD",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "FUTEX-WAKE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "GET-FOREGROUND",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "MUTEX-LOCK",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "GET-SPINLOCK",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "MUTEX-TRY-LOCK",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "HOLDING-MUTEX-P",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "INTERRUPT-THREAD",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "INTERRUPT-THREAD-ERROR-THREAD",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "THREAD-JOIN",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "THREAD-JOIN-ERROR-THREAD",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "THREAD-JOIN-PROBLEM",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "LIST-THREADS",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "MAIN-THREAD",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "MAIN-THREAD-P",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "LISTENER-THREAD-SPAWN",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "MUTEX-MAKE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "RWLOCK-MAKE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "SEMAPHORE-MAKE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "SEMAPHORE-MAKE-NOTIFICATION",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "THREAD-SPAWN",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "WAITQUEUE-MAKE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "MUTEX",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "MUTEX-NAME",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "MUTEX-OWNER",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "MUTEX-VALUE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "PRINT-ALLOCATOR-HISTOGRAM",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "RELEASE-FOREGROUND",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "MUTEX-UNLOCK",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "RELEASE-SPINLOCK",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "RESET-ALLOCATOR-HISTOGRAM",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "RETURN-FROM-THREAD",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "RWLOCK-RDLOCK",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "RWLOCK-UNLOCK",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "RWLOCK-WRLOCK",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "SEMAPHORE",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "SEMAPHORE-COUNT",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "SEMAPHORE-NAME",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "SEMAPHORE-NOTIFICATION",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "SEMAPHORE-NOTIFICATION-STATUS",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "SEMAPHORE-POST",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "SPINLOCK",
        kind: SymbolKind::Type,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "SYMBOL-VALUE-IN-THREAD",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "THREAD-TERMINATE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "THREAD",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "THREAD-LIVE-P",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "THREAD-DEADLOCK-CYCLE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "THREAD-EPHEMERAL-P",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "THREAD-ERROR-THREAD",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "THREAD-NAME",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "THREAD-NATIVE-ID",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "THREAD-YIELD",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "SEMAPHORE-TRY-WAIT",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "SEMAPHORE-WAIT",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "WAITQUEUE",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "WAITQUEUE-NAME",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "WITH-LOCK",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "WITH-NEW-SESSION",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "WITH-RECURSIVE-LOCK",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "WITH-SESSION-LOCK",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "WITH-SPINLOCK",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-THREADS",
        name: "WITH-TLS-LOCK",
        kind: SymbolKind::Macro,
    },
];
