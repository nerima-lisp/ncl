use super::*;

/// Each generalized place must expand into a non-empty form that survives a
/// full collection under `gc_stress` + strict forwarding.
#[test]
fn generalized_places_survive_gc_stress_and_strict_forwarding() {
    let runtime = Runtime::new().unwrap_or_else(super::test_operation_failure);
    let mut ctx = ThreadContext::new();
    registry_with_builtins(&runtime, &mut ctx).unwrap_or_else(super::test_operation_failure);
    ctx.set_strict_forwarding(true);

    let registry = PlaceRegistry::new(&runtime);
    let list_symbol = symbol(&mut ctx, &runtime, "L").unwrap_or_else(super::test_operation_failure);
    let var_symbol = symbol(&mut ctx, &runtime, "X").unwrap_or_else(super::test_operation_failure);

    let mut cases = Vec::new();
    macro_rules! place {
        ($operator:literal, [$($arg:expr),* $(,)?]) => {{
            let operator = symbol(&mut ctx, &runtime, $operator).unwrap_or_else(super::test_operation_failure);
            let args = [$($arg),*];
            let mut values = Vec::with_capacity(args.len() + 1);
            values.push(operator);
            values.extend_from_slice(&args);
            cases.push(list(&mut ctx, &runtime, &values).unwrap_or_else(super::test_operation_failure));
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
            let mut expanded = expand_setf(ctx, &runtime, &registry, &[**place_root, value])
                .unwrap_or_else(super::test_operation_failure);
            // Only aarch64 observes this relocation: an object found by
            // conservative scanning is pinned and skipped by `move_live_objects`,
            // and on x86-64 `heap_collect` refreshes that snapshot (callee-saved
            // registers plus the whole stack) at the collection site, so a live
            // local keeps its address. aarch64 keeps the older snapshot, sees the
            // value only through the precise `push_root` slot, and forwards it.
            #[cfg(target_arch = "aarch64")]
            let before = expanded;
            let root = ncl_object::push_root(ctx, &mut expanded);
            ctx.collect(true)
                .unwrap_or_else(super::test_operation_failure);
            // check-added-lines: allow(panic) test-only assertion
            #[cfg(target_arch = "aarch64")]
            assert_ne!(expanded, before);
            // check-added-lines: allow(panic) test-only assertion
            assert!(
                !elements(ctx, expanded)
                    .unwrap_or_else(super::test_operation_failure)
                    .is_empty()
            );
            // check-added-lines: allow(panic) test-only assertion
            assert!(ncl_object::pop_root(ctx, root));
        }
        Ok(())
    })
    .unwrap_or_else(super::test_operation_failure);
}

