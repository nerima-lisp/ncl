//! Registration of the printer's owned symbols.

use ncl_object::{
    ObjectError, Package, Runtime, ThreadContext, Word, set_symbol_special, set_symbol_value,
};

/// The `COMMON-LISP` functions `ncl-printer` owns.
const COMMON_LISP_FUNCTIONS: [&str; 18] = [
    "COPY-PPRINT-DISPATCH",
    "PPRINT",
    "PPRINT-DISPATCH",
    "PPRINT-FILL",
    "PPRINT-INDENT",
    "PPRINT-LINEAR",
    "PPRINT-NEWLINE",
    "PPRINT-TAB",
    "PPRINT-TABULAR",
    "PRIN1",
    "PRIN1-TO-STRING",
    "PRINC",
    "PRINC-TO-STRING",
    "PRINT",
    "PRINT-NOT-READABLE-OBJECT",
    "PRINT-OBJECT",
    "SET-PPRINT-DISPATCH",
    "WRITE-TO-STRING",
];

/// The `SB-EXT` functions `ncl-printer` owns.
const SB_EXT_FUNCTIONS: [&str; 2] = ["PRINT-SYMBOL-WITH-PREFIX", "PRINT-UNREADABLY"];

/// The `(package, name)` special variables `ncl-printer` owns.
const VARIABLES: [(&str, &str); 4] = [
    ("COMMON-LISP", "*PRINT-PPRINT-DISPATCH*"),
    ("COMMON-LISP", "*PRINT-READABLY*"),
    ("SB-EXT", "*PRINT-CIRCLE-NOT-SHARED*"),
    ("SB-EXT", "*PRINT-VECTOR-LENGTH*"),
];

/// Register every symbol `ncl-printer` owns with `runtime`.
///
/// Functions are registered with an unbound placeholder until the runtime
/// supplies a callable function object; the owned variables are interned,
/// marked special, and initialised to `NIL`.
///
/// # Errors
///
/// Returns an [`ObjectError`] when a package cannot be created, an allocation
/// fails, or a symbol flag cannot be written.
pub fn register(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    for package in ["COMMON-LISP", "SB-EXT"] {
        runtime.ensure_package(ctx, package)?;
    }
    for name in COMMON_LISP_FUNCTIONS {
        runtime.define_function(ctx, "COMMON-LISP", name, Word::UNBOUND)?;
    }
    for name in SB_EXT_FUNCTIONS {
        runtime.define_function(ctx, "SB-EXT", name, Word::UNBOUND)?;
    }
    for (package, name) in VARIABLES {
        let package = runtime.ensure_package(ctx, package)?;
        let (symbol, _status) = Package::from(package).intern(ctx, runtime, name)?;
        set_symbol_special(ctx, symbol, true)?;
        set_symbol_value(ctx, symbol, Word::NIL)?;
    }
    Ok(())
}
