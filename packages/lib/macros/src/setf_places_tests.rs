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

#[test]
fn registered_place_expanders_emit_their_concrete_access_and_store_operators(
) -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    registry_with_builtins(&runtime, &mut ctx)?;
    let target = symbol(&mut ctx, &runtime, "TARGET")?;
    let index = Word::fixnum(1);
    type Expander = fn(&mut ThreadContext, &Runtime, &[Word]) -> Result<SetfExpansion, ObjectError>;

    let cases: [(&str, Expander, Vec<Word>, &str, &str); 9] = [
        ("CAR", car_place, vec![target], "CAR", "PROGN"),
        ("CDR", cdr_place, vec![target], "CDR", "PROGN"),
        ("FIRST", first_place, vec![target], "FIRST", "PROGN"),
        ("REST", rest_place, vec![target], "REST", "PROGN"),
        ("NTH", nth_place, vec![index, target], "NTH", "PROGN"),
        (
            "SYMBOL-VALUE",
            symbol_value_place,
            vec![target],
            "SYMBOL-VALUE",
            "SET",
        ),
        (
            "AREF",
            aref_place,
            vec![target, index],
            "AREF",
            "NCL-EXT::AREF-SET",
        ),
        (
            "SVREF",
            svref_place,
            vec![target, index],
            "SVREF",
            "NCL-EXT::SVREF-SET",
        ),
        (
            "GETHASH",
            gethash_place,
            vec![target, index],
            "GETHASH",
            "NCL-EXT::GETHASH-SET",
        ),
    ];

    for (name, expand, args, access_name, store_name) in cases {
        let expansion = expand(&mut ctx, &runtime, &args)?;
        assert_eq!( // check-added-lines: allow(panic,index) exact access operator assertion.
            elements(&mut ctx, expansion.access_form)?[0],
            symbol(&mut ctx, &runtime, access_name)?,
            "{name} access"
        );
        let store = elements(&mut ctx, expansion.store_form)?;
        assert_eq!( // check-added-lines: allow(panic,index) exact store operator assertion.
            store[0],
            symbol(&mut ctx, &runtime, store_name)?,
            "{name} store"
        );
        assert_eq!(expansion.store_variables.len(), 1, "{name} store variable"); // check-added-lines: allow(panic) exact expansion count assertion.
        assert_eq!(expansion.value_forms, args, "{name} value forms"); // check-added-lines: allow(panic) exact expansion assertion.
    }
    Ok(())
}
