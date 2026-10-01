use super::*;
use crate::form::elements;
use crate::place::PlaceRegistry;
use crate::setf::expand_setf;
use ncl_object::{Runtime, ThreadContext};

fn registry_with_builtins(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
    // The accessor builtins referenced by these expansions (CAR, RPLACA,
    // SET, ...) live in other crates and are not needed to build the
    // expansion form itself; only the place-expander registration (this
    // crate's `register`) and symbol interning are required here.
    ctx.register(runtime)?;
    crate::register(runtime)?;
    Ok(())
}

/// Each generalized place this module registers must expand into a non-empty
/// form and survive a GC cycle under `gc_stress` + strict forwarding, both
/// before and after the place is evaluated once more.
#[test]
fn generalized_places_survive_gc_stress_and_strict_forwarding() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    registry_with_builtins(&runtime, &mut ctx)?;
    ctx.set_strict_forwarding(true);

    let registry = PlaceRegistry::new(&runtime);
    let list_symbol = symbol(&mut ctx, &runtime, "L")?;
    let var_symbol = symbol(&mut ctx, &runtime, "X")?;
    let array_symbol = symbol(&mut ctx, &runtime, "ARRAY")?;

    let mut cases = Vec::new();
    macro_rules! place {
        ($operator:literal, [$($arg:expr),* $(,)?]) => {{
            let operator = symbol(&mut ctx, &runtime, $operator)?;
            let args = [$($arg),*];
            let mut values = Vec::with_capacity(args.len() + 1);
            values.push(operator);
            values.extend_from_slice(&args);
            cases.push(list(&mut ctx, &runtime, &values)?);
        }};
    }
    place!("CAR", [list_symbol]);
    place!("CDR", [list_symbol]);
    place!("FIRST", [list_symbol]);
    place!("REST", [list_symbol]);
    place!("NTH", [Word::fixnum(0), list_symbol]);
    place!("SYMBOL-VALUE", [var_symbol]);
    place!("SBIT", [array_symbol, Word::fixnum(0)]);
    cases.push(var_symbol);

    ctx.set_gc_stress(true);
    ncl_object::with_roots(&mut ctx, &cases, |ctx, roots| {
        for place_root in roots {
            let value = Word::fixnum(42);
            let mut expanded = expand_setf(ctx, &runtime, &registry, &[**place_root, value])?;
            // Only aarch64 observes this relocation: an object found by
            // conservative scanning is pinned and skipped by `move_live_objects`,
            // and on x86-64 `heap_collect` refreshes that snapshot (callee-saved
            // registers plus the whole stack) at the collection site, so a live
            // local keeps its address. aarch64 keeps the older snapshot, sees the
            // value only through the precise `push_root` slot, and forwards it.
            #[cfg(target_arch = "aarch64")]
            let before = expanded;
            let root = ncl_object::push_root(ctx, &mut expanded);
            ctx.collect(true)?;
            // check-added-lines: allow(panic) test-only assertion
            #[cfg(target_arch = "aarch64")]
            assert_ne!(expanded, before);
            // check-added-lines: allow(panic) test-only assertion
            assert!(!elements(ctx, expanded)?.is_empty());
            // check-added-lines: allow(panic) test-only assertion
            assert!(ncl_object::pop_root(ctx, root));
        }
        Ok(())
    })
}
