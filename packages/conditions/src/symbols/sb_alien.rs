//! `SB-ALIEN` symbols owned by `ncl-conditions`.

use super::{SymbolKind, SymbolRow};

/// The `SB-ALIEN` Phase-1 symbols owned by this crate.
pub const SB_ALIEN: &[SymbolRow] = &[SymbolRow {
    package: "SB-ALIEN",
    name: "UNDEFINED-ALIEN-ERROR",
    kind: SymbolKind::Class,
}];
