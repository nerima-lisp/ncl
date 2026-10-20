//! `NCL-FFI` system area pointer symbols owned by `ncl-ffi`.

use super::{SymbolKind, SymbolRow};

/// The `NCL-FFI` system area pointer symbols owned by this crate.
pub const SAP: &[SymbolRow] = &[
    SymbolRow {
        package: "NCL-FFI",
        name: "INT-SAP",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SAP+",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SAP-",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SAP-INT",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SAP-REF-8",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SAP-REF-16",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SAP-REF-32",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SAP-REF-64",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SAP-REF-DOUBLE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SAP-REF-SAP",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SAP-REF-SINGLE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SAP-REF-WORD",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SAP<",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SAP<=",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SAP=",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SAP>",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SAP>=",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SIGNED-SAP-REF-8",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SIGNED-SAP-REF-16",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SIGNED-SAP-REF-32",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SIGNED-SAP-REF-64",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SIGNED-SAP-REF-WORD",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SYSTEM-AREA-POINTER",
        kind: SymbolKind::Class,
    },
];
