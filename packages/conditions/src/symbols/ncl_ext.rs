//! NCL-EXT condition types owned by ncl-conditions.

use super::{SymbolKind, SymbolRow};

/// Condition types in NCL-EXT.
pub const NCL_EXT: &[SymbolRow] = &[
    SymbolRow {
        package: "NCL-EXT",
        name: "CODE-DELETION-NOTE",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-EXT",
        name: "COMPILER-NOTE",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-EXT",
        name: "DEFCONSTANT-UNEQL",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-EXT",
        name: "DELETE-FILE-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-EXT",
        name: "DEPRECATION-CONDITION",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-EXT",
        name: "DEPRECATION-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-EXT",
        name: "EARLY-DEPRECATION-WARNING",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-EXT",
        name: "FILE-DOES-NOT-EXIST",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-EXT",
        name: "FILE-EXISTS",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-EXT",
        name: "FINAL-DEPRECATION-WARNING",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-EXT",
        name: "IMPLICIT-GENERIC-FUNCTION-WARNING",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-EXT",
        name: "INVALID-FASL",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-EXT",
        name: "LATE-DEPRECATION-WARNING",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-EXT",
        name: "NAME-CONFLICT",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-EXT",
        name: "PACKAGE-DOES-NOT-EXIST",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-EXT",
        name: "PACKAGE-LOCK-VIOLATION",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-EXT",
        name: "PACKAGE-LOCKED-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-EXT",
        name: "READER-PACKAGE-DOES-NOT-EXIST",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-EXT",
        name: "STEP-CONDITION",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-EXT",
        name: "STEP-FINISHED-CONDITION",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-EXT",
        name: "STEP-FORM-CONDITION",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-EXT",
        name: "STEP-VALUES-CONDITION",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-EXT",
        name: "SYMBOL-PACKAGE-LOCKED-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-EXT",
        name: "TIMEOUT",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "NCL-EXT",
        name: "UNKNOWN-KEYWORD-ARGUMENT",
        kind: SymbolKind::Class,
    },
];

