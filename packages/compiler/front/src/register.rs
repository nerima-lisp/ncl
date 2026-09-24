//! Registration of the symbols this crate owns.
//!
//! `register` interns every Phase 1 symbol the ownership table assigns to
//! `ncl-compiler-front` and sets the symbol-kind flag bits that describe the
//! symbol: `macro`, `constant`, and `special`. The `function` and `class` rows
//! are not registered here, because they require the actual functions and the
//! `FUNCTION` class; the back-half lane (L12b) supplies them.

use ncl_object::{
    ObjectError, Package, Runtime, ThreadContext, Word, set_symbol_constant, set_symbol_macro,
    set_symbol_special,
};

use crate::owned_symbols::{OWNED_SYMBOLS, SymbolKind};

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
        apply_kinds(&mut ctx, symbol, owned.kinds)?;
    }
    Ok(())
}

/// The number of symbols this crate owns.
#[must_use]
pub const fn owned_symbol_count() -> usize {
    OWNED_SYMBOLS.len()
}

/// Set the flag bit each kind implies.
///
/// `class`, `condition`, `function`, `other`, `special-operator`, and `type`
/// have no flag bit; the gate checks only that those symbols are interned.
fn apply_kinds(
    ctx: &mut ThreadContext,
    symbol: Word,
    kinds: &[SymbolKind],
) -> Result<(), ObjectError> {
    for kind in kinds {
        match kind {
            SymbolKind::Constant => set_symbol_constant(ctx, symbol, true)?,
            SymbolKind::Variable => set_symbol_special(ctx, symbol, true)?,
            SymbolKind::Macro => set_symbol_macro(ctx, symbol, true)?,
            SymbolKind::Class
            | SymbolKind::Condition
            | SymbolKind::Function
            | SymbolKind::Other
            | SymbolKind::SpecialOperator
            | SymbolKind::Type => {}
        }
    }
    Ok(())
}
