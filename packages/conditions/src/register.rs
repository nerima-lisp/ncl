//! Registration of the owned symbols and the standard condition hierarchy.

use ncl_object::{ObjectError, Package, Runtime, ThreadContext, Word};

use crate::class::{HIERARCHY, install_class, wire_superclass};
use crate::symbols::{SYMBOLS, SymbolKind, SymbolRow};

/// Register every owned symbol and the standard condition hierarchy.
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
    for row in SYMBOLS {
        register_symbol(runtime, &mut ctx, row)?;
    }
    install_hierarchy(runtime, &mut ctx)?;
    Ok(())
}

fn register_symbol(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    row: &SymbolRow,
) -> Result<(), ObjectError> {
    let package = runtime.ensure_package(ctx, row.package)?;
    Package::from(package).intern(ctx, runtime, row.name)?;
    match row.kind {
        SymbolKind::Function | SymbolKind::ClassAndFunction => {
            runtime.define_function(ctx, row.package, row.name, Word::UNBOUND)?;
        }
        SymbolKind::Class | SymbolKind::Variable | SymbolKind::Other => {}
    }
    Ok(())
}

fn install_hierarchy(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
    for row in SYMBOLS {
        match row.kind {
            SymbolKind::Class | SymbolKind::ClassAndFunction => {
                install_class(ctx, runtime, row.name)?;
            }
            SymbolKind::Function | SymbolKind::Variable | SymbolKind::Other => {}
        }
    }
    for row in HIERARCHY {
        if let Some(parent) = row.superclass {
            wire_superclass(ctx, runtime, row.name, parent)?;
        }
    }
    Ok(())
}
