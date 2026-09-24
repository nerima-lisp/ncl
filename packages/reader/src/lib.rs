//! The Lisp reader: standard readtable, macro and dispatch characters, and the
//! recursive `read` entry points.
//!
//! `ncl-reader` depends only on `ncl-object`. It builds Lisp objects (`Word`
//! values) for symbols, numbers, strings, characters, conses, vectors, and
//! specialized arrays, and registers the 23 symbols it owns from the
//! conformance ownership table.
//!
//! The readtable wraps the `ncl_object` readtable descriptor; its syntax and
//! dispatch tables are 256-entry simple vectors indexed by character code.
//! Dynamic variables (`*read-base*`, `*read-eval*`, `*read-suppress*`,
//! `*read-default-float-format*`, and the active readtable) are passed
//! explicitly through [`ReadOptions`] because special-variable rebinding is not
//! wired in this phase.

#![allow(
    clippy::cast_possible_truncation,
    reason = "character codes are checked against the 256-entry table before indexing, and i128 values are range-checked against the fixnum range before the i64 cast"
)]
#![allow(
    clippy::too_many_arguments,
    reason = "the reader threads ctx, runtime, source, options, and its rooted slots"
)]
#![allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "rooted Word slots are passed by reference so each read re-reads the collector-updated value; a by-value copy would be stale after GC"
)]

mod dispatch;
mod error;
mod features;
mod input;
mod number;
mod reader;
mod readtable;
mod token;

use ncl_object::{ObjectError, Package, Runtime, ThreadContext, Word};

pub use error::ReadError;
pub use input::{CharSource, StringSource};
pub use number::parse_integer;
pub use reader::{
    FloatFormat, ReadOptions, read, read_delimited_list, read_from_string,
    read_preserving_whitespace,
};
pub use readtable::{
    Readtable, ReadtableCase, copy_readtable, get_dispatch_macro_character, get_macro_character,
    make_dispatch_macro_character, readtable_case, readtablep, set_dispatch_macro_character,
    set_macro_character, set_syntax_from_char, standard_readtable,
};

/// The package, name, and registration kind of one owned symbol.
#[derive(Clone, Copy)]
enum SymbolKind {
    /// A special variable: interned only.
    Variable,
    /// A function: interned and registered with [`Runtime::define_function`].
    Function,
    /// A class: interned and registered with [`Runtime::define_class`].
    Class,
    /// A macro: interned only.
    Macro,
}

/// Every Phase 1 symbol owned by `ncl-reader`.
const OWNED_SYMBOLS: &[(&str, &str, SymbolKind)] = &[
    ("COMMON-LISP", "*READ-BASE*", SymbolKind::Variable),
    (
        "COMMON-LISP",
        "*READ-DEFAULT-FLOAT-FORMAT*",
        SymbolKind::Variable,
    ),
    ("COMMON-LISP", "*READ-EVAL*", SymbolKind::Variable),
    ("COMMON-LISP", "*READ-SUPPRESS*", SymbolKind::Variable),
    ("COMMON-LISP", "*READTABLE*", SymbolKind::Variable),
    ("COMMON-LISP", "READTABLE", SymbolKind::Class),
    ("COMMON-LISP", "WITH-STANDARD-IO-SYNTAX", SymbolKind::Macro),
    ("COMMON-LISP", "COPY-READTABLE", SymbolKind::Function),
    (
        "COMMON-LISP",
        "GET-DISPATCH-MACRO-CHARACTER",
        SymbolKind::Function,
    ),
    ("COMMON-LISP", "GET-MACRO-CHARACTER", SymbolKind::Function),
    (
        "COMMON-LISP",
        "MAKE-DISPATCH-MACRO-CHARACTER",
        SymbolKind::Function,
    ),
    ("COMMON-LISP", "PARSE-INTEGER", SymbolKind::Function),
    ("COMMON-LISP", "READ", SymbolKind::Function),
    ("COMMON-LISP", "READ-DELIMITED-LIST", SymbolKind::Function),
    ("COMMON-LISP", "READ-FROM-STRING", SymbolKind::Function),
    (
        "COMMON-LISP",
        "READ-PRESERVING-WHITESPACE",
        SymbolKind::Function,
    ),
    ("COMMON-LISP", "READTABLE-CASE", SymbolKind::Function),
    ("COMMON-LISP", "READTABLEP", SymbolKind::Function),
    (
        "COMMON-LISP",
        "SET-DISPATCH-MACRO-CHARACTER",
        SymbolKind::Function,
    ),
    ("COMMON-LISP", "SET-MACRO-CHARACTER", SymbolKind::Function),
    ("COMMON-LISP", "SET-SYNTAX-FROM-CHAR", SymbolKind::Function),
    (
        "SB-EXT",
        "READTABLE-BASE-CHAR-PREFERENCE",
        SymbolKind::Function,
    ),
    ("SB-EXT", "READTABLE-NORMALIZATION", SymbolKind::Function),
];

/// Intern every owned symbol and register the function and class entries.
///
/// Function and class words are placeholders ([`Word::UNBOUND`]) until a
/// callable function ABI and CLOS class objects land in a later lane; the
/// ownership gate checks presence, not callability.
///
/// # Errors
/// Returns an object-layer failure when a package or symbol cannot be created.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    runtime.ensure_package(&mut ctx, "SB-EXT")?;
    for &(package, name, kind) in OWNED_SYMBOLS {
        let package_word = runtime
            .find_package(&ctx, package)
            .ok_or(ObjectError::Layout)?;
        let (symbol, _) = Package::from(package_word).intern(&mut ctx, runtime, name)?;
        match kind {
            SymbolKind::Variable => ncl_object::set_symbol_special(&mut ctx, symbol, true)?,
            SymbolKind::Function => {
                runtime.define_function(&mut ctx, package, name, Word::UNBOUND)?;
            }
            SymbolKind::Class => {
                runtime.define_class(&mut ctx, name, Word::UNBOUND)?;
            }
            SymbolKind::Macro => ncl_object::set_symbol_macro(&mut ctx, symbol, true)?,
        }
    }
    Ok(())
}
