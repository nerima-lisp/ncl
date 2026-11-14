use super::*;

#[test]
fn bounded_heap_failure_propagates_from_read_eval_form() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut opts = standard(&runtime, &mut ctx);
    opts.set_read_evaluation(ncl_reader::ReadEvaluation::Enabled);
    let mut roots = vec![opts.readtable().object().as_word()];
    roots.resize(1_024, Word::NIL);

    ncl_object::with_rooted_slice(&mut ctx, &roots, |ctx, roots| {
        let root_count = fill_heap_with_rooted_objects(ctx, &runtime, roots)?;
        assert!(root_count > 1);
        opts.set_readtable(ncl_reader::Readtable::from_object(
            ncl_readtables::Readtable::from_word(
                *roots.first().ok_or(ncl_object::ObjectError::Layout)?,
            ),
        ));

        let error = read_from_string(ctx, &runtime, "#.(1)", &opts).unwrap_err();
        assert!(matches!(
            error,
            ncl_reader::ReadError::Object(ncl_object::ObjectError::Storage(_))
        ));
        assert_eq!(format!("{error:?}"), "Object(Storage(CapacityExceeded))");
        Ok(())
    })
    .unwrap();
}

#[test]
fn bounded_heap_failure_propagates_from_read_eval_symbol_lookup() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut evaluator_sentinel = ();
    register_eval_stub(
        &mut ctx,
        &runtime,
        &mut evaluator_sentinel,
        set_read_base_to_form,
    );
    let mut opts = standard(&runtime, &mut ctx);
    opts.set_read_evaluation(ncl_reader::ReadEvaluation::Enabled);
    let mut roots = vec![opts.readtable().object().as_word()];
    roots.resize(1_024, Word::NIL);

    ncl_object::with_rooted_slice(&mut ctx, &roots, |ctx, roots| {
        let root_count = fill_heap_with_rooted_objects(ctx, &runtime, roots)?;
        assert!(root_count > 1);
        opts.set_readtable(ncl_reader::Readtable::from_object(
            ncl_readtables::Readtable::from_word(
                *roots.first().ok_or(ncl_object::ObjectError::Layout)?,
            ),
        ));

        let error = read_from_string(ctx, &runtime, "#.41", &opts).unwrap_err();
        assert!(matches!(
            error,
            ncl_reader::ReadError::Object(ncl_object::ObjectError::Storage(_))
        ));
        assert_eq!(format!("{error:?}"), "Object(Storage(CapacityExceeded))");
        Ok(())
    })
    .unwrap();
}

#[test]
fn bounded_heap_failure_propagates_from_dynamic_read_eval_lookup() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut opts = standard(&runtime, &mut ctx);
    opts.set_read_evaluation(ncl_reader::ReadEvaluation::Dynamic);
    let mut roots = vec![opts.readtable().object().as_word()];
    roots.resize(1_024, Word::NIL);

    ncl_object::with_rooted_slice(&mut ctx, &roots, |ctx, roots| {
        let root_count = fill_heap_with_rooted_objects(ctx, &runtime, roots)?;
        assert!(root_count > 1);
        opts.set_readtable(ncl_reader::Readtable::from_object(
            ncl_readtables::Readtable::from_word(
                *roots.first().ok_or(ncl_object::ObjectError::Layout)?,
            ),
        ));

        let error = read_from_string(ctx, &runtime, "#.41", &opts).unwrap_err();
        assert!(matches!(
            error,
            ncl_reader::ReadError::Object(ncl_object::ObjectError::Storage(_))
        ));
        assert_eq!(format!("{error:?}"), "Object(Storage(CapacityExceeded))");
        Ok(())
    })
    .unwrap();
}

#[test]
fn bounded_heap_failure_propagates_from_dynamic_read_suppression_lookup() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut opts = standard(&runtime, &mut ctx);
    opts.set_read_suppression(ncl_reader::ReadSuppression::Dynamic);
    let mut roots = vec![opts.readtable().object().as_word()];
    roots.resize(1_024, Word::NIL);

    ncl_object::with_rooted_slice(&mut ctx, &roots, |ctx, roots| {
        let root_count = fill_heap_with_rooted_objects(ctx, &runtime, roots)?;
        assert!(root_count > 1);
        opts.set_readtable(ncl_reader::Readtable::from_object(
            ncl_readtables::Readtable::from_word(
                *roots.first().ok_or(ncl_object::ObjectError::Layout)?,
            ),
        ));

        let error = read_from_string(ctx, &runtime, "41", &opts).unwrap_err();
        assert!(matches!(
            error,
            ncl_reader::ReadError::Object(ncl_object::ObjectError::Storage(_))
        ));
        assert_eq!(format!("{error:?}"), "Object(Storage(CapacityExceeded))");
        Ok(())
    })
    .unwrap();
}

