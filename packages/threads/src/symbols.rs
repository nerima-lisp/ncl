//! Static table of the symbols owned by `ncl-threads`.
//!
//! Every row mirrors the crate-local ownership table for `ncl-threads`. The
//! phase-one rows are split by package across the submodules below; the `rows`
//! accessor yields them in ownership-table order.

mod sb_ext;
mod sb_sys;
mod sb_thread;

pub use ncl_ownership::{SymbolKind, SymbolRow};

/// The `SB-EXT` Phase-1 symbols owned by this crate.
pub use sb_ext::SB_EXT;
/// The `SB-SYS` Phase-1 symbols owned by this crate.
pub use sb_sys::SB_SYS;
/// The `SB-THREAD` Phase-1 symbols owned by this crate.
pub use sb_thread::SB_THREAD;

/// Every Phase-1 symbol owned by this crate, in ownership-table order.
pub fn rows() -> impl Iterator<Item = &'static SymbolRow> {
    SB_EXT.iter().chain(SB_SYS).chain(SB_THREAD)
}
