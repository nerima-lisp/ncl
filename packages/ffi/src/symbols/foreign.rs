//! `NCL-FFI` symbols owned by `ncl-ffi`.

use super::{SymbolKind, SymbolRow};

/// The `NCL-FFI` Phase-1 symbols owned by this crate.
pub const FOREIGN: &[SymbolRow] = &[
    SymbolRow {
        package: "NCL-FFI",
        name: "*",
        kind: SymbolKind::VariableAndFunction,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "ADDR",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "FOREIGN",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "CALLBACK-TYPE",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "CALLBACK-FUNCTION",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "CALL-FOREIGN",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "FOREIGN-POINTER",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "FOREIGN-TYPE-SIZE",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "ARRAY",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "BOOLEAN",
        kind: SymbolKind::Type,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "C-STRING",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "CAST",
        kind: SymbolKind::MacroAndClass,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "CHAR",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "DEFINE-CALLBACK-TYPE",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "DEFINE-FOREIGN-ROUTINE",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "DEFINE-FOREIGN-TYPE",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "DEFINE-FOREIGN-VARIABLE",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "DEREF",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "DOUBLE",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "DOUBLE-FLOAT",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "ENUM",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "EXTERN-FOREIGN",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "FLOAT",
        kind: SymbolKind::ClassAndFunction,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "FREE-FOREIGN",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "FUNCTION",
        kind: SymbolKind::SpecialOperatorAndClass,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "GET-ERRNO",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "INT",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "INTEGER",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "LOAD-FOREIGN-ONCE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "LOAD-FOREIGN",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "LOAD-SHARED-OBJECT",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "LONG",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "LONG-FLOAT",
        kind: SymbolKind::Type,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "LONG-LONG",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "MAKE-FOREIGN",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "MAKE-FOREIGN-STRING",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "NULL-FOREIGN",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "OFF-T",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "POINTER-FOREIGN",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SET-ERRNO",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SHORT",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SIGNED",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SINGLE-FLOAT",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SIZE-T",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SLOT",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SSIZE-T",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "STRUCT",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "POINTER-TYPE",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "UNION",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "UNLOAD-SHARED-OBJECT",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "UNSIGNED",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "UNSIGNED-CHAR",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "UNSIGNED-INT",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "UNSIGNED-LONG",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "UNSIGNED-LONG-LONG",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "UNSIGNED-SHORT",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "UTF8-STRING",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "VALUES",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "VOID",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "WITH-FOREIGN",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "WITH-CALLBACK-TYPE",
        kind: SymbolKind::Macro,
    },
];
