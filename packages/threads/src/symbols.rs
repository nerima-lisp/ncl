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
    NCL_TIMING.iter().chain(NCL_INTERRUPTS).chain(NCL_THREADS)
}
