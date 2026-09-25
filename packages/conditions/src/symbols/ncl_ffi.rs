//! NCL-FFI condition types owned by ncl-conditions.

use super::{SymbolKind, SymbolRow};

/// Condition types in NCL-FFI.
pub const NCL_FFI: &[SymbolRow] = &[
    SymbolRow {
        package: "NCL-FFI",
        name: "UNDEFINED-ALIEN-ERROR",
        kind: SymbolKind::Class,
    },
];

