//! Static table of the symbols owned by `ncl-conditions`.
//!
//! Every row mirrors `conformance/ownership/symbols.tsv` for crate
//! `ncl-conditions`, phase 1. The rows are split by package across the
//! submodules below; the `symbols` accessor yields them in ownership-table
//! order.

mod common_lisp;
mod sb_alien;
mod sb_debug;
mod sb_ext;
mod sb_sys;
mod sb_thread;

pub use ncl_ownership::{SymbolKind, SymbolRow};

pub use common_lisp::COMMON_LISP;
pub use sb_alien::SB_ALIEN;
pub use sb_debug::SB_DEBUG;
pub use sb_ext::SB_EXT;
pub use sb_sys::SB_SYS;
pub use sb_thread::SB_THREAD;

/// Every Phase-1 symbol owned by this crate, in ownership-table order.
pub fn symbols() -> impl Iterator<Item = &'static SymbolRow> {
    COMMON_LISP
        .iter()
        .chain(SB_ALIEN)
        .chain(SB_DEBUG)
        .chain(SB_EXT)
        .chain(SB_SYS)
        .chain(SB_THREAD)
}
