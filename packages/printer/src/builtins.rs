//! Registration of the printer's owned symbols and its dispatch table.

use ncl_object::{
    ObjectError, Package, Runtime, ThreadContext, Word, car, cdr, make_cons, pop_root, push_root,
    set_symbol_special, set_symbol_value,
};

/// The `(package, name)` functions `ncl-printer` owns.
const FUNCTIONS: [(&str, &str); 20] = [
    ("COMMON-LISP", "COPY-PPRINT-DISPATCH"),
    ("COMMON-LISP", "PPRINT"),
    ("COMMON-LISP", "PPRINT-DISPATCH"),
    ("COMMON-LISP", "PPRINT-FILL"),
    ("COMMON-LISP", "PPRINT-INDENT"),
    ("COMMON-LISP", "PPRINT-LINEAR"),
    ("COMMON-LISP", "PPRINT-NEWLINE"),
    ("COMMON-LISP", "PPRINT-TAB"),
    ("COMMON-LISP", "PPRINT-TABULAR"),
    ("COMMON-LISP", "PRIN1"),
    ("COMMON-LISP", "PRIN1-TO-STRING"),
    ("COMMON-LISP", "PRINC"),
    ("COMMON-LISP", "PRINC-TO-STRING"),
    ("COMMON-LISP", "PRINT"),
    ("COMMON-LISP", "PRINT-NOT-READABLE-OBJECT"),
    ("COMMON-LISP", "PRINT-OBJECT"),
    ("COMMON-LISP", "SET-PPRINT-DISPATCH"),
    ("COMMON-LISP", "WRITE-TO-STRING"),
    ("NCL-EXT", "PRINT-SYMBOL-WITH-PREFIX"),
    ("NCL-EXT", "PRINT-UNREADABLY"),
];

/// The `(package, name)` special variables `ncl-printer` owns.
const VARIABLES: [(&str, &str); 4] = [
    ("COMMON-LISP", "*PRINT-PPRINT-DISPATCH*"),
    ("COMMON-LISP", "*PRINT-READABLY*"),
    ("NCL-EXT", "*PRINT-CIRCLE-NOT-SHARED*"),
    ("NCL-EXT", "*PRINT-VECTOR-LENGTH*"),
];

/// Register every symbol `ncl-printer` owns with `runtime`.
///
/// Each function symbol is interned in its package and registered with an
/// unbound placeholder until the runtime supplies a callable function object.
/// The owned variables are interned, marked special, and initialised:
/// `*PRINT-PPRINT-DISPATCH*` to an empty dispatch table, the rest to `NIL`.
///
/// # Errors
///
/// Returns an [`ObjectError`] when a package cannot be created, an allocation
/// fails, or a symbol flag cannot be written.
pub fn register(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    for package in ["COMMON-LISP", "NCL-EXT"] {
        runtime.ensure_package(ctx, package)?;
    }
    for (package, name) in FUNCTIONS {
        let package_word = runtime.ensure_package(ctx, package)?;
        Package::from(package_word).intern(ctx, runtime, name)?;
        runtime.define_function(ctx, package, name, Word::UNBOUND)?;
    }
    for (package, name) in VARIABLES {
        let package = runtime.ensure_package(ctx, package)?;
        let (mut symbol, _status) = Package::from(package).intern(ctx, runtime, name)?;
        let token = push_root(ctx, &mut symbol);
        let result = initialise_variable(ctx, runtime, name, symbol);
        let _ = pop_root(ctx, token);
        result?;
    }
    Ok(())
}

/// Mark one owned variable special and give it its initial value.
fn initialise_variable(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    symbol: Word,
) -> Result<(), ObjectError> {
    set_symbol_special(ctx, symbol, true)?;
    let value = if name == "*PRINT-PPRINT-DISPATCH*" {
        default_table(ctx, runtime)?
    } else {
        Word::NIL
    };
    set_symbol_value(ctx, symbol, value)
}

/// Build a dispatch table whose default entry has no function.
///
/// The table is a list of `(type-specifier . function)` entries. The default
/// entry's specifier is `T`, which matches every object.
///
/// # Errors
///
/// Returns an [`ObjectError`] when an entry cannot be allocated.
pub fn default_table(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<Word, ObjectError> {
    let mut entry = make_cons(ctx, runtime, Word::TRUE, Word::NIL)?;
    let token = push_root(ctx, &mut entry);
    let result = make_cons(ctx, runtime, entry, Word::NIL);
    let _ = pop_root(ctx, token);
    result
}

/// Return the function `table` associates with `object`, or `NIL`.
///
/// A `T` entry matches every object; any other specifier matches by identity.
/// Type-specifier matching needs `ncl-types`, which is not on `main` yet, so
/// non-`T` entries compare with `eq`.
///
/// # Errors
///
/// Returns an [`ObjectError`] when a list accessor fails.
#[must_use = "the dispatch function is the result"]
pub fn pprint_dispatch(
    ctx: &mut ThreadContext,
    object: Word,
    table: Word,
) -> Result<Word, ObjectError> {
    let mut cursor = table;
    while cursor != Word::NIL {
        let entry = car(ctx, cursor)?;
        if entry.is_cons() {
            let specifier = car(ctx, entry)?;
            let function = cdr(ctx, entry)?;
            if specifier == Word::TRUE || specifier == object {
                return Ok(function);
            }
        }
        cursor = cdr(ctx, cursor)?;
    }
    Ok(Word::NIL)
}

/// Return `table` with `(type_specifier . function)` prepended.
///
/// The new entry shadows any earlier entry with the same specifier, because
/// lookup returns the first match.
///
/// # Errors
///
/// Returns an [`ObjectError`] when an entry cannot be allocated.
#[must_use = "the updated table is the result"]
pub fn set_pprint_dispatch(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    type_specifier: Word,
    function: Word,
    table: Word,
) -> Result<Word, ObjectError> {
    let mut entry = make_cons(ctx, runtime, type_specifier, function)?;
    let token = push_root(ctx, &mut entry);
    let result = make_cons(ctx, runtime, entry, table);
    let _ = pop_root(ctx, token);
    result
}

/// Return a shallow copy of a dispatch table.
///
/// # Errors
///
/// Returns an [`ObjectError`] when an entry cannot be allocated.
#[must_use = "the copied table is the result"]
pub fn copy_pprint_dispatch(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    table: Word,
) -> Result<Word, ObjectError> {
    let mut source = table;
    let mut copied = Word::NIL;
    let source_token = push_root(ctx, &mut source);
    let copied_token = push_root(ctx, &mut copied);
    let result = copy_entries(ctx, runtime, &mut source, &mut copied);
    let _ = pop_root(ctx, copied_token);
    let _ = pop_root(ctx, source_token);
    result?;
    Ok(copied)
}

fn copy_entries(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut Word,
    copied: &mut Word,
) -> Result<(), ObjectError> {
    while *source != Word::NIL {
        let entry = car(ctx, *source)?;
        *copied = make_cons(ctx, runtime, entry, *copied)?;
        *source = cdr(ctx, *source)?;
    }
    Ok(())
}