#[test]
fn bounded_heap_failure_propagates_from_suppression_lookup_in_read_eval() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut opts = standard(&runtime, &mut ctx);
    opts.set_read_suppression(ncl_reader::ReadSuppression::Dynamic);
    let mut roots = vec![opts.readtable().object().as_word()];
    roots.resize(1_024, Word::NIL);

    ncl_object::with_rooted_slice(&mut ctx, &roots, |ctx, roots| {
        let root_count = fill_heap_with_rooted_objects(ctx, &runtime, roots)?;
        assert!(root_count > 1);
        opts.set_readtable(ncl_reader::Readtable::from_object(
            ncl_readtables::Readtable::from_word(
                *roots.first().ok_or(ncl_object::ObjectError::Layout)?,
            ),
        ));

        let error = read_from_string(ctx, &runtime, "#.41", &opts).unwrap_err();
        assert!(matches!(
            error,
            ncl_reader::ReadError::Object(ncl_object::ObjectError::Storage(_))
        ));
        assert_eq!(format!("{error:?}"), "Object(Storage(CapacityExceeded))");
        Ok(())
    })
    .unwrap();
}

#[test]
fn bounded_heap_dynamic_read_suppression_refresh_succeeds_after_heap_exhaustion() {
    let runtime = bounded_runtime();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut evaluator_sentinel = ();
    register_eval_stub(
        &mut ctx,
        &runtime,
        &mut evaluator_sentinel,
        exhaust_heap_and_return_form,
    );
    let mut opts = standard(&runtime, &mut ctx);
    opts.set_read_suppression(ncl_reader::ReadSuppression::Dynamic);
    opts.set_dynamic_read_suppression(false);

    assert!(matches!(
        read_from_string(&mut ctx, &runtime, "#.41", &opts),
        Ok(Some(value)) if value == Word::NIL
    ));
}

#[test]
fn bounded_heap_failure_propagates_from_dynamic_read_base_refresh() {
    let runtime = bounded_runtime();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut evaluator_sentinel = ();
    register_eval_stub(
        &mut ctx,
        &runtime,
        &mut evaluator_sentinel,
        exhaust_heap_and_return_form,
    );
    let mut opts = standard(&runtime, &mut ctx);
    opts.set_read_base(ncl_reader::ReadBase::new(10).unwrap().dynamic());

    let error = read_from_string(&mut ctx, &runtime, "#.41", &opts).unwrap_err();
    assert!(matches!(
        error,
        ncl_reader::ReadError::Object(ncl_object::ObjectError::Storage(_))
    ));
    assert_eq!(format!("{error:?}"), "Object(Storage(CapacityExceeded))");
}

#[test]
fn dynamic_read_base_rejects_invalid_values_after_read_eval() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut evaluator_sentinel = ();
    register_eval_stub(
        &mut ctx,
        &runtime,
        &mut evaluator_sentinel,
        set_read_base_to_form,
    );
    let mut opts = standard(&runtime, &mut ctx);
    opts.set_read_base(ncl_reader::ReadBase::new(10).unwrap().dynamic());

    for input in ["#.NIL", "#.-1", "#.1"] {
        let error = read_from_string(&mut ctx, &runtime, input, &opts).unwrap_err();
        assert!(matches!(
            error,
            ncl_reader::ReadError::Object(ncl_object::ObjectError::TypeError)
        ));
    }
}

#[test]
fn dynamic_read_base_refreshes_after_read_eval() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut evaluator_sentinel = ();
    register_eval_stub(
        &mut ctx,
        &runtime,
        &mut evaluator_sentinel,
        set_read_base_to_form,
    );
    let mut opts = standard(&runtime, &mut ctx);
    opts.set_read_base(ncl_reader::ReadBase::new(10).unwrap().dynamic());

    let value = read_from_string(&mut ctx, &runtime, "(#.16 FF)", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(classify(car(&ctx, value).unwrap()), ObjectRef::Fixnum(16));
    let second = cdr(&ctx, value).unwrap();
    assert_eq!(classify(car(&ctx, second).unwrap()), ObjectRef::Fixnum(255));
}
