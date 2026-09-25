//! Static table of the symbols owned by `ncl-ffi`.
//!
//! Every row mirrors the crate-local ownership table for `ncl-ffi`, phase 1.
//! The rows are split by package across the submodules below; the
//! `symbols` accessor yields them in ownership-table order.

mod sb_alien;
mod sb_ext;
mod sb_sys;

pub use ncl_ownership::{SymbolKind, SymbolRow};

pub use sb_alien::SB_ALIEN;
pub use sb_ext::SB_EXT;
pub use sb_sys::SB_SYS;

/// Every Phase-1 symbol owned by this crate, in ownership-table order.
pub fn symbols() -> impl Iterator<Item = &'static SymbolRow> {
    SB_ALIEN.iter().chain(SB_EXT).chain(SB_SYS)
}
