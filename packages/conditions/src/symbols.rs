//! Static table of the symbols owned by `ncl-conditions`.
//!
//! Every row mirrors `conformance/ownership/symbols.tsv` for crate
//! `ncl-conditions`, phase 1. The rows are split by package across the
//! submodules below; the `symbols` accessor yields them in ownership-table
//! order.

mod common_lisp;
mod ncl_ext;
mod ncl_ffi;
mod ncl_sys;
mod ncl_threads;

pub use ncl_ownership::{SymbolKind, SymbolRow};

pub use common_lisp::COMMON_LISP;
pub use ncl_ext::NCL_EXT;
pub use ncl_ffi::NCL_FFI;
pub use ncl_sys::NCL_SYS;
pub use ncl_threads::NCL_THREADS;

/// Every Phase-1 symbol owned by this crate, in ownership-table order.
pub fn symbols() -> impl Iterator<Item = &'static SymbolRow> {
    COMMON_LISP
        .iter()
        .chain(NCL_EXT)
        .chain(NCL_FFI)
        .chain(NCL_SYS)
        .chain(NCL_THREADS)
}
