use super::*;

#[test]
fn read_eval_is_rejected() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let opts = standard(&runtime, &mut ctx);
    let error = read_from_string(&mut ctx, &runtime, "#.1", &opts).unwrap_err();
    assert!(matches!(error, ncl_reader::ReadError::ReadEvalUnavailable));
}

#[test]
fn read_eval_requires_a_form_after_the_dispatch_macro() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let opts = standard(&runtime, &mut ctx);

    let error = read_from_string(&mut ctx, &runtime, "#.", &opts).unwrap_err();
    assert!(matches!(error, ncl_reader::ReadError::UnexpectedEof));
}

#[test]
fn suppressed_read_eval_still_requires_a_form() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut opts = standard(&runtime, &mut ctx);
    opts.set_read_suppression(ncl_reader::ReadSuppression::Discard);

    let error = read_from_string(&mut ctx, &runtime, "#.", &opts).unwrap_err();
    assert!(matches!(error, ncl_reader::ReadError::UnexpectedEof));

    let value = read_from_string(&mut ctx, &runtime, "#.41", &opts).unwrap();
    assert_eq!(value, Some(Word::NIL));

    let error = read_from_string(&mut ctx, &runtime, "#.#q", &opts).unwrap_err();
    assert!(matches!(
        error,
        ncl_reader::ReadError::UndefinedDispatchMacro('q')
    ));
}

#[test]
fn dynamic_read_eval_rejects_nil() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    set_common_lisp_value(&mut ctx, &runtime, "*READ-EVAL*", Word::NIL);
    let mut opts = standard(&runtime, &mut ctx);
    opts.set_read_evaluation(ncl_reader::ReadEvaluation::Dynamic);

    let error = read_from_string(&mut ctx, &runtime, "#.#q", &opts).unwrap_err();
    assert!(matches!(error, ncl_reader::ReadError::ReadEvalDisabled));
}

#[test]
fn read_eval_propagates_errors_from_its_form() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut opts = standard(&runtime, &mut ctx);
    opts.set_read_evaluation(ncl_reader::ReadEvaluation::Enabled);

    let error = read_from_string(&mut ctx, &runtime, "#.#q", &opts).unwrap_err();
    assert!(matches!(
        error,
        ncl_reader::ReadError::UndefinedDispatchMacro('q')
    ));
}

#[test]
fn dynamic_read_eval_accepts_a_non_nil_generalized_boolean() {
    fn return_form(
        _ctx: &mut ThreadContext,
        _runtime: &Runtime,
        args: &ncl_object::BuiltinArgs<'_>,
        _values: &mut ncl_object::MultipleValues,
    ) -> Result<Word, ncl_object::ObjectError> {
        args.required(0)
    }

    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut evaluator_sentinel = ();
    register_eval_stub(&mut ctx, &runtime, &mut evaluator_sentinel, return_form);
    set_common_lisp_value(&mut ctx, &runtime, "*READ-EVAL*", Word::fixnum(0));
    let mut opts = standard(&runtime, &mut ctx);
    opts.set_read_evaluation(ncl_reader::ReadEvaluation::Dynamic);

    let value = read_from_string(&mut ctx, &runtime, "#.41", &opts).unwrap();
    assert_eq!(value, Some(Word::fixnum(41)));
}

#[test]
fn read_eval_propagates_evaluator_errors() {
    fn fail_eval(
        _ctx: &mut ThreadContext,
        _runtime: &Runtime,
        _args: &ncl_object::BuiltinArgs<'_>,
        _values: &mut ncl_object::MultipleValues,
    ) -> Result<Word, ncl_object::ObjectError> {
        Err(ncl_object::ObjectError::TypeError)
    }

    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut evaluator_sentinel = ();
    register_eval_stub(&mut ctx, &runtime, &mut evaluator_sentinel, fail_eval);
    let mut opts = standard(&runtime, &mut ctx);
    opts.set_read_evaluation(ncl_reader::ReadEvaluation::Enabled);

    let error = read_from_string(&mut ctx, &runtime, "#.41", &opts).unwrap_err();
    assert!(matches!(
        error,
        ncl_reader::ReadError::Object(ncl_object::ObjectError::TypeError)
    ));
}

#[test]
fn dynamic_read_suppression_refreshes_after_nested_read_eval() {
    fn enable_read_suppression(
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        args: &ncl_object::BuiltinArgs<'_>,
        _values: &mut ncl_object::MultipleValues,
    ) -> Result<Word, ncl_object::ObjectError> {
        let form = args.required(0)?;
        let common_lisp = runtime
            .find_package(ctx, "COMMON-LISP")
            .ok_or(ncl_object::ObjectError::Layout)?;
        let (symbol, _) =
            Package::from_word(common_lisp).intern(ctx, runtime, "*READ-SUPPRESS*")?;
        let symbol = ncl_object::HeapRootedWord::new(ctx, symbol)?;
        ncl_object::set_symbol_value(ctx, symbol.get(), Word::fixnum(1))?;
        Ok(form)
    }

    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut evaluator_sentinel = ();
    register_eval_stub(
        &mut ctx,
        &runtime,
        &mut evaluator_sentinel,
        enable_read_suppression,
    );
    set_common_lisp_value(&mut ctx, &runtime, "*READ-SUPPRESS*", Word::NIL);
    let mut opts = standard(&runtime, &mut ctx);
    opts.set_read_suppression(ncl_reader::ReadSuppression::Dynamic);

    let value = read_from_string(&mut ctx, &runtime, "#.#.41", &opts).unwrap();
    assert_eq!(value, Some(Word::NIL));
    let common_lisp = runtime.find_package(&ctx, "COMMON-LISP").unwrap();
    let (symbol, _) = Package::from_word(common_lisp)
        .intern(&mut ctx, &runtime, "*READ-SUPPRESS*")
        .unwrap();
    assert_eq!(ncl_object::symbol_value(&ctx, symbol), Ok(Word::fixnum(1)));
}

#[test]
fn dynamic_read_suppression_override_controls_reads() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut opts = standard(&runtime, &mut ctx);
    opts.set_read_suppression(ncl_reader::ReadSuppression::Dynamic);

    opts.set_dynamic_read_suppression(true);
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "41", &opts).unwrap(),
        Some(Word::NIL)
    );

    opts.set_dynamic_read_suppression(false);
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "41", &opts).unwrap(),
        Some(Word::fixnum(41))
    );
}
