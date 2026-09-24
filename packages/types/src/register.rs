//! Registration of the standard type names and type-related functions.

use ncl_object::{ObjectError, Package, Runtime, ThreadContext, Word, set_symbol_constant};

const COMMON_LISP: &str = "COMMON-LISP";
const SB_EXT: &str = "SB-EXT";

/// Owned symbols to intern, as `(package, name)` pairs.
const SYMBOLS: &[(&str, &str)] = &[
    (COMMON_LISP, "ARRAY"),
    (COMMON_LISP, "BASE-CHAR"),
    (COMMON_LISP, "BASE-STRING"),
    (COMMON_LISP, "BIGNUM"),
    (COMMON_LISP, "BIT-VECTOR"),
    (COMMON_LISP, "BOOLEAN"),
    (COMMON_LISP, "BROADCAST-STREAM"),
    (COMMON_LISP, "COERCE"),
    (COMMON_LISP, "COMPILED-FUNCTION"),
    (COMMON_LISP, "CONCATENATED-STREAM"),
    (COMMON_LISP, "DOUBLE-FLOAT"),
    (COMMON_LISP, "ECHO-STREAM"),
    (COMMON_LISP, "EXTENDED-CHAR"),
    (COMMON_LISP, "FILE-STREAM"),
    (COMMON_LISP, "FIXNUM"),
    (COMMON_LISP, "HASH-TABLE"),
    (COMMON_LISP, "INTEGER"),
    (COMMON_LISP, "KEYWORD"),
    (COMMON_LISP, "KEYWORDP"),
    (COMMON_LISP, "LONG-FLOAT"),
    (COMMON_LISP, "NIL"),
    (COMMON_LISP, "NULL"),
    (COMMON_LISP, "NUMBER"),
    (COMMON_LISP, "PACKAGE"),
    (COMMON_LISP, "RANDOM-STATE"),
    (COMMON_LISP, "RATIO"),
    (COMMON_LISP, "REAL"),
    (COMMON_LISP, "RESTART"),
    (COMMON_LISP, "SEQUENCE"),
    (COMMON_LISP, "SHORT-FLOAT"),
    (COMMON_LISP, "SIGNED-BYTE"),
    (COMMON_LISP, "SIMPLE-ARRAY"),
    (COMMON_LISP, "SIMPLE-BASE-STRING"),
    (COMMON_LISP, "SIMPLE-BIT-VECTOR"),
    (COMMON_LISP, "SIMPLE-STRING"),
    (COMMON_LISP, "SIMPLE-VECTOR"),
    (COMMON_LISP, "SINGLE-FLOAT"),
    (COMMON_LISP, "STANDARD-CHAR"),
    (COMMON_LISP, "STREAM"),
    (COMMON_LISP, "STRING-STREAM"),
    (COMMON_LISP, "SUBTYPEP"),
    (COMMON_LISP, "SXHASH"),
    (COMMON_LISP, "SYMBOL"),
    (COMMON_LISP, "SYNONYM-STREAM"),
    (COMMON_LISP, "T"),
    (COMMON_LISP, "TWO-WAY-STREAM"),
    (COMMON_LISP, "TYPE-OF"),
    (COMMON_LISP, "TYPEP"),
    (COMMON_LISP, "UNSIGNED-BYTE"),
    (COMMON_LISP, "UPGRADED-COMPLEX-PART-TYPE"),
    (SB_EXT, "DOUBLE-FLOAT-NEGATIVE-INFINITY"),
    (SB_EXT, "DOUBLE-FLOAT-POSITIVE-INFINITY"),
    (SB_EXT, "FLOAT-DENORMALIZED-P"),
    (SB_EXT, "FLOAT-INFINITY-P"),
    (SB_EXT, "FLOAT-NAN-P"),
    (SB_EXT, "FLOAT-TRAPPING-NAN-P"),
    (SB_EXT, "LONG-FLOAT-NEGATIVE-INFINITY"),
    (SB_EXT, "LONG-FLOAT-POSITIVE-INFINITY"),
    (SB_EXT, "MOST-POSITIVE-WORD"),
    (SB_EXT, "SHORT-FLOAT-NEGATIVE-INFINITY"),
    (SB_EXT, "SHORT-FLOAT-POSITIVE-INFINITY"),
    (SB_EXT, "SINGLE-FLOAT-NEGATIVE-INFINITY"),
    (SB_EXT, "SINGLE-FLOAT-POSITIVE-INFINITY"),
    (SB_EXT, "WORD"),
];

