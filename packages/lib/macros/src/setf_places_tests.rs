use super::*;
use crate::form::elements;
use crate::place::PlaceRegistry;
use crate::setf::{
    expand_decf, expand_get_setf_expansion, expand_incf, expand_pop, expand_psetf, expand_push,
    expand_remf, expand_rotatef, expand_setf, expand_shiftf,
};
use ncl_object::{HeapRootedWord, Runtime, ThreadContext};

fn registry_with_builtins(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
    // The accessor builtins referenced by these expansions (CAR, RPLACA,
    // SET, ...) live in other crates and are not needed to build the
    // expansion form itself; only the place-expander registration (this
    // crate's `register`) and symbol interning are required here.
    ctx.register(runtime)?;
    crate::register(runtime)?;
    Ok(())
}

fn compound_accessor_names() -> Vec<String> {
    (1..=4)
        .flat_map(|depth| {
            (0..(1 << depth)).map(move |bits| {
                let path = (0..depth)
                    .map(|index| if bits & (1 << index) == 0 { 'A' } else { 'D' })
                    .collect::<String>();
                format!("C{path}R")
            })
        })
        .collect()
}

fn make_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    operator: &str,
    variable: Word,
) -> Result<HeapRootedWord, ObjectError> {
    let operator = symbol(ctx, runtime, operator)?;
    let operator = HeapRootedWord::new(ctx, operator)?;
    let place = list(ctx, runtime, &[operator.get(), variable])?;
    HeapRootedWord::new(ctx, place)
}

fn assert_expansion(
    ctx: &mut ThreadContext,
    result: Result<Word, ObjectError>,
    place_name: &str,
    operation: &str,
) -> Result<(), ObjectError> {
    let succeeded = result.is_ok();
    assert!(
        succeeded,
        "{operation} expansion failed for {place_name}: {:?}",
        result.as_ref().err()
    );
    let Some(mut expansion) = result.ok() else {
        return Ok(());
    };
    let root = ncl_object::push_root(ctx, &mut expansion);
    let nonempty = !elements(ctx, expansion)?.is_empty();
    let popped = ncl_object::pop_root(ctx, root);
    assert!(nonempty, "{operation} expansion was empty for {place_name}");
    assert!(
        popped,
        "{operation} expansion root was lost for {place_name}"
    );
    Ok(())
}

