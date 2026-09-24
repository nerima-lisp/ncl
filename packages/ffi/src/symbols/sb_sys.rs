//! `SB-SYS` symbols owned by `ncl-ffi`.

use super::{SymbolKind, SymbolRow};

/// The `SB-SYS` Phase-1 symbols owned by this crate.
pub const SB_SYS: &[SymbolRow] = &[
    SymbolRow {
        package: "SB-SYS",
        name: "*RUNTIME-DLHANDLE*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "*SHARED-OBJECTS*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "ALLOCATE-SYSTEM-MEMORY",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "DEALLOCATE-SYSTEM-MEMORY",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "DLOPEN-OR-LOSE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "EXTERN-ALIEN-NAME",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "FIND-DYNAMIC-FOREIGN-SYMBOL-ADDRESS",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "FIND-FOREIGN-SYMBOL-ADDRESS",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "FOREIGN-SYMBOL-ADDRESS",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "FOREIGN-SYMBOL-DATAREF-SAP",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "FOREIGN-SYMBOL-SAP",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "INT-SAP",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "INVALIDATE-DESCRIPTOR",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "MEMMOVE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SAP+",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SAP-",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SAP-FOREIGN-SYMBOL",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SAP-INT",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SAP-REF-16",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SAP-REF-32",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SAP-REF-64",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SAP-REF-8",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SAP-REF-DOUBLE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SAP-REF-LISPOBJ",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SAP-REF-LONG",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SAP-REF-SAP",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SAP-REF-SINGLE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SAP-REF-WORD",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SAP<",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SAP<=",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SAP=",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SAP>",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SAP>=",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SIGNED-SAP-REF-16",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SIGNED-SAP-REF-32",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SIGNED-SAP-REF-64",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SIGNED-SAP-REF-8",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SIGNED-SAP-REF-WORD",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SYSTEM-AREA-POINTER",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SYSTEM-AREA-POINTER-P",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "UPDATE-ALIEN-LINKAGE-TABLE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "VECTOR-SAP",
        kind: SymbolKind::Function,
    },
];
