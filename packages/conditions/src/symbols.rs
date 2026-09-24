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

/// Registration kind of an owned symbol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolKind {
    /// A function, registered with `define_function`.
    Function,
    /// A condition class, registered with `define_class`.
    Class,
    /// A condition class that is also a function.
    ClassAndFunction,
    /// A variable, which only needs to be interned.
    Variable,
    /// A symbol with no dedicated registry, which only needs to be interned.
    Other,
}

/// One owned symbol.
pub struct SymbolRow {
    /// Owning package name.
    pub package: &'static str,
    /// Symbol name.
    pub name: &'static str,
    /// Registration kind.
    pub kind: SymbolKind,
}

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
