//! `NCL-FFI` symbols owned by `ncl-ffi`.

use super::{SymbolKind, SymbolRow};

/// The `NCL-FFI` Phase-1 symbols owned by this crate.
pub const SYSTEM: &[SymbolRow] = &[
    SymbolRow {
        package: "NCL-FFI",
        name: "*RUNTIME-DLHANDLE*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "*SHARED-OBJECTS*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "ALLOCATE-SYSTEM-MEMORY",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "DEALLOCATE-SYSTEM-MEMORY",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "DLOPEN-OR-LOSE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "EXTERN-FOREIGN-NAME",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "FIND-DYNAMIC-FOREIGN-SYMBOL-ADDRESS",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "FIND-FOREIGN-SYMBOL-ADDRESS",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "FOREIGN-SYMBOL-ADDRESS",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "FOREIGN-SYMBOL-DATAREF-POINTER",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "FOREIGN-SYMBOL-POINTER",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "INT-POINTER",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "INVALIDATE-DESCRIPTOR",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "MEMMOVE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "POINTER+",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "POINTER-",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "POINTER-FOREIGN-SYMBOL",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "POINTER-INT",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "POINTER-REF-16",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "POINTER-REF-32",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "POINTER-REF-64",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "POINTER-REF-8",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "POINTER-REF-DOUBLE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "POINTER-REF-LISPOBJ",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "POINTER-REF-LONG",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "POINTER-REF-SAP",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "POINTER-REF-SINGLE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "POINTER-REF-WORD",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "POINTER<",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "POINTER<=",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "POINTER=",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "POINTER>",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "POINTER>=",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SIGNED-POINTER-REF-16",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SIGNED-POINTER-REF-32",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SIGNED-POINTER-REF-64",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SIGNED-POINTER-REF-8",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "SIGNED-POINTER-REF-WORD",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "FOREIGN-POINTER-P",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "UPDATE-FOREIGN-LINKAGE-TABLE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "NCL-FFI",
        name: "VECTOR-POINTER",
        kind: SymbolKind::Function,
    },
];
