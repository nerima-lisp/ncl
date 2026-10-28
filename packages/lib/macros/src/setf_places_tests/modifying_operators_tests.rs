use super::*;

#[test]
fn incf_decf_push_reject_extra_arguments() {
    let runtime = Runtime::new().unwrap_or_else(super::test_operation_failure);
    let mut ctx = ThreadContext::new();
    registry_with_builtins(&runtime, &mut ctx).unwrap_or_else(super::test_operation_failure);
    let registry = PlaceRegistry::new(&runtime);
    let place = symbol(&mut ctx, &runtime, "X").unwrap_or_else(super::test_operation_failure);

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
}

#[test]
fn pushnew_rejects_malformed_keyword_arguments() {
    let runtime = Runtime::new().unwrap_or_else(super::test_operation_failure);
    let mut ctx = ThreadContext::new();
    registry_with_builtins(&runtime, &mut ctx).unwrap_or_else(super::test_operation_failure);
    let registry = PlaceRegistry::new(&runtime);
    let place = symbol(&mut ctx, &runtime, "X").unwrap_or_else(super::test_operation_failure);
    let key =
        symbol(&mut ctx, &runtime, "KEYWORD::KEY").unwrap_or_else(super::test_operation_failure);
    let test =
        symbol(&mut ctx, &runtime, "KEYWORD::TEST").unwrap_or_else(super::test_operation_failure);
    let test_not = symbol(&mut ctx, &runtime, "KEYWORD::TEST-NOT")
        .unwrap_or_else(super::test_operation_failure);
    let unknown = symbol(&mut ctx, &runtime, "KEYWORD::UNKNOWN")
        .unwrap_or_else(super::test_operation_failure);
    let non_keyword =
        symbol(&mut ctx, &runtime, "NCL::TEST").unwrap_or_else(super::test_operation_failure);

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
    assert!(matches!(
        expand_push(
            &mut ctx,
            &runtime,
            &registry,
            &[Word::fixnum(1), place, Word::fixnum(7), Word::NIL],
            true,
        ),
        Err(ObjectError::TypeError)
    ));
    assert!(matches!(
        expand_push(
            &mut ctx,
            &runtime,
            &registry,
            &[Word::fixnum(1), place, non_keyword, Word::NIL],
            true,
        ),
        Err(ObjectError::TypeError)
    ));
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
}

#[test]
fn pop_remf_reject_extra_arguments() {
    let runtime = Runtime::new().unwrap_or_else(super::test_operation_failure);
    let mut ctx = ThreadContext::new();
    registry_with_builtins(&runtime, &mut ctx).unwrap_or_else(super::test_operation_failure);
    let registry = PlaceRegistry::new(&runtime);
    let place = symbol(&mut ctx, &runtime, "X").unwrap_or_else(super::test_operation_failure);

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
}