/// Owned symbols whose constant bit must be set, as `(package, name)` pairs.
const CONSTANTS: &[(&str, &str)] = &[
    (COMMON_LISP, "NIL"),
    (COMMON_LISP, "T"),
    (SB_EXT, "DOUBLE-FLOAT-NEGATIVE-INFINITY"),
    (SB_EXT, "DOUBLE-FLOAT-POSITIVE-INFINITY"),
    (SB_EXT, "LONG-FLOAT-NEGATIVE-INFINITY"),
    (SB_EXT, "LONG-FLOAT-POSITIVE-INFINITY"),
    (SB_EXT, "MOST-POSITIVE-WORD"),
    (SB_EXT, "SHORT-FLOAT-NEGATIVE-INFINITY"),
    (SB_EXT, "SHORT-FLOAT-POSITIVE-INFINITY"),
    (SB_EXT, "SINGLE-FLOAT-NEGATIVE-INFINITY"),
    (SB_EXT, "SINGLE-FLOAT-POSITIVE-INFINITY"),
];

/// Class names to register.
const CLASSES: &[&str] = &[
    "ARRAY",
    "BASE-STRING",
    "BIGNUM",
    "BIT-VECTOR",
    "BROADCAST-STREAM",
    "CONCATENATED-STREAM",
    "DOUBLE-FLOAT",
    "ECHO-STREAM",
    "FILE-STREAM",
    "FIXNUM",
    "HASH-TABLE",
    "INTEGER",
    "NULL",
    "NUMBER",
    "PACKAGE",
    "RANDOM-STATE",
    "RATIO",
    "REAL",
    "RESTART",
    "SEQUENCE",
    "SIMPLE-ARRAY",
    "SIMPLE-BASE-STRING",
    "SIMPLE-BIT-VECTOR",
    "SIMPLE-STRING",
    "SIMPLE-VECTOR",
    "SINGLE-FLOAT",
    "STREAM",
    "STRING-STREAM",
    "SYMBOL",
    "SYNONYM-STREAM",
    "T",
    "TWO-WAY-STREAM",
];

/// Function names to register, as `(package, name)` pairs.
const FUNCTIONS: &[(&str, &str)] = &[
    (COMMON_LISP, "COERCE"),
    (COMMON_LISP, "KEYWORDP"),
    (COMMON_LISP, "NULL"),
    (COMMON_LISP, "SUBTYPEP"),
    (COMMON_LISP, "SXHASH"),
    (COMMON_LISP, "TYPE-OF"),
    (COMMON_LISP, "TYPEP"),
    (COMMON_LISP, "UPGRADED-COMPLEX-PART-TYPE"),
    (SB_EXT, "FLOAT-DENORMALIZED-P"),
    (SB_EXT, "FLOAT-INFINITY-P"),
    (SB_EXT, "FLOAT-NAN-P"),
    (SB_EXT, "FLOAT-TRAPPING-NAN-P"),
];

/// Register every owned type name, class name, and function name.
///
/// This is the per-crate registration function for `ncl-types`, called by
/// `ncl-stdlib::register_all` in dependency order. It interns the owned
/// symbols into their packages and records class and function names in the
/// runtime registries.
///
/// Class objects and function objects are placeholders (`Word::fixnum(1)` and
/// `Word::UNBOUND` respectively). Real class objects arrive with the
/// `ncl-clos` lane and real function objects (builtin ABI bindings) with the
/// `ncl-runtime` code objects.
///
/// # Errors
///
/// Propagates package, intern, and registration failures.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;

    runtime.ensure_package(&mut ctx, SB_EXT)?;

    for &(package, name) in SYMBOLS {
        let package = runtime
            .find_package(&ctx, package)
            .ok_or(ObjectError::PackageConflict)?;
        let _ = Package::from(package).intern(&mut ctx, runtime, name)?;
    }

    for &(package, name) in CONSTANTS {
        let package = runtime
            .find_package(&ctx, package)
            .ok_or(ObjectError::PackageConflict)?;
        let (symbol, _) = Package::from(package).intern(&mut ctx, runtime, name)?;
        set_symbol_constant(&mut ctx, symbol, true)?;
    }

    for &name in CLASSES {
        runtime.define_class(&mut ctx, name, Word::fixnum(1))?;
    }

    for &(package, name) in FUNCTIONS {
        runtime.define_function(&mut ctx, package, name, Word::UNBOUND)?;
    }

    Ok(())
}