#[test]
fn nth_and_elt_places_build_their_store_paths_and_validate_arity() {
    let runtime = Runtime::new().unwrap_or_else(super::test_operation_failure);
    let mut ctx = ThreadContext::new();
    registry_with_builtins(&runtime, &mut ctx).unwrap_or_else(super::test_operation_failure);
    let registry = PlaceRegistry::new(&runtime);
    let sequence =
        symbol(&mut ctx, &runtime, "SEQUENCE").unwrap_or_else(super::test_operation_failure);
    let index = symbol(&mut ctx, &runtime, "INDEX").unwrap_or_else(super::test_operation_failure);
    let value = symbol(&mut ctx, &runtime, "VALUE").unwrap_or_else(super::test_operation_failure);

    let nth = place_form(&mut ctx, &runtime, "NTH", &[index, sequence])
        .unwrap_or_else(super::test_operation_failure);
    let nth_expansion = expand_get_setf_expansion(&mut ctx, &runtime, &registry, nth)
        .unwrap_or_else(super::test_operation_failure);
    assert_eq!(nth_expansion.temporary_variables.len(), 2);
    assert_eq!(nth_expansion.value_forms, vec![index, sequence]);
    let nthcdr = symbol(&mut ctx, &runtime, "NTHCDR").unwrap_or_else(super::test_operation_failure);
    let rplaca = symbol(&mut ctx, &runtime, "RPLACA").unwrap_or_else(super::test_operation_failure);
    assert!(
        contains_operator(&mut ctx, nth_expansion.store_form, nthcdr)
            .unwrap_or_else(super::test_operation_failure)
    );
    assert!(
        contains_operator(&mut ctx, nth_expansion.store_form, rplaca)
            .unwrap_or_else(super::test_operation_failure)
    );

    let elt = place_form(&mut ctx, &runtime, "ELT", &[sequence, index])
        .unwrap_or_else(super::test_operation_failure);
    let elt_expansion = expand_get_setf_expansion(&mut ctx, &runtime, &registry, elt)
        .unwrap_or_else(super::test_operation_failure);
    assert_eq!(elt_expansion.temporary_variables.len(), 2);
    assert_eq!(elt_expansion.value_forms, vec![sequence, index]);
    let sequence_temp = elt_expansion.temporary_variables[0];
    let index_temp = elt_expansion.temporary_variables[1];
    let store = elt_expansion.store_variables[0];
    let store_parts =
        elements(&mut ctx, elt_expansion.store_form).unwrap_or_else(super::test_operation_failure);
    let progn = symbol(&mut ctx, &runtime, "PROGN").unwrap_or_else(super::test_operation_failure);
    assert_eq!(store_parts.len(), 3);
    assert_eq!(store_parts[0], progn);
    assert_eq!(store_parts[1], elt_expansion.access_form);
    let branch_parts =
        elements(&mut ctx, store_parts[2]).unwrap_or_else(super::test_operation_failure);
    let if_symbol = symbol(&mut ctx, &runtime, "IF").unwrap_or_else(super::test_operation_failure);
    assert_eq!(branch_parts.len(), 4);
    assert_eq!(branch_parts[0], if_symbol);
    assert_eq!(
        elements(&mut ctx, branch_parts[1]).unwrap_or_else(super::test_operation_failure),
        vec![
            symbol(&mut ctx, &runtime, "LISTP").unwrap_or_else(super::test_operation_failure),
            sequence_temp
        ]
    );
    let list_store =
        elements(&mut ctx, branch_parts[2]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(list_store.len(), 3);
    assert_eq!(list_store[0], progn);
    let rplaca_parts =
        elements(&mut ctx, list_store[1]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(rplaca_parts.len(), 3);
    assert_eq!(
        rplaca_parts[0],
        symbol(&mut ctx, &runtime, "RPLACA").unwrap_or_else(super::test_operation_failure)
    );
    let nthcdr_parts =
        elements(&mut ctx, rplaca_parts[1]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(
        nthcdr_parts,
        vec![
            symbol(&mut ctx, &runtime, "NTHCDR").unwrap_or_else(super::test_operation_failure),
            index_temp,
            sequence_temp,
        ]
    );
    assert_eq!(rplaca_parts[2], store);
    assert_eq!(list_store[2], store);
    assert_eq!(
        elements(&mut ctx, branch_parts[3]).unwrap_or_else(super::test_operation_failure),
        vec![
            symbol(&mut ctx, &runtime, "NCL-EXT::AREF-SET")
                .unwrap_or_else(super::test_operation_failure),
            sequence_temp,
            index_temp,
            store,
        ]
    );

    let malformed_nth = place_form(&mut ctx, &runtime, "NTH", &[sequence])
        .unwrap_or_else(super::test_operation_failure);
    assert!(matches!(
        expand_get_setf_expansion(&mut ctx, &runtime, &registry, malformed_nth),
        Err(ObjectError::TypeError)
    ));
    let malformed_elt = place_form(&mut ctx, &runtime, "ELT", &[sequence, index, value])
        .unwrap_or_else(super::test_operation_failure);
    assert!(matches!(
        expand_get_setf_expansion(&mut ctx, &runtime, &registry, malformed_elt),
        Err(ObjectError::TypeError)
    ));
}

#[test]
fn svref_and_mutator_places_build_stores_and_validate_arities() {
    let runtime = Runtime::new().unwrap_or_else(super::test_operation_failure);
    let mut ctx = ThreadContext::new();
    registry_with_builtins(&runtime, &mut ctx).unwrap_or_else(super::test_operation_failure);
    let registry = PlaceRegistry::new(&runtime);
    let sequence =
        symbol(&mut ctx, &runtime, "SEQUENCE").unwrap_or_else(super::test_operation_failure);
    let index = symbol(&mut ctx, &runtime, "INDEX").unwrap_or_else(super::test_operation_failure);

    let svref = place_form(&mut ctx, &runtime, "SVREF", &[sequence, index])
        .unwrap_or_else(super::test_operation_failure);
    let svref_expansion = expand_get_setf_expansion(&mut ctx, &runtime, &registry, svref)
        .unwrap_or_else(super::test_operation_failure);
    assert_eq!(svref_expansion.temporary_variables.len(), 2);
    assert_eq!(svref_expansion.value_forms, vec![sequence, index]);
    assert_eq!(
        elements(&mut ctx, svref_expansion.access_form)
            .unwrap_or_else(super::test_operation_failure),
        vec![
            symbol(&mut ctx, &runtime, "SVREF").unwrap_or_else(super::test_operation_failure),
            svref_expansion.temporary_variables[0],
            svref_expansion.temporary_variables[1],
        ]
    );
    assert_eq!(
        elements(&mut ctx, svref_expansion.store_form)
            .unwrap_or_else(super::test_operation_failure),
        vec![
            symbol(&mut ctx, &runtime, "NCL-EXT::SVREF-SET")
                .unwrap_or_else(super::test_operation_failure),
            svref_expansion.temporary_variables[0],
            svref_expansion.temporary_variables[1],
            svref_expansion.store_variables[0],
        ]
    );
    let missing_svref_arguments =
        place_form(&mut ctx, &runtime, "SVREF", &[]).unwrap_or_else(super::test_operation_failure);
    assert!(matches!(
        expand_get_setf_expansion(&mut ctx, &runtime, &registry, missing_svref_arguments),
        Err(ObjectError::TypeError)
    ));

    let car = place_form(&mut ctx, &runtime, "CAR", &[sequence])
        .unwrap_or_else(super::test_operation_failure);
    let car_expansion = expand_get_setf_expansion(&mut ctx, &runtime, &registry, car)
        .unwrap_or_else(super::test_operation_failure);
    assert_eq!(car_expansion.temporary_variables.len(), 1);
    assert_eq!(car_expansion.value_forms, vec![sequence]);
    let car_store =
        elements(&mut ctx, car_expansion.store_form).unwrap_or_else(super::test_operation_failure);
    assert_eq!(car_store.len(), 3);
    assert_eq!(
        car_store[0],
        symbol(&mut ctx, &runtime, "PROGN").unwrap_or_else(super::test_operation_failure)
    );
    assert_eq!(
        elements(&mut ctx, car_store[1]).unwrap_or_else(super::test_operation_failure),
        vec![
            symbol(&mut ctx, &runtime, "RPLACA").unwrap_or_else(super::test_operation_failure),
            car_expansion.temporary_variables[0],
            car_expansion.store_variables[0],
        ]
    );
    assert_eq!(car_store[2], car_expansion.store_variables[0]);
    for arguments in [&[][..], &[sequence, index][..]] {
        let malformed_car = place_form(&mut ctx, &runtime, "CAR", arguments)
            .unwrap_or_else(super::test_operation_failure);
        assert!(matches!(
            expand_get_setf_expansion(&mut ctx, &runtime, &registry, malformed_car),
            Err(ObjectError::TypeError)
        ));
    }
}

#[test]
fn standardized_cons_helper_returns_none_for_other_accessors() {
    let runtime = Runtime::new().unwrap_or_else(super::test_operation_failure);
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(super::test_operation_failure);
    let unrecognized =
        symbol(&mut ctx, &runtime, "CAXR").unwrap_or_else(super::test_operation_failure);
    assert!(matches!(
        super::standard_cons_place(&mut ctx, &runtime, unrecognized, &[]),
        Ok(None)
    ));
    assert!(matches!(
        super::standard_cons_place(&mut ctx, &runtime, Word::fixnum(25), &[]),
        Ok(None)
    ));

    let foreign_second = symbol(&mut ctx, &runtime, "N25-FOREIGN::SECOND")
        .unwrap_or_else(super::test_operation_failure);
    assert!(matches!(
        super::standard_cons_place(&mut ctx, &runtime, foreign_second, &[]),
        Ok(None)
    ));
    let foreign_caar = symbol(&mut ctx, &runtime, "N25-FOREIGN::CAAR")
        .unwrap_or_else(super::test_operation_failure);
    assert!(matches!(
        super::standard_cons_place(&mut ctx, &runtime, foreign_caar, &[Word::NIL]),
        Ok(None)
    ));

    let caar = symbol(&mut ctx, &runtime, "CAAR").unwrap_or_else(super::test_operation_failure);
    let sequence =
        symbol(&mut ctx, &runtime, "SEQUENCE").unwrap_or_else(super::test_operation_failure);
    let index = symbol(&mut ctx, &runtime, "INDEX").unwrap_or_else(super::test_operation_failure);
    assert!(matches!(
        super::standard_cons_place(&mut ctx, &runtime, caar, &[sequence, index]),
        Err(ObjectError::TypeError)
    ));
}

#[test]
fn standardized_cons_places_expand_for_setf_and_incf() {
    let runtime = Runtime::new().unwrap_or_else(super::test_operation_failure);
    let mut ctx = ThreadContext::new();
    registry_with_builtins(&runtime, &mut ctx).unwrap_or_else(super::test_operation_failure);
    ctx.set_strict_forwarding(true);

    let registry = PlaceRegistry::new(&runtime);
    let variable = symbol(&mut ctx, &runtime, "X").unwrap_or_else(super::test_operation_failure);
    let variable =
        HeapRootedWord::new(&ctx, variable).unwrap_or_else(super::test_operation_failure);
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
        let place = make_place(&mut ctx, &runtime, &name, variable.get())
            .unwrap_or_else(super::test_operation_failure);
        cases.push((name, place));
    }
    let documentation =
        symbol(&mut ctx, &runtime, "DOCUMENTATION").unwrap_or_else(super::test_operation_failure);
    let documentation =
        HeapRootedWord::new(&ctx, documentation).unwrap_or_else(super::test_operation_failure);
    let kind = symbol(&mut ctx, &runtime, "T").unwrap_or_else(super::test_operation_failure);
    let kind = HeapRootedWord::new(&ctx, kind).unwrap_or_else(super::test_operation_failure);
    let package = symbol(&mut ctx, &runtime, "P").unwrap_or_else(super::test_operation_failure);
    let package = HeapRootedWord::new(&ctx, package).unwrap_or_else(super::test_operation_failure);
    let documentation_place = list(
        &mut ctx,
        &runtime,
        &[documentation.get(), package.get(), kind.get()],
    )
    .unwrap_or_else(super::test_operation_failure);
    cases.push((
        "(DOCUMENTATION P T)".to_owned(),
        HeapRootedWord::new(&ctx, documentation_place)
            .unwrap_or_else(super::test_operation_failure),
    ));

    for (name, place) in cases {
        let setf_arguments = [place.get(), Word::fixnum(41)];
        let result = expand_setf(&mut ctx, &runtime, &registry, &setf_arguments);
        assert_expansion(&mut ctx, result, &name, "SETF")
            .unwrap_or_else(super::test_operation_failure);
        let incf_arguments = [place.get()];
        let result = expand_incf(&mut ctx, &runtime, &registry, &incf_arguments);
        assert_expansion(&mut ctx, result, &name, "INCF")
            .unwrap_or_else(super::test_operation_failure);
    }

    let second = symbol(&mut ctx, &runtime, "SECOND").unwrap_or_else(super::test_operation_failure);
    let second = HeapRootedWord::new(&ctx, second).unwrap_or_else(super::test_operation_failure);
    let malformed_place = list(
        &mut ctx,
        &runtime,
        &[second.get(), variable.get(), Word::fixnum(0)],
    )
    .unwrap_or_else(super::test_operation_failure);
    let malformed_place =
        HeapRootedWord::new(&ctx, malformed_place).unwrap_or_else(super::test_operation_failure);
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

    let documentation =
        symbol(&mut ctx, &runtime, "DOCUMENTATION").unwrap_or_else(super::test_operation_failure);
    let malformed_documentation = list(&mut ctx, &runtime, &[documentation, variable.get()])
        .unwrap_or_else(super::test_operation_failure);
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
}

#[test]
fn synthesized_cons_and_ordinal_places_expand_under_gc_stress() {
    let runtime = Runtime::new().unwrap_or_else(super::test_operation_failure);
    let mut ctx = ThreadContext::new();
    registry_with_builtins(&runtime, &mut ctx).unwrap_or_else(super::test_operation_failure);
    ctx.set_strict_forwarding(true);
    ctx.set_gc_stress(true);

    let registry = PlaceRegistry::new(&runtime);
    let variable = symbol(&mut ctx, &runtime, "X").unwrap_or_else(super::test_operation_failure);
    let variable =
        HeapRootedWord::new(&ctx, variable).unwrap_or_else(super::test_operation_failure);
    for name in ["CDDDDR", "TENTH"] {
        let place = make_place(&mut ctx, &runtime, name, variable.get())
            .unwrap_or_else(super::test_operation_failure);
        let setf_arguments = [place.get(), Word::fixnum(41)];
        let result = expand_setf(&mut ctx, &runtime, &registry, &setf_arguments);
        assert_expansion(&mut ctx, result, name, "SETF")
            .unwrap_or_else(super::test_operation_failure);
        let incf_arguments = [place.get()];
        let result = expand_incf(&mut ctx, &runtime, &registry, &incf_arguments);
        assert_expansion(&mut ctx, result, name, "INCF")
            .unwrap_or_else(super::test_operation_failure);
    }
}

#[test]
fn subseq_without_end_builds_a_start_only_replace_store() {
    let runtime = Runtime::new().unwrap_or_else(super::test_operation_failure);
    let mut ctx = ThreadContext::new();
    registry_with_builtins(&runtime, &mut ctx).unwrap_or_else(super::test_operation_failure);
    let registry = PlaceRegistry::new(&runtime);
    let sequence =
        symbol(&mut ctx, &runtime, "SEQUENCE").unwrap_or_else(super::test_operation_failure);
    let start = symbol(&mut ctx, &runtime, "START").unwrap_or_else(super::test_operation_failure);
    let place = place_form(&mut ctx, &runtime, "SUBSEQ", &[sequence, start])
        .unwrap_or_else(super::test_operation_failure);

    let expansion = expand_get_setf_expansion(&mut ctx, &runtime, &registry, place)
        .unwrap_or_else(super::test_operation_failure);
    assert_eq!(expansion.temporary_variables.len(), 2);
    assert_eq!(expansion.value_forms, vec![sequence, start]);

    let store_parts =
        elements(&mut ctx, expansion.store_form).unwrap_or_else(super::test_operation_failure);
    assert_eq!(store_parts.len(), 3);
    assert_eq!(
        store_parts[0],
        symbol(&mut ctx, &runtime, "PROGN").unwrap_or_else(super::test_operation_failure)
    );
    assert_eq!(store_parts[2], expansion.store_variables[0]);

    let replace_parts =
        elements(&mut ctx, store_parts[1]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(replace_parts.len(), 5);
    assert_eq!(
        replace_parts[0],
        symbol(&mut ctx, &runtime, "REPLACE").unwrap_or_else(super::test_operation_failure)
    );
    assert_eq!(replace_parts[1], expansion.temporary_variables[0]);
    assert_eq!(replace_parts[2], expansion.store_variables[0]);
    assert_eq!(
        replace_parts[3],
        symbol(&mut ctx, &runtime, "KEYWORD::START").unwrap_or_else(super::test_operation_failure)
    );
    assert_eq!(replace_parts[4], expansion.temporary_variables[1]);

    for arguments in [
        &[sequence][..],
        &[sequence, start, Word::fixnum(1), Word::NIL][..],
    ] {
        let malformed = place_form(&mut ctx, &runtime, "SUBSEQ", arguments)
            .unwrap_or_else(super::test_operation_failure);
        assert!(matches!(
            expand_get_setf_expansion(&mut ctx, &runtime, &registry, malformed),
            Err(ObjectError::TypeError)
        ));
    }
}
