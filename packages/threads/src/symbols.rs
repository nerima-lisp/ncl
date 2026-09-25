//! Static table of the symbols owned by `ncl-threads`.
//!
//! Every row mirrors the crate-local ownership table for `ncl-threads`. The
//! phase-one rows are split by package across the submodules below; the `rows`
//! accessor yields them in ownership-table order.

mod ncl_interrupts;
mod ncl_threads;
mod ncl_timing;

pub use ncl_ownership::{SymbolKind, SymbolRow};

/// The `NCL-THREADS` Phase-1 symbols owned by this crate.
pub use ncl_interrupts::NCL_INTERRUPTS;
/// The `NCL-THREADS` Phase-1 symbols owned by this crate.
pub use ncl_threads::NCL_THREADS;
/// The `NCL-THREADS` Phase-1 symbols owned by this crate.
pub use ncl_timing::NCL_TIMING;

/// Every Phase-1 symbol owned by this crate, in ownership-table order.
pub fn rows() -> impl Iterator<Item = &'static SymbolRow> {
    NCL_TIMING
        .iter()
        .chain(NCL_INTERRUPTS)
        .chain(NCL_THREADS)
        .filter(|row| retained(row.name))
}

fn retained(name: &str) -> bool {
    !matches!(
        name,
        "*ALLOW-WITH-INTERRUPTS*"
            | "*INTERRUPT-PENDING*"
            | "*INTERRUPTS-ENABLED*"
            | "*PERIODIC-POLLING-FUNCTION*"
            | "*PERIODIC-POLLING-PERIOD*"
            | "ALLOW-WITH-INTERRUPTS"
            | "CANCEL-DEADLINE"
            | "IN-INTERRUPTION"
            | "INVOKE-INTERRUPTION"
            | "WITH-INTERRUPT-BINDINGS"
            | "WITH-LOCAL-INTERRUPTS"
            | "%DISPOSE-THREAD-STRUCTS"
            | "%THREAD-LOCAL-REFERENCES"
            | "*INTERRUPT-HANDLER*"
            | "THREAD-ABORT"
            | "ALLOCATOR-HISTOGRAM"
            | "AVL-FIND<="
            | "AVL-FIND>="
            | "AVLTREE-LIST"
            | "BARRIER"
            | "CURRENT-THREAD-SAP"
            | "DESTROY-THREAD"
            | "FUTEX-WAKE"
            | "LISTENER-THREAD-SPAWN"
            | "INTERRUPT-THREAD-ERROR-THREAD"
            | "THREAD-JOIN-ERROR-THREAD"
            | "THREAD-JOIN-PROBLEM"
            | "PRINT-ALLOCATOR-HISTOGRAM"
            | "RESET-ALLOCATOR-HISTOGRAM"
            | "RETURN-FROM-THREAD"
            | "SYMBOL-VALUE-IN-THREAD"
            | "THREAD-DEADLOCK-CYCLE"
            | "THREAD-EPHEMERAL-P"
            | "WITH-NEW-SESSION"
            | "WITH-SESSION-LOCK"
            | "WITH-SPINLOCK"
            | "WITH-TLS-LOCK"
            | "ATOMIC-DECF"
            | "ATOMIC-INCF"
            | "ATOMIC-POP"
            | "ATOMIC-PUSH"
            | "ATOMIC-UPDATE"
            | "CAS"
            | "COMPARE-AND-SWAP"
            | "PROCESS-STATUS-HOOK"
            | "WAIT-FOR"
    )
}
