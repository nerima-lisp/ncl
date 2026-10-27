//! The Lisp reader: standard readtable, macro and dispatch characters, and the
//! recursive `read` entry points.
//!
//! `ncl-reader` depends only on `ncl-object`. It builds Lisp objects (`Word`
//! values) for symbols, numbers, strings, characters, conses, vectors, and
//! specialized arrays, and registers the 21 symbols it owns from the
//! conformance ownership table.
//!
//! The readtable wraps the `ncl_object` readtable descriptor; its syntax and
//! dispatch tables are 256-entry simple vectors indexed by character code.
//! Runtime `#.` dispatch reads the live `*read-eval*` value. Other reader
//! settings pass through [`ReadOptions`]; dynamic `*readtable*` rebinding is not
//! wired into lower-level reader calls.

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

mod copy_readtable;
mod copy_readtable_builtin;
mod dispatch;
mod error;
mod features;
mod input;
mod number;
mod reader;
mod readtable;
pub(crate) mod token;

use ncl_object::{ObjectError, Package, Runtime, ThreadContext, set_symbol_value, with_root};

pub use copy_readtable::{copy_readtable, copy_readtable_to};
pub use error::ReadError;
pub use input::{CharSource, StringSource};
pub use number::parse_integer;
pub use reader::{
    FloatFormat, PackageName, ReadBase, ReadEvaluation, ReadOptions, ReadSuppression, read,
    read_delimited_list, read_from_string, read_preserving_whitespace,
};
pub use readtable::{
    Readtable, ReadtableCase, get_dispatch_macro_character, get_macro_character,
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
];

fn set_common_lisp_variable_value(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    value: ncl_object::Word,
) -> Result<(), ObjectError> {
    let value = ncl_object::HeapRootedWord::new(ctx, value)?;
    let mut common_lisp = runtime
        .find_package(ctx, "COMMON-LISP")
        .ok_or(ObjectError::Layout)?;
    with_root(ctx, &mut common_lisp, |ctx, common_lisp| {
        let (mut symbol, _) = Package::from_word(common_lisp.get()).intern(ctx, runtime, name)?;
        with_root(ctx, &mut symbol, |ctx, symbol| {
            set_symbol_value(ctx, symbol.get(), value.get())
        })
    })
}

/// Intern every owned symbol and install metadata for implemented entries.
///
/// `COPY-READTABLE` is callable. Other reader functions and the `READTABLE`
/// class remain interned until their implementations exist.
///
/// # Errors
/// Returns an object-layer failure when a package or symbol cannot be created.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    for &(package, name, kind) in OWNED_SYMBOLS {
        let package_word = runtime
            .find_package(&ctx, package)
            .ok_or(ObjectError::Layout)?;
        let (symbol, _) = Package::from_word(package_word).intern(&mut ctx, runtime, name)?;
        match kind {
            SymbolKind::Variable => ncl_object::set_symbol_special(&mut ctx, symbol, true)?,
            SymbolKind::Function | SymbolKind::Class => {}
        }
    }
    let mut standard_readtable = readtable::standard_readtable_object(&mut ctx, runtime)?
        .object()
        .as_word();
    with_root(
        &mut ctx,
        &mut standard_readtable,
        |ctx, standard_readtable| {
            let mut common_lisp = runtime
                .find_package(ctx, "COMMON-LISP")
                .ok_or(ObjectError::Layout)?;
            with_root(ctx, &mut common_lisp, |ctx, common_lisp| {
                let (mut symbol, _) =
                    Package::from_word(common_lisp.get()).intern(ctx, runtime, "*READTABLE*")?;
                with_root(ctx, &mut symbol, |ctx, symbol| {
                    set_symbol_value(ctx, symbol.get(), standard_readtable.get())
                })
            })
        },
    )?;
    set_common_lisp_variable_value(
        &mut ctx,
        runtime,
        "*READ-BASE*",
        ncl_object::Word::fixnum(10),
    )?;
    let mut common_lisp = runtime
        .find_package(&ctx, "COMMON-LISP")
        .ok_or(ObjectError::Layout)?;
    with_root(&mut ctx, &mut common_lisp, |ctx, common_lisp| {
        let (mut default_float_format, _) =
            Package::from_word(common_lisp.get()).intern(ctx, runtime, "SINGLE-FLOAT")?;
        with_root(
            ctx,
            &mut default_float_format,
            |ctx, default_float_format| {
                set_common_lisp_variable_value(
                    ctx,
                    runtime,
                    "*READ-DEFAULT-FLOAT-FORMAT*",
                    default_float_format.get(),
                )
            },
        )?;
        let (mut truth, _) = Package::from_word(common_lisp.get()).intern(ctx, runtime, "T")?;
        with_root(ctx, &mut truth, |ctx, truth| {
            set_common_lisp_variable_value(ctx, runtime, "*READ-EVAL*", truth.get())
        })
    })?;
    set_common_lisp_variable_value(&mut ctx, runtime, "*READ-SUPPRESS*", ncl_object::Word::NIL)?;
    copy_readtable_builtin::register(&mut ctx, runtime)?;
    Ok(())
}
