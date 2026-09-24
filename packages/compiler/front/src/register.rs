//! Registration of the symbols this crate owns.
//!
//! `register` interns every Phase 1 symbol the ownership table assigns to
//! `ncl-compiler-front` and registers the object each kind implies: the
//! `macro`, `constant`, and `special` flag bits, a function object for each
//! `function`-kind row, and the `FUNCTION` class. The function and class
//! entries are `Word::UNBOUND` placeholders until `ncl-runtime` (L23) supplies
//! the real function objects and `ncl-clos` (L18) the real class.

use ncl_object::{
    ObjectError, Package, Runtime, ThreadContext, Word, set_symbol_constant, set_symbol_macro,
    set_symbol_special,
};

use crate::owned_symbols::{OWNED_SYMBOLS, OwnedSymbol, SymbolKind};

/// Register every symbol this crate owns.
///
/// This is the per-crate registration entry point that `ncl-stdlib` calls in
/// dependency order. It creates an internal context, so callers pass only the
/// shared runtime, and it is idempotent.
///
/// # Errors
///
/// Returns an object-layer error when a package cannot be created or a symbol
/// cannot be interned or flagged.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    for owned in OWNED_SYMBOLS {
        let package = runtime.ensure_package(&mut ctx, owned.package)?;
        let (symbol, _) = Package::from(package).intern(&mut ctx, runtime, owned.name)?;
        apply_kinds(&mut ctx, runtime, symbol, owned)?;
    }
    Ok(())
}

/// The number of symbols this crate owns.
#[must_use]
pub const fn owned_symbol_count() -> usize {
    OWNED_SYMBOLS.len()
}

/// Register the object each kind implies.
///
/// `macro`, `variable`, and `constant` set a symbol flag bit; the flag bits are
/// applied before any registry call so no allocation can move `symbol` while it
/// is still in use. `function` and `class`/`condition` register an object in the
/// runtime registry, because the ownership gate looks those up by name; the
/// value is `Word::UNBOUND` until the owning lane supplies the real object.
/// `other`, `special-operator`, and `type` are interned only.
fn apply_kinds(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    symbol: Word,
    owned: &OwnedSymbol,
) -> Result<(), ObjectError> {
    for kind in owned.kinds {
        match kind {
            SymbolKind::Constant => set_symbol_constant(ctx, symbol, true)?,
            SymbolKind::Variable => set_symbol_special(ctx, symbol, true)?,
            SymbolKind::Macro => set_symbol_macro(ctx, symbol, true)?,
            SymbolKind::Function
            | SymbolKind::Class
            | SymbolKind::Condition
            | SymbolKind::Other
            | SymbolKind::SpecialOperator
            | SymbolKind::Type => {}
        }
    }
    for kind in owned.kinds {
        match kind {
            SymbolKind::Function => {
                runtime.define_function(ctx, owned.package, owned.name, Word::UNBOUND)?;
            }
            SymbolKind::Class | SymbolKind::Condition => {
                runtime.define_class(ctx, owned.name, Word::UNBOUND)?;
            }
            SymbolKind::Constant
            | SymbolKind::Variable
            | SymbolKind::Macro
            | SymbolKind::Other
            | SymbolKind::SpecialOperator
            | SymbolKind::Type => {}
        }
    }
    Ok(())
}
