//! `SB-EXT` symbols owned by `ncl-conditions`.

use super::{SymbolKind, SymbolRow};

/// The `SB-EXT` Phase-1 symbols owned by this crate.
pub const SB_EXT: &[SymbolRow] = &[
    SymbolRow {
        package: "SB-EXT",
        name: "CODE-DELETION-NOTE",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "COMPILER-NOTE",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "DEFCONSTANT-UNEQL",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "DELETE-FILE-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "DEPRECATION-CONDITION",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "DEPRECATION-CONDITION-NAME",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "DEPRECATION-CONDITION-NAMESPACE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "DEPRECATION-CONDITION-REPLACEMENTS",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "DEPRECATION-CONDITION-RUNTIME-ERROR",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "DEPRECATION-CONDITION-SOFTWARE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "DEPRECATION-CONDITION-VERSION",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "DEPRECATION-ERROR",
        kind: SymbolKind::ClassAndFunction,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "EARLY-DEPRECATION-WARNING",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "FILE-DOES-NOT-EXIST",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "FILE-EXISTS",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "FINAL-DEPRECATION-WARNING",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "IMPLICIT-GENERIC-FUNCTION-WARNING",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "INHIBIT-WARNINGS",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "INVALID-FASL",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "LATE-DEPRECATION-WARNING",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "MUFFLE-CONDITIONS",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "NAME-CONFLICT",
        kind: SymbolKind::ClassAndFunction,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "PACKAGE-DOES-NOT-EXIST",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "PACKAGE-LOCK-VIOLATION",
        kind: SymbolKind::ClassAndFunction,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "PACKAGE-LOCKED-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "READER-PACKAGE-DOES-NOT-EXIST",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "RETRY",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "STEP-CONDITION",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "STEP-FINISHED-CONDITION",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "STEP-FORM-CONDITION",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "STEP-VALUES-CONDITION",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "SYMBOL-PACKAGE-LOCKED-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "TIMEOUT",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "UNKNOWN-KEYWORD-ARGUMENT",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-EXT",
        name: "UNMUFFLE-CONDITIONS",
        kind: SymbolKind::Other,
    },
];
