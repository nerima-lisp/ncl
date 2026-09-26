//! `COMMON-LISP` symbols owned by `ncl-conditions`.

use super::{SymbolKind, SymbolRow};

/// The `COMMON-LISP` Phase-1 symbols owned by this crate.
pub const COMMON_LISP: &[SymbolRow] = &[
    SymbolRow {
        package: "COMMON-LISP",
        name: "*BREAK-ON-SIGNALS*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "*DEBUGGER-HOOK*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "ABORT",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "ARITHMETIC-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "ARITHMETIC-ERROR-OPERANDS",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "ARITHMETIC-ERROR-OPERATION",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "BREAK",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "CELL-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "CELL-ERROR-NAME",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "CERROR",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "COMPUTE-RESTARTS",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "CONDITION",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "CONTINUE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "CONTROL-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "DIVISION-BY-ZERO",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "END-OF-FILE",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "ERROR",
        kind: SymbolKind::ClassAndFunction,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "FILE-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "FIND-RESTART",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "FLOATING-POINT-INEXACT",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "FLOATING-POINT-INVALID-OPERATION",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "FLOATING-POINT-OVERFLOW",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "FLOATING-POINT-UNDERFLOW",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "INVALID-METHOD-ERROR",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "INVOKE-DEBUGGER",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "INVOKE-RESTART",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "INVOKE-RESTART-INTERACTIVELY",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "MAKE-CONDITION",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "METHOD-COMBINATION-ERROR",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "MUFFLE-WARNING",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "PACKAGE-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "PARSE-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "PRINT-NOT-READABLE",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "PROGRAM-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "READER-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "RESTART-NAME",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "SERIOUS-CONDITION",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "SIGNAL",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "SIMPLE-CONDITION",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "SIMPLE-CONDITION-FORMAT-ARGUMENTS",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "SIMPLE-CONDITION-FORMAT-CONTROL",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "SIMPLE-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "SIMPLE-TYPE-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "SIMPLE-WARNING",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "STORAGE-CONDITION",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "STORE-VALUE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "STREAM-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "STYLE-WARNING",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "TYPE-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "TYPE-ERROR-DATUM",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "TYPE-ERROR-EXPECTED-TYPE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "UNBOUND-SLOT",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "UNBOUND-SLOT-INSTANCE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "UNBOUND-VARIABLE",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "UNDEFINED-FUNCTION",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "USE-VALUE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "COMMON-LISP",
        name: "WARNING",
        kind: SymbolKind::Class,
    },
];
