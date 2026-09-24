//! Static table of the symbols owned by `ncl-conditions`.
//!
//! Every row mirrors `conformance/ownership/symbols.tsv` for crate
//! `ncl-conditions`, phase 1.

/// Registration kind of an owned symbol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolKind {
    /// A function, registered with `define_function`.
    Function,
    /// A condition class, registered with `define_class`.
    Class,
    /// A condition class that is also a function.
    ClassAndFunction,
    /// A variable, which only needs to be interned.
    Variable,
    /// A symbol with no dedicated registry, which only needs to be interned.
    Other,
}

/// One owned symbol.
pub struct SymbolRow {
    /// Owning package name.
    pub package: &'static str,
    /// Symbol name.
    pub name: &'static str,
    /// Registration kind.
    pub kind: SymbolKind,
}

/// Every Phase-1 symbol owned by this crate.
pub const SYMBOLS: &[SymbolRow] = &[
    // COMMON-LISP
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
    // SB-ALIEN
    SymbolRow {
        package: "SB-ALIEN",
        name: "UNDEFINED-ALIEN-ERROR",
        kind: SymbolKind::Class,
    },
    // SB-DEBUG
    SymbolRow {
        package: "SB-DEBUG",
        name: "*BACKTRACE-FRAME-COUNT*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "*DEBUG-BEGINNER-HELP-P*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "*DEBUG-CONDITION*",
        kind: SymbolKind::Other,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "*DEBUG-HELP-STRING*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "*DEBUG-PRINT-VARIABLE-ALIST*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "*DEBUG-READTABLE*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "*FLUSH-DEBUG-ERRORS*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "*IN-THE-DEBUGGER*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "*MAX-TRACE-INDENTATION*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "*METHOD-FRAME-STYLE*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "*STACK-TOP-HINT*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "*TRACE-ENCAPSULATE-DEFAULT*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "*TRACE-INDENTATION-STEP*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "*TRACE-REPORT-DEFAULT*",
        kind: SymbolKind::Variable,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "ARG",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "BACKTRACE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "BACKTRACE-AS-LIST",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "FRAME-HAS-DEBUG-TAG-P",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "INTERNAL-DEBUG",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "LIST-BACKTRACE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "MAP-BACKTRACE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "PRINT-BACKTRACE",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "UNWIND-TO-FRAME-AND-CALL",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-DEBUG",
        name: "VAR",
        kind: SymbolKind::Function,
    },
    // SB-EXT
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
    // SB-SYS
    SymbolRow {
        package: "SB-SYS",
        name: "BREAKPOINT-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "DEADLINE-TIMEOUT",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "INTERACTIVE-INTERRUPT",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "IO-TIMEOUT",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "MEMORY-FAULT-ERROR",
        kind: SymbolKind::ClassAndFunction,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SYSTEM-CONDITION",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SYSTEM-CONDITION-ADDRESS",
        kind: SymbolKind::Function,
    },
    SymbolRow {
        package: "SB-SYS",
        name: "SYSTEM-CONDITION-CONTEXT",
        kind: SymbolKind::Function,
    },
    // SB-THREAD
    SymbolRow {
        package: "SB-THREAD",
        name: "INTERRUPT-THREAD-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "JOIN-THREAD-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "SYMBOL-VALUE-IN-THREAD-ERROR",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "THREAD-DEADLOCK",
        kind: SymbolKind::Class,
    },
    SymbolRow {
        package: "SB-THREAD",
        name: "THREAD-ERROR",
        kind: SymbolKind::Class,
    },
];