fn contains_nested_the(
    ctx: &mut ThreadContext,
    form: Word,
    the: Word,
    outermost_type: Word,
    nested_type: Word,
) -> Result<bool, ObjectError> {
    if !form.is_cons() {
        return Ok(false);
    }
    let parts = elements(ctx, form)?;
    if parts.len() == 3 && parts[0] == the && parts[1] == outermost_type && parts[2].is_cons() {
        let inner = elements(ctx, parts[2])?;
        if inner.len() == 3 && inner[0] == the && inner[1] == nested_type {
            return Ok(true);
        }
    }
    for part in parts {
        if contains_nested_the(ctx, part, the, outermost_type, nested_type)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Each generalized place must expand into a non-empty form that survives a
/// full collection under `gc_stress` + strict forwarding.
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
    place!("ELT", [list_symbol, Word::fixnum(0)]);
    place!("SUBSEQ", [list_symbol, Word::fixnum(0), Word::fixnum(1)]);
    place!("CADR", [list_symbol]);
    place!("SYMBOL-VALUE", [var_symbol]);
    place!("SBIT", [var_symbol, Word::fixnum(0)]);
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
fn standardized_cons_places_expand_for_setf_and_incf() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    registry_with_builtins(&runtime, &mut ctx)?;
    ctx.set_strict_forwarding(true);

    let registry = PlaceRegistry::new(&runtime);
    let variable = symbol(&mut ctx, &runtime, "X")?;
    let variable = HeapRootedWord::new(&mut ctx, variable)?;
    let mut cases = Vec::<(String, HeapRootedWord)>::new();
    for name in compound_accessor_names()
        .into_iter()
        .chain((2..=10).map(|index| match index {
            2 => "SECOND".to_owned(),
            3 => "THIRD".to_owned(),
            4 => "FOURTH".to_owned(),
            5 => "FIFTH".to_owned(),
            6 => "SIXTH".to_owned(),
            7 => "SEVENTH".to_owned(),
            8 => "EIGHTH".to_owned(),
            9 => "NINTH".to_owned(),
            _ => "TENTH".to_owned(),
        }))
    {
        let place = make_place(&mut ctx, &runtime, &name, variable.get())?;
        cases.push((name, place));
    }
    let documentation = symbol(&mut ctx, &runtime, "DOCUMENTATION")?;
    let documentation = HeapRootedWord::new(&mut ctx, documentation)?;
    let kind = symbol(&mut ctx, &runtime, "T")?;
    let kind = HeapRootedWord::new(&mut ctx, kind)?;
    let package = symbol(&mut ctx, &runtime, "P")?;
    let package = HeapRootedWord::new(&mut ctx, package)?;
    let documentation_place = list(
        &mut ctx,
        &runtime,
        &[documentation.get(), package.get(), kind.get()],
    )?;
    cases.push((
        "(DOCUMENTATION P T)".to_owned(),
        HeapRootedWord::new(&mut ctx, documentation_place)?,
    ));

    for (name, place) in cases {
        let setf_arguments = [place.get(), Word::fixnum(41)];
        let result = expand_setf(&mut ctx, &runtime, &registry, &setf_arguments);
        assert_expansion(&mut ctx, result, &name, "SETF")?;
        let incf_arguments = [place.get()];
        let result = expand_incf(&mut ctx, &runtime, &registry, &incf_arguments);
        assert_expansion(&mut ctx, result, &name, "INCF")?;
    }

    let second = symbol(&mut ctx, &runtime, "SECOND")?;
    let second = HeapRootedWord::new(&mut ctx, second)?;
    let malformed_place = list(
        &mut ctx,
        &runtime,
        &[second.get(), variable.get(), Word::fixnum(0)],
    )?;
    let malformed_place = HeapRootedWord::new(&mut ctx, malformed_place)?;
    let result = expand_setf(
        &mut ctx,
        &runtime,
        &registry,
        &[malformed_place.get(), Word::fixnum(41)],
    );
    assert!(
        matches!(result, Err(ObjectError::TypeError)),
        "SECOND place with an extra argument must be rejected",
    );

    let documentation = symbol(&mut ctx, &runtime, "DOCUMENTATION")?;
    let malformed_documentation = list(&mut ctx, &runtime, &[documentation, variable.get()])?;
    let result = expand_setf(
        &mut ctx,
        &runtime,
        &registry,
        &[malformed_documentation, Word::fixnum(41)],
    );
    assert!(
        matches!(result, Err(ObjectError::TypeError)),
        "DOCUMENTATION place with a missing documentation type must be rejected",
    );
    Ok(())
}

#[test]
fn modifying_operators_reject_malformed_arity_and_pushnew_keys() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    registry_with_builtins(&runtime, &mut ctx)?;
    let registry = PlaceRegistry::new(&runtime);
    let place = symbol(&mut ctx, &runtime, "X")?;
    let key = symbol(&mut ctx, &runtime, "KEYWORD::KEY")?;
    let test = symbol(&mut ctx, &runtime, "KEYWORD::TEST")?;
    let test_not = symbol(&mut ctx, &runtime, "KEYWORD::TEST-NOT")?;
    let unknown = symbol(&mut ctx, &runtime, "KEYWORD::UNKNOWN")?;

    assert!(
        matches!(
            expand_incf(
                &mut ctx,
                &runtime,
                &registry,
                &[place, Word::fixnum(1), Word::fixnum(2)]
            ),
            Err(ObjectError::TypeError)
        ),
        "INCF must reject extra arguments",
    );
    assert!(
        matches!(
            expand_decf(
                &mut ctx,
                &runtime,
                &registry,
                &[place, Word::fixnum(1), Word::fixnum(2)]
            ),
            Err(ObjectError::TypeError)
        ),
        "DECF must reject extra arguments",
    );
    assert!(
        matches!(
            expand_push(
                &mut ctx,
                &runtime,
                &registry,
                &[Word::fixnum(1), place, Word::NIL],
                false
            ),
            Err(ObjectError::TypeError)
        ),
        "PUSH must reject extra arguments",
    );
    assert!(
        matches!(
            expand_push(
                &mut ctx,
                &runtime,
                &registry,
                &[Word::fixnum(1), place, key],
                true
            ),
            Err(ObjectError::TypeError)
        ),
        "PUSHNEW must reject a missing keyword value",
    );
    assert!(
        matches!(
            expand_push(
                &mut ctx,
                &runtime,
                &registry,
                &[Word::fixnum(1), place, unknown, Word::NIL],
                true,
            ),
            Err(ObjectError::TypeError)
        ),
        "PUSHNEW must reject unsupported keyword names",
    );
    assert!(
        matches!(
            expand_push(
                &mut ctx,
                &runtime,
                &registry,
                &[Word::fixnum(1), place, test, Word::NIL, test_not, Word::NIL,],
                true,
            ),
            Err(ObjectError::TypeError)
        ),
        "PUSHNEW must reject TEST and TEST-NOT together",
    );
    assert!(
        matches!(
            expand_pop(&mut ctx, &runtime, &registry, &[place, Word::NIL]),
            Err(ObjectError::TypeError)
        ),
        "POP must reject extra arguments",
    );
    assert!(
        matches!(
            expand_remf(
                &mut ctx,
                &runtime,
                &registry,
                &[place, Word::NIL, Word::NIL]
            ),
            Err(ObjectError::TypeError)
        ),
        "REMF must reject extra arguments",
    );
    Ok(())
}

