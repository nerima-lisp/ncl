//! Precise roots for values that cross an allocation or a foreign call.
//!
//! `ncl-object` keeps `with_root` / `with_roots` crate-internal, so this module
//! provides the same discipline over the public `push_root` / `pop_root` API:
//! the slot is interior-mutable and the closure receives an `ncl_sys::RootSlot`
//! the collector rewrites in place.

use core::cell::Cell;

use ncl_object::{ThreadContext, Word, pop_root, push_root};
use ncl_sys::{RootSlot, RootToken};

use crate::FfiError;

/// Root `value` across allocations and hand the closure its live slot.
///
/// # Errors
/// Returns [`FfiError::RootStackCorrupt`] when the token does not pop in stack
/// order, and any error the closure returns.
pub fn with_root<T, E>(
    ctx: &mut ThreadContext,
    value: &mut Word,
    f: impl FnOnce(&mut ThreadContext, RootSlot<'_>) -> Result<T, E>,
) -> Result<T, FfiError>
where
    FfiError: From<E>,
{
    let mut slot = Cell::new(*value);
    let token = push_root(ctx, slot.get_mut());
    let result = f(ctx, RootSlot::new(&slot));
    *value = slot.get();
    finish(ctx, token)?;
    result.map_err(FfiError::from)
}

/// Root every value in `values` across allocations.
///
/// # Errors
/// Returns [`FfiError::RootStackCorrupt`] when a token does not pop in stack
/// order, and any error the closure returns.
pub fn with_roots<T, E>(
    ctx: &mut ThreadContext,
    values: &[Word],
    f: impl FnOnce(&mut ThreadContext, &[RootSlot<'_>]) -> Result<T, E>,
) -> Result<T, FfiError>
where
    FfiError: From<E>,
{
    let mut cells: Vec<Cell<Word>> = values.iter().copied().map(Cell::new).collect();
    let mut tokens: Vec<RootToken> = Vec::with_capacity(cells.len());
    for cell in &mut cells {
        tokens.push(push_root(ctx, cell.get_mut()));
    }
    let slots: Vec<RootSlot<'_>> = cells.iter().map(RootSlot::new).collect();
    let result = f(ctx, &slots);
    for token in tokens.into_iter().rev() {
        finish(ctx, token)?;
    }
    result.map_err(FfiError::from)
}

/// Pop one root token, reporting a broken root stack.
fn finish(ctx: &mut ThreadContext, token: RootToken) -> Result<(), FfiError> {
    if pop_root(ctx, token) {
        Ok(())
    } else {
        Err(FfiError::RootStackCorrupt)
    }
}
