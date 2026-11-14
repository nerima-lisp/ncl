use super::*;

#[test]
fn read_eval_uses_the_evaluator_when_enabled() {
    fn increment_form(
        _ctx: &mut ThreadContext,
        _runtime: &Runtime,
        args: &ncl_object::BuiltinArgs<'_>,
        _values: &mut ncl_object::MultipleValues,
    ) -> Result<Word, ncl_object::ObjectError> {
        let form = args
            .required(0)?
            .as_fixnum()
            .ok_or(ncl_object::ObjectError::TypeError)?;
        Ok(Word::fixnum(form + 1))
    }

    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let evaluator = ncl_object::Builtin {
        lambda_list: ncl_object::LambdaList::new(EVAL_PARAMETERS, &[], None, &[], false),
        convention: ncl_object::BuiltinConvention::Direct(ncl_object::Arity::exact(1)),
    };
    runtime
        .register_builtin(
            &mut ctx,
            ncl_object::BuiltinIdentifier::new(
                ncl_object::BuiltinPackage::CommonLisp,
                ncl_object::BuiltinName::new("EVAL"),
            ),
            ncl_object::BuiltinImplementation::direct(evaluator, increment_form),
        )
        .unwrap();

    // The reader checks for an active evaluator before calling the EVAL stub above.
    let mut evaluator_sentinel = ();
    ctx.set_evaluator_runtime(std::ptr::from_mut(&mut evaluator_sentinel));
    let mut opts = standard(&runtime, &mut ctx);
    opts.set_read_evaluation(ncl_reader::ReadEvaluation::Enabled);

    let value = read_from_string(&mut ctx, &runtime, "#.41", &opts).unwrap();
    assert_eq!(value, Some(Word::fixnum(42)));
}

#[test]
fn read_eval_disabled_is_rejected() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut opts = standard(&runtime, &mut ctx);
    opts.set_read_evaluation(ncl_reader::ReadEvaluation::Disabled);

    let error = read_from_string(&mut ctx, &runtime, "#.1", &opts).unwrap_err();
    assert!(matches!(error, ncl_reader::ReadError::ReadEvalDisabled));
}

#[test]
fn read_eval_is_skipped_when_read_suppressed() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut opts = standard(&runtime, &mut ctx);
    opts.set_read_suppression(ncl_reader::ReadSuppression::Discard);

    let value = read_from_string(&mut ctx, &runtime, "#.(+ 1 2)", &opts).unwrap();
    assert_eq!(value, Some(ncl_object::Word::NIL));
}

#[test]
fn labels_reference_prior_forms() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let form = read_one(&runtime, &mut ctx, "(#1=(a b) #1#)");
    let first = car(&ctx, form).unwrap();
    assert_eq!(list_names(&ctx, first), ["A", "B"]);
    let rest = cdr(&ctx, form).unwrap();
    let second = car(&ctx, rest).unwrap();
    assert_eq!(list_names(&ctx, second), ["A", "B"]);
}

#[test]
fn labels_survive_gc_stress_and_strict_forwarding() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);

    let form = read_one(&runtime, &mut ctx, "(#1=(a b c) #1#)");
    let first = car(&ctx, form).unwrap();
    assert_eq!(list_names(&ctx, first), ["A", "B", "C"]);
    let rest = cdr(&ctx, form).unwrap();
    let second = car(&ctx, rest).unwrap();
    assert_eq!(list_names(&ctx, second), ["A", "B", "C"]);
}

#[test]
fn standard_readtable_reports_upcase_and_is_a_readtable() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let table = ncl_reader::standard_readtable(&mut ctx, &runtime).unwrap();
    assert_eq!(
        ncl_reader::readtable_case(&ctx, table).unwrap(),
        ReadtableCase::Upcase
    );
    assert!(ncl_reader::readtablep(&ctx, table.object().as_word()));
    let copy = ncl_reader::copy_readtable(&mut ctx, &runtime, table).unwrap();
    assert_eq!(
        ncl_reader::readtable_case(&ctx, copy).unwrap(),
        ReadtableCase::Upcase
    );
}

#[test]
fn parse_integer_reads_digits_and_stops() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let (value, index) =
        ncl_reader::parse_integer(&mut ctx, &runtime, "42 rest", None, None, None).unwrap();
    assert_eq!(classify(value), ObjectRef::Fixnum(42));
    assert_eq!(index, 2);
    let (value, index) =
        ncl_reader::parse_integer(&mut ctx, &runtime, "ff", Some(16), None, None).unwrap();
    assert_eq!(classify(value), ObjectRef::Fixnum(255));
    assert_eq!(index, 2);
}