#[test]
fn synthesized_cons_and_ordinal_places_expand_under_gc_stress() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    registry_with_builtins(&runtime, &mut ctx)?;
    ctx.set_strict_forwarding(true);
    ctx.set_gc_stress(true);

    let registry = PlaceRegistry::new(&runtime);
    let variable = symbol(&mut ctx, &runtime, "X")?;
    let variable = HeapRootedWord::new(&mut ctx, variable)?;
    for name in ["CDDDDR", "TENTH"] {
        let place = make_place(&mut ctx, &runtime, name, variable.get())?;
        let setf_arguments = [place.get(), Word::fixnum(41)];
        let result = expand_setf(&mut ctx, &runtime, &registry, &setf_arguments);
        assert_expansion(&mut ctx, result, name, "SETF")?;
        let incf_arguments = [place.get()];
        let result = expand_incf(&mut ctx, &runtime, &registry, &incf_arguments);
        assert_expansion(&mut ctx, result, name, "INCF")?;
    }
    Ok(())
}

#[test]
fn nested_the_is_preserved_by_setf_expansion_and_modification_operators() -> Result<(), ObjectError>
{
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    registry_with_builtins(&runtime, &mut ctx)?;
    ctx.set_gc_stress(true);

    let registry = PlaceRegistry::new(&runtime);
    let variable = symbol(&mut ctx, &runtime, "X")?;
    let variable = HeapRootedWord::new(&mut ctx, variable)?;
    let the = symbol(&mut ctx, &runtime, "THE")?;
    let the = HeapRootedWord::new(&mut ctx, the)?;
    let outer_type = symbol(&mut ctx, &runtime, "FIXNUM")?;
    let outer_type = HeapRootedWord::new(&mut ctx, outer_type)?;
    let inner_type = symbol(&mut ctx, &runtime, "INTEGER")?;
    let inner_type = HeapRootedWord::new(&mut ctx, inner_type)?;
    let inner = list(
        &mut ctx,
        &runtime,
        &[the.get(), inner_type.get(), variable.get()],
    )?;
    let inner = HeapRootedWord::new(&mut ctx, inner)?;
    let place = list(
        &mut ctx,
        &runtime,
        &[the.get(), outer_type.get(), inner.get()],
    )?;
    let place = HeapRootedWord::new(&mut ctx, place)?;

    let setf_expansion = expand_get_setf_expansion(&mut ctx, &runtime, &registry, place.get())?;
    crate::setf_support::with_expansion_roots(
        &mut ctx,
        &setf_expansion,
        |ctx, _, _, _, store, _| {
            assert!(
                contains_nested_the(
                    ctx,
                    store.get(),
                    the.get(),
                    inner_type.get(),
                    outer_type.get(),
                )?,
                "GET-SETF-EXPANSION lost the ordered THE store-value declarations",
            );
            Ok(())
        },
    )?;

    let operators = [
        "SETF", "PSETF", "INCF", "DECF", "PUSH", "PUSHNEW", "POP", "SHIFTF", "ROTATEF", "REMF",
    ];
    for operator in operators {
        let args = match operator {
            "SETF" | "PSETF" => vec![place.get(), Word::fixnum(7)],
            "INCF" | "DECF" | "POP" | "ROTATEF" => vec![place.get()],
            "PUSH" | "PUSHNEW" => vec![Word::fixnum(7), place.get()],
            "SHIFTF" => vec![place.get(), Word::fixnum(7)],
            "REMF" => vec![place.get(), Word::fixnum(7)],
            _ => unreachable!(),
        };
        let result = match operator {
            "SETF" => expand_setf(&mut ctx, &runtime, &registry, &args),
            "PSETF" => expand_psetf(&mut ctx, &runtime, &registry, &args),
            "INCF" => expand_incf(&mut ctx, &runtime, &registry, &args),
            "DECF" => expand_decf(&mut ctx, &runtime, &registry, &args),
            "PUSH" => expand_push(&mut ctx, &runtime, &registry, &args, false),
            "PUSHNEW" => expand_push(&mut ctx, &runtime, &registry, &args, true),
            "POP" => expand_pop(&mut ctx, &runtime, &registry, &args),
            "SHIFTF" => expand_shiftf(&mut ctx, &runtime, &registry, &args),
            "ROTATEF" => expand_rotatef(&mut ctx, &runtime, &registry, &args),
            "REMF" => expand_remf(&mut ctx, &runtime, &registry, &args),
            _ => unreachable!(),
        };
        let succeeded = result.is_ok();
        assert!(
            succeeded,
            "{operator} failed for nested THE place: {:?}",
            result.as_ref().err()
        );
        let Some(expansion) = result.ok() else {
            continue;
        };
        let expansion = HeapRootedWord::new(&mut ctx, expansion)?;
        assert!(
            contains_nested_the(
                &mut ctx,
                expansion.get(),
                the.get(),
                inner_type.get(),
                outer_type.get(),
            )?,
            "{operator} lost the ordered nested THE declarations",
        );
    }
    Ok(())
}
