//! Registration of the owned symbols and the thread-layer classes.
//!
//! This is the per-crate entry point that `ncl-stdlib` calls in dependency
//! order. It interns every `conformance/ownership/symbols.tsv` row assigned to
//! `ncl-threads`, sets the special, macro, and constant bits the ownership gate
//! checks, registers the runtime-side function for each function and macro
//! symbol, and installs the class descriptors the object model uses.

use ncl_object::{
    ObjectError, Package, Runtime, ThreadContext, Word, make_simple_vector, make_string, pop_root,
    push_root, set_symbol_constant, set_symbol_macro, set_symbol_special,
};

use crate::symbols::{SymbolKind, SymbolRow, rows};

/// Classes installed by this crate, paired with their home package.
///
/// The first seven come from the ownership table; `PROCESS`, `RWLOCK`, and
/// `SPINLOCK` are support classes for objects this crate creates. `SPINLOCK` is
/// a `type` row in the table and is interned as well as installed here.
const CLASSES: &[(&str, &str)] = &[
    ("TIMER", "SB-EXT"),
    ("FOREIGN-THREAD", "SB-THREAD"),
    ("MUTEX", "SB-THREAD"),
    ("RWLOCK", "SB-THREAD"),
    ("SEMAPHORE", "SB-THREAD"),
    ("SEMAPHORE-NOTIFICATION", "SB-THREAD"),
    ("SPINLOCK", "SB-THREAD"),
    ("THREAD", "SB-THREAD"),
    ("WAITQUEUE", "SB-THREAD"),
    ("PROCESS", "SB-EXT"),
];

/// Register every owned symbol and class with `runtime`.
///
/// # Errors
/// Returns an object-layer error when allocation, interning, or registration
/// fails.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    for row in rows() {
        register_row(runtime, &mut ctx, row)?;
    }
    install_classes(runtime, &mut ctx)
}

fn register_row(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    row: &SymbolRow,
) -> Result<(), ObjectError> {
    let package = runtime.ensure_package(ctx, row.package)?;
    let (mut symbol, _status) = Package::from(package).intern(ctx, runtime, row.name)?;
    let token = push_root(ctx, &mut symbol);
    let result = apply_kind(runtime, ctx, row, symbol);
    let _ = pop_root(ctx, token);
    result
}

fn apply_kind(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    row: &SymbolRow,
    symbol: Word,
) -> Result<(), ObjectError> {
    match row.kind {
        SymbolKind::Function => runtime.define_function(ctx, row.package, row.name, Word::UNBOUND),
        SymbolKind::Macro => {
            runtime.define_function(ctx, row.package, row.name, Word::UNBOUND)?;
            set_symbol_macro(ctx, symbol, true)
        }
        SymbolKind::Variable => set_symbol_special(ctx, symbol, true),
        SymbolKind::Constant => set_symbol_constant(ctx, symbol, true),
        SymbolKind::Class | SymbolKind::Type | SymbolKind::Other
        | SymbolKind::ClassAndFunction | SymbolKind::MacroAndClass
        | SymbolKind::SpecialOperatorAndClass | SymbolKind::VariableAndFunction => Ok(()),
    }
}

fn install_classes(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
    for (name, package) in CLASSES {
        let package = runtime.ensure_package(ctx, package)?;
        let _ = Package::from(package).intern(ctx, runtime, name)?;
        let descriptor = class_descriptor(ctx, runtime, name)?;
        runtime.define_class(ctx, *name, descriptor)?;
    }
    Ok(())
}

fn class_descriptor(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Result<Word, ObjectError> {
    let name_word = make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
    make_simple_vector(ctx, runtime, &[name_word, Word::NIL, Word::NIL])
}
