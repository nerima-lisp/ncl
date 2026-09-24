//! `SB-ALIEN` symbols owned by `ncl-ffi`.

use super::{SymbolKind, SymbolRow};

/// The `SB-ALIEN` Phase-1 symbols owned by this crate.
pub const SB_ALIEN: &[SymbolRow] = &[
    SymbolRow {
        package: "SB-ALIEN",
        name: "*",
        kind: SymbolKind::VariableAndFunction,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "ADDR",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "ALIEN",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "ALIEN-CALLABLE",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "ALIEN-CALLABLE-FUNCTION",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "ALIEN-FUNCALL",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "ALIEN-SAP",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "ALIEN-SIZE",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "ARRAY",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "BOOLEAN",
        kind: SymbolKind::Type,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "C-STRING",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "CAST",
        kind: SymbolKind::MacroAndClass,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "CHAR",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "DEFINE-ALIEN-CALLABLE",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "DEFINE-ALIEN-ROUTINE",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "DEFINE-ALIEN-TYPE",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "DEFINE-ALIEN-VARIABLE",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "DEREF",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "DOUBLE",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "DOUBLE-FLOAT",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "ENUM",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "EXTERN-ALIEN",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "FLOAT",
        kind: SymbolKind::ClassAndFunction,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "FREE-ALIEN",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "FUNCTION",
        kind: SymbolKind::SpecialOperatorAndClass,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "GET-ERRNO",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "INT",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "INTEGER",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "LOAD-1-FOREIGN",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "LOAD-FOREIGN",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "LOAD-SHARED-OBJECT",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "LONG",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "LONG-FLOAT",
        kind: SymbolKind::Type,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "LONG-LONG",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "MAKE-ALIEN",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "MAKE-ALIEN-STRING",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "NULL-ALIEN",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "OFF-T",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "SAP-ALIEN",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "SET-ERRNO",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "SHORT",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "SIGNED",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "SINGLE-FLOAT",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "SIZE-T",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "SLOT",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "SSIZE-T",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "STRUCT",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "SYSTEM-AREA-POINTER",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "UNION",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "UNLOAD-SHARED-OBJECT",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "UNSIGNED",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "UNSIGNED-CHAR",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "UNSIGNED-INT",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "UNSIGNED-LONG",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "UNSIGNED-LONG-LONG",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "UNSIGNED-SHORT",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "UTF8-STRING",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "VALUES",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "VOID",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "WITH-ALIEN",
        kind: SymbolKind::Macro,
    },
    SymbolRow {
        package: "SB-ALIEN",
        name: "WITH-ALIEN-CALLABLE",
        kind: SymbolKind::Macro,
    },
];
