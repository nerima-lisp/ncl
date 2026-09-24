//! Static table of the symbols owned by `ncl-threads`.
//!
//! Every row mirrors `conformance/ownership/symbols.tsv` for crate
//! `ncl-threads`, phase 1. The rows are split by package across the
//! submodules below; the `rows` accessor yields them in ownership-table
//! order.

mod sb_ext;
mod sb_sys;
mod sb_thread;

/// Registration kind of an owned symbol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolKind {
    /// A function, registered with `define_function`.
    Function,
    /// A macro, registered with `define_macro`.
    Macro,
    /// A variable, interned with its special bit set.
    Variable,
    /// A constant.
    Constant,
    /// A class.
    Class,
    /// A type.
    Type,
    /// A symbol with no dedicated registry, which only needs to be interned.
    Other,
}

/// One owned symbol.
#[derive(Debug)]
pub struct SymbolRow {
    /// Owning package name.
    pub package: &'static str,
    /// Symbol name.
    pub name: &'static str,
    /// Registration kind.
    pub kind: SymbolKind,
}

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
