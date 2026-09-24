//! Registration of the owned symbols and the alien type classes.

use ncl_object::{
    ObjectError, Package, Runtime, ThreadContext, Word, make_simple_vector, make_string,
    set_symbol_macro, set_symbol_special,
};

use crate::FfiError;
use crate::roots::with_root;
use crate::symbols::{SymbolRow, symbols};

/// Register every symbol owned by `ncl-ffi`.
///
/// This is the per-crate registration entry point that `ncl-stdlib` calls in
/// dependency order. It creates an internal context, so callers pass only the
/// shared runtime.
///
/// # Errors
/// Returns an object-layer error when allocation, interning, or registration
/// fails.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    for row in symbols() {
        register_symbol(runtime, &mut ctx, row)?;
    }
    install_classes(runtime, &mut ctx)?;
    Ok(())
}

/// Intern one symbol and set the bits its kind requires.
fn register_symbol(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    row: &SymbolRow,
) -> Result<(), ObjectError> {
    let package = runtime.ensure_package(ctx, row.package)?;
    let (symbol, _status) = Package::from(package).intern(ctx, runtime, row.name)?;
    if row.kind.defines_function() {
        runtime.define_function(ctx, row.package, row.name, Word::UNBOUND)?;
    }
    if row.kind.is_macro() {
        set_symbol_macro(ctx, symbol, true)?;
    }
    if row.kind.is_variable() {
        set_symbol_special(ctx, symbol, true)?;
    }
    Ok(())
}

/// Register the class object of every class-valued alien symbol.
fn install_classes(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
    for row in symbols() {
        if row.kind.defines_class() {
            install_class(runtime, ctx, row.name)?;
        }
    }
    Ok(())
}

/// Register one alien type class with a minimal descriptor.
fn install_class(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
) -> Result<(), ObjectError> {
    let mut name_word = make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
    with_root(ctx, &mut name_word, |ctx, name_word| {
        let descriptor = make_simple_vector(ctx, runtime, &[*name_word, Word::NIL, Word::NIL])?;
        runtime.define_class(ctx, name, descriptor)
    })
    .map_err(FfiError::into_object_error)
}
