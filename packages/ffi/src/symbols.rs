//! Static table of the symbols owned by `ncl-ffi`.
//!
//! The phase-one symbols owned by the `NCL-FFI` package.

mod extension;
mod foreign;
mod system;

pub use ncl_ownership::{SymbolKind, SymbolRow};

pub use extension::EXTENSION;
pub use foreign::FOREIGN;
pub use system::SYSTEM;

/// Every Phase-1 symbol owned by this crate, in ownership-table order.
pub fn symbols() -> impl Iterator<Item = &'static SymbolRow> {
    FOREIGN.iter().chain(EXTENSION).chain(SYSTEM)
}