#[test]
fn pop_returns_the_removed_head_and_shiftf_rotatef_use_the_old_values() {
    let runtime = Runtime::new().unwrap_or_else(super::test_operation_failure);
    let mut ctx = ThreadContext::new();
    registry_with_builtins(&runtime, &mut ctx).unwrap_or_else(super::test_operation_failure);
    let registry = PlaceRegistry::new(&runtime);
    let first =
        symbol(&mut ctx, &runtime, "FIRST-PLACE").unwrap_or_else(super::test_operation_failure);
    let second =
        symbol(&mut ctx, &runtime, "SECOND-PLACE").unwrap_or_else(super::test_operation_failure);

    let pop = expand_pop(&mut ctx, &runtime, &registry, &[first])
        .unwrap_or_else(super::test_operation_failure);
    let pop_parts = elements(&mut ctx, pop).unwrap_or_else(super::test_operation_failure);
    assert_eq!(
        pop_parts[0],
        symbol(&mut ctx, &runtime, "LET*").unwrap_or_else(super::test_operation_failure)
    );
    let pop_bindings =
        elements(&mut ctx, pop_parts[1]).unwrap_or_else(super::test_operation_failure);
    let pop_body = elements(&mut ctx, pop_parts[2]).unwrap_or_else(super::test_operation_failure);
    let pop_value_variable =
        elements(&mut ctx, pop_bindings[1]).unwrap_or_else(super::test_operation_failure)[0];
    assert_eq!(pop_body.len(), 3);
    assert_eq!(
        pop_body[0],
        symbol(&mut ctx, &runtime, "PROGN").unwrap_or_else(super::test_operation_failure)
    );
    assert_eq!(pop_body[2], pop_value_variable);
    let car = symbol(&mut ctx, &runtime, "CAR").unwrap_or_else(super::test_operation_failure);
    let cdr = symbol(&mut ctx, &runtime, "CDR").unwrap_or_else(super::test_operation_failure);
    assert!(contains_operator(&mut ctx, pop, car).unwrap_or_else(super::test_operation_failure));
    assert!(contains_operator(&mut ctx, pop, cdr).unwrap_or_else(super::test_operation_failure));

    for arguments in [&[][..], &[first][..]] {
        assert!(matches!(
            expand_shiftf(&mut ctx, &runtime, &registry, arguments),
            Err(ObjectError::TypeError)
        ));
    }
    let shiftf = expand_shiftf(
        &mut ctx,
        &runtime,
        &registry,
        &[first, second, Word::fixnum(55)],
    )
    .unwrap_or_else(super::test_operation_failure);
    let shiftf_parts = elements(&mut ctx, shiftf).unwrap_or_else(super::test_operation_failure);
    let shiftf_bindings =
        elements(&mut ctx, shiftf_parts[1]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(shiftf_bindings.len(), 3);
    assert_eq!(
        elements(&mut ctx, shiftf_bindings[0]).unwrap_or_else(super::test_operation_failure)[1],
        Word::fixnum(55)
    );
    let shiftf_old_first =
        elements(&mut ctx, shiftf_bindings[1]).unwrap_or_else(super::test_operation_failure)[0];
    let shiftf_body =
        elements(&mut ctx, shiftf_parts[2]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(shiftf_body.last(), Some(&shiftf_old_first));

    let rotatef = expand_rotatef(&mut ctx, &runtime, &registry, &[first, second])
        .unwrap_or_else(super::test_operation_failure);
    let rotatef_parts = elements(&mut ctx, rotatef).unwrap_or_else(super::test_operation_failure);
    let rotatef_bindings =
        elements(&mut ctx, rotatef_parts[1]).unwrap_or_else(super::test_operation_failure);
    let rotatef_body =
        elements(&mut ctx, rotatef_parts[2]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(rotatef_bindings.len(), 2);
    assert_eq!(rotatef_body.last().copied(), Some(Word::NIL));
    let rotatef_old_second =
        elements(&mut ctx, rotatef_bindings[1]).unwrap_or_else(super::test_operation_failure)[0];
    let first_store =
        elements(&mut ctx, rotatef_body[1]).unwrap_or_else(super::test_operation_failure);
    let first_store_bindings =
        elements(&mut ctx, first_store[1]).unwrap_or_else(super::test_operation_failure);
    let first_store_binding =
        elements(&mut ctx, first_store_bindings[0]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(first_store_binding[1], rotatef_old_second);
    let first_assignment =
        elements(&mut ctx, first_store[2]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(first_assignment[1], first);
}

#[test]
fn nested_the_is_preserved_by_setf_expansion_and_modification_operators() {
    let runtime = Runtime::new().unwrap_or_else(super::test_operation_failure);
    let mut ctx = ThreadContext::new();
    registry_with_builtins(&runtime, &mut ctx).unwrap_or_else(super::test_operation_failure);
    ctx.set_gc_stress(true);

    let registry = PlaceRegistry::new(&runtime);
    let variable = symbol(&mut ctx, &runtime, "X").unwrap_or_else(super::test_operation_failure);
    let variable =
        HeapRootedWord::new(&ctx, variable).unwrap_or_else(super::test_operation_failure);
    let the = symbol(&mut ctx, &runtime, "THE").unwrap_or_else(super::test_operation_failure);
    let the = HeapRootedWord::new(&ctx, the).unwrap_or_else(super::test_operation_failure);
    let outer_type =
        symbol(&mut ctx, &runtime, "FIXNUM").unwrap_or_else(super::test_operation_failure);
    let outer_type =
        HeapRootedWord::new(&ctx, outer_type).unwrap_or_else(super::test_operation_failure);
    let inner_type =
        symbol(&mut ctx, &runtime, "INTEGER").unwrap_or_else(super::test_operation_failure);
    let inner_type =
        HeapRootedWord::new(&ctx, inner_type).unwrap_or_else(super::test_operation_failure);
    let inner = list(
        &mut ctx,
        &runtime,
        &[the.get(), inner_type.get(), variable.get()],
    )
    .unwrap_or_else(super::test_operation_failure);
    let inner = HeapRootedWord::new(&ctx, inner).unwrap_or_else(super::test_operation_failure);
    let place = list(
        &mut ctx,
        &runtime,
        &[the.get(), outer_type.get(), inner.get()],
    )
    .unwrap_or_else(super::test_operation_failure);
    let place = HeapRootedWord::new(&ctx, place).unwrap_or_else(super::test_operation_failure);

    let setf_expansion = expand_get_setf_expansion(&mut ctx, &runtime, &registry, place.get())
        .unwrap_or_else(super::test_operation_failure);
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
                )
                .unwrap_or_else(super::test_operation_failure),
                "GET-SETF-EXPANSION lost the ordered THE store-value declarations",
            );
            Ok(())
        },
    )
    .unwrap_or_else(super::test_operation_failure);

    let operators = [
        "SETF", "PSETF", "INCF", "DECF", "PUSH", "PUSHNEW", "POP", "SHIFTF", "ROTATEF", "REMF",
    ];
    for operator in operators {
        let args = match operator {
            "SETF" | "PSETF" | "SHIFTF" | "REMF" => vec![place.get(), Word::fixnum(7)],
            "INCF" | "DECF" | "POP" | "ROTATEF" => vec![place.get()],
            "PUSH" | "PUSHNEW" => vec![Word::fixnum(7), place.get()],
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
        let expansion =
            HeapRootedWord::new(&ctx, expansion).unwrap_or_else(super::test_operation_failure);
        assert!(
            contains_nested_the(
                &mut ctx,
                expansion.get(),
                the.get(),
                inner_type.get(),
                outer_type.get(),
            )
            .unwrap_or_else(super::test_operation_failure),
            "{operator} lost the ordered nested THE declarations",
        );
    }
}

#[test]
fn incf_and_decf_accept_explicit_deltas() {
    let runtime = Runtime::new().unwrap_or_else(super::test_operation_failure);
    let mut ctx = ThreadContext::new();
    registry_with_builtins(&runtime, &mut ctx).unwrap_or_else(super::test_operation_failure);
    let registry = PlaceRegistry::new(&runtime);
    let place = symbol(&mut ctx, &runtime, "X").unwrap_or_else(super::test_operation_failure);
    let delta = Word::fixnum(7);

    let increment = expand_incf(&mut ctx, &runtime, &registry, &[place, delta])
        .unwrap_or_else(super::test_operation_failure);
    let plus = symbol(&mut ctx, &runtime, "+").unwrap_or_else(super::test_operation_failure);
    assert!(
        contains_operator(&mut ctx, increment, plus,).unwrap_or_else(super::test_operation_failure)
    );
    assert!(
        contains_form(&mut ctx, increment, delta).unwrap_or_else(super::test_operation_failure)
    );

    let decrement = expand_decf(&mut ctx, &runtime, &registry, &[place, delta])
        .unwrap_or_else(super::test_operation_failure);
    let minus = symbol(&mut ctx, &runtime, "-").unwrap_or_else(super::test_operation_failure);
    assert!(
        contains_operator(&mut ctx, decrement, minus,)
            .unwrap_or_else(super::test_operation_failure)
    );
    assert!(
        contains_form(&mut ctx, decrement, delta).unwrap_or_else(super::test_operation_failure)
    );
}

#[test]
fn pushnew_accepts_key_test_and_test_not_options() {
    let runtime = Runtime::new().unwrap_or_else(super::test_operation_failure);
    let mut ctx = ThreadContext::new();
    registry_with_builtins(&runtime, &mut ctx).unwrap_or_else(super::test_operation_failure);
    let registry = PlaceRegistry::new(&runtime);
    let place = symbol(&mut ctx, &runtime, "X").unwrap_or_else(super::test_operation_failure);
    let key =
        symbol(&mut ctx, &runtime, "KEYWORD::KEY").unwrap_or_else(super::test_operation_failure);
    let test =
        symbol(&mut ctx, &runtime, "KEYWORD::TEST").unwrap_or_else(super::test_operation_failure);
    let test_not = symbol(&mut ctx, &runtime, "KEYWORD::TEST-NOT")
        .unwrap_or_else(super::test_operation_failure);
    let function =
        symbol(&mut ctx, &runtime, "IDENTITY").unwrap_or_else(super::test_operation_failure);
    let item = Word::fixnum(9);
    let adjoin = symbol(&mut ctx, &runtime, "ADJOIN").unwrap_or_else(super::test_operation_failure);

    let keyed_test = expand_push(
        &mut ctx,
        &runtime,
        &registry,
        &[item, place, key, function, test, function],
        true,
    )
    .unwrap_or_else(super::test_operation_failure);
    assert!(
        contains_operator(&mut ctx, keyed_test, adjoin,)
            .unwrap_or_else(super::test_operation_failure)
    );
    assert!(contains_form(&mut ctx, keyed_test, key).unwrap_or_else(super::test_operation_failure));
    assert!(
        contains_form(&mut ctx, keyed_test, test).unwrap_or_else(super::test_operation_failure)
    );
    let keyed_parts = elements(&mut ctx, keyed_test).unwrap_or_else(super::test_operation_failure);
    let keyed_bindings =
        elements(&mut ctx, keyed_parts[1]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(keyed_bindings.len(), 3);
    let key_binding =
        elements(&mut ctx, keyed_bindings[1]).unwrap_or_else(super::test_operation_failure);
    let test_binding =
        elements(&mut ctx, keyed_bindings[2]).unwrap_or_else(super::test_operation_failure);
    assert_eq!(key_binding[1], function);
    assert_eq!(test_binding[1], function);
    assert!(
        contains_form(&mut ctx, keyed_test, key_binding[0])
            .unwrap_or_else(super::test_operation_failure)
    );
    assert!(
        contains_form(&mut ctx, keyed_test, test_binding[0])
            .unwrap_or_else(super::test_operation_failure)
    );

    let keyed_test_not = expand_push(
        &mut ctx,
        &runtime,
        &registry,
        &[item, place, key, function, test_not, function],
        true,
    )
    .unwrap_or_else(super::test_operation_failure);
    assert!(
        contains_operator(&mut ctx, keyed_test_not, adjoin,)
            .unwrap_or_else(super::test_operation_failure)
    );
    assert!(
        contains_form(&mut ctx, keyed_test_not, key).unwrap_or_else(super::test_operation_failure)
    );
    assert!(
        contains_form(&mut ctx, keyed_test_not, test_not)
            .unwrap_or_else(super::test_operation_failure)
    );
}

#[test]
fn rotatef_without_places_returns_nil() {
    let runtime = Runtime::new().unwrap_or_else(super::test_operation_failure);
    let mut ctx = ThreadContext::new();
    let registry = PlaceRegistry::new(&runtime);

    assert!(matches!(
        expand_rotatef(&mut ctx, &runtime, &registry, &[]),
        Ok(Word::NIL)
    ));
}
