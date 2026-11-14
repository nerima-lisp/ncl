use super::*;

#[test]
fn reads_common_lisp_nil_as_the_empty_list() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let common = Package::from_word(runtime.find_package(&ctx, "COMMON-LISP").unwrap());
    let (nil, status) = common.intern(&mut ctx, &runtime, "NIL").unwrap();
    assert_eq!(status, ncl_object::package::FindStatus::External);
    assert_eq!(symbol_package(&ctx, nil), Ok(common.as_word()));

    assert_eq!(read_one(&runtime, &mut ctx, "NIL"), Word::NIL);
    assert_eq!(read_one(&runtime, &mut ctx, "CL:NIL"), Word::NIL);
    assert_eq!(read_one(&runtime, &mut ctx, "COMMON-LISP:NIL"), Word::NIL);

    let keyword = read_one(&runtime, &mut ctx, ":NIL");
    assert_ne!(keyword, Word::NIL);
    assert_eq!(
        symbol_package(&ctx, keyword).unwrap(),
        runtime.find_package(&ctx, "KEYWORD").unwrap()
    );

    let uninterned = read_one(&runtime, &mut ctx, "#:NIL");
    assert_ne!(uninterned, Word::NIL);
    assert_eq!(symbol_package(&ctx, uninterned), Ok(Word::NIL));

    let shadow_package = runtime.ensure_package(&mut ctx, "SHADOW-NIL").unwrap();
    let shadow = Package::from_word(shadow_package);
    let (shadow_nil, _) = shadow.intern(&mut ctx, &runtime, "NIL").unwrap();
    let shadow_name = symbol_name(&ctx, shadow_nil).unwrap();
    shadow.shadow(&mut ctx, &runtime, shadow_name).unwrap();
    shadow
        .use_package(&mut ctx, &runtime, common.as_word())
        .unwrap();
    let mut opts = standard(&runtime, &mut ctx);
    opts.set_current_package("SHADOW-NIL").unwrap();
    let read_shadow_nil = read_from_string(&mut ctx, &runtime, "NIL", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(read_shadow_nil, shadow_nil);
    assert_ne!(read_shadow_nil, Word::NIL);
}

#[test]
fn reads_radix_integers() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    assert_eq!(
        classify(read_one(&runtime, &mut ctx, "#xff")),
        ObjectRef::Fixnum(255)
    );
    assert_eq!(
        classify(read_one(&runtime, &mut ctx, "#b101")),
        ObjectRef::Fixnum(5)
    );
    assert_eq!(
        classify(read_one(&runtime, &mut ctx, "#o17")),
        ObjectRef::Fixnum(15)
    );
    assert_eq!(
        classify(read_one(&runtime, &mut ctx, "#2r101")),
        ObjectRef::Fixnum(5)
    );
}

#[test]
fn reads_complex_literals() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let c = read_one(&runtime, &mut ctx, "#c(1 2)");
    assert!(matches!(classify_object(&ctx, c), ObjectRef::Complex(_)));
}

#[test]
fn reads_function_abbreviation() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let form = read_one(&runtime, &mut ctx, "#'car");
    let head = car(&ctx, form).unwrap();
    assert_eq!(name_of(&ctx, head), "FUNCTION");
}

#[test]
fn block_comments_are_skipped() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    assert_eq!(
        classify(read_one(&runtime, &mut ctx, "#| a comment |# 42")),
        ObjectRef::Fixnum(42)
    );
}

#[test]
fn feature_conditionals_select_forms() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    runtime.add_feature("MY-FEATURE");
    assert_eq!(
        classify(read_one(&runtime, &mut ctx, "#+my-feature 1 2")),
        ObjectRef::Fixnum(1)
    );
    assert_eq!(
        classify(read_one(&runtime, &mut ctx, "#-my-feature 1 2")),
        ObjectRef::Fixnum(2)
    );
    assert_eq!(
        classify(read_one(
            &runtime,
            &mut ctx,
            "#+ncl-reader-test-absent (ncl-reader-missing-package::value) 17"
        )),
        ObjectRef::Fixnum(17)
    );
    runtime.add_feature("NCL-READER-TEST-INNER");
    assert_eq!(
        classify(read_one(
            &runtime,
            &mut ctx,
            "#+ncl-reader-test-absent (progn #+(and ncl-reader-test-inner (not ncl-reader-test-absent)) (ncl-reader-missing-package::value) 17) 23"
        )),
        ObjectRef::Fixnum(23)
    );
    assert_eq!(
        classify(read_one(
            &runtime,
            &mut ctx,
            "#+ncl-reader-test-absent (progn (or #+ncl-reader-test-absent 500 #+ncl-reader-test-absent 3000)) 23"
        )),
        ObjectRef::Fixnum(23)
    );
}

#[test]
fn read_base_is_honoured() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut opts = standard(&runtime, &mut ctx);
    opts.set_read_base(ncl_reader::ReadBase::new(16).unwrap());
    let mut source = ncl_reader::StringSource::new("ff");
    let word = read(&mut ctx, &runtime, &mut source, &opts)
        .unwrap()
        .unwrap();
    assert_eq!(classify(word), ObjectRef::Fixnum(255));
}

#[test]
fn reads_all_float_exponent_markers() -> Result<(), String> {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();

    for token in [
        "1.31273e3",
        "1.31273s3",
        "1.31273f3",
        "1.31273d3",
        "1.31273l3",
    ] {
        let word = read_one(&runtime, &mut ctx, token);
        let ObjectRef::DoubleFloat(raw) = classify_object(&ctx, word) else {
            return Err(format!("expected a DoubleFloat object for {token}"));
        };
        let value =
            ncl_object::double_value(&ctx, ncl_object::DoubleFloat::from_word(raw)).unwrap();
        assert_eq!(value.to_bits(), 1312.73_f64.to_bits(), "{token}");
    }

    let mut opts = standard(&runtime, &mut ctx);
    opts.set_default_float_format(ncl_reader::FloatFormat::SingleFloat);
    let word = read_from_string(&mut ctx, &runtime, "1.5e0", &opts)
        .unwrap()
        .unwrap();
    let ObjectRef::DoubleFloat(raw) = classify_object(&ctx, word) else {
        return Err("expected a DoubleFloat object with the single-float default".into());
    };
    let value = ncl_object::double_value(&ctx, ncl_object::DoubleFloat::from_word(raw)).unwrap();
    assert_eq!(value.to_bits(), 1.5_f64.to_bits());
    Ok(())
}

#[test]
fn signed_float_exponent_and_missing_digits_are_handled() -> Result<(), String> {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();

    let word = read_one(&runtime, &mut ctx, "1.0s-2");
    let ObjectRef::DoubleFloat(raw) = classify_object(&ctx, word) else {
        return Err("expected a DoubleFloat object for a signed exponent".into());
    };
    let value = ncl_object::double_value(&ctx, ncl_object::DoubleFloat::from_word(raw)).unwrap();
    assert_eq!(value.to_bits(), 0.01_f64.to_bits());

    let opts = standard(&runtime, &mut ctx);
    let error = read_from_string(&mut ctx, &runtime, "1.0s", &opts).unwrap_err();
    assert!(matches!(error, ncl_reader::ReadError::InvalidNumber(_)));
    Ok(())
}

#[test]
fn unmatched_paren_is_an_error() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let opts = standard(&runtime, &mut ctx);
    let error = read_from_string(&mut ctx, &runtime, ")", &opts).unwrap_err();
    assert!(matches!(error, ncl_reader::ReadError::UnmatchedRightParen));
}

#[test]
fn malformed_dotted_lists_report_missing_cdr_and_trailing_forms() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let opts = standard(&runtime, &mut ctx);

    let error = read_from_string(&mut ctx, &runtime, "(. item)", &opts).unwrap_err();
    assert!(matches!(error, ncl_reader::ReadError::DotWithoutCdr));

    let error = read_from_string(&mut ctx, &runtime, "(item . #|ignored|#)", &opts).unwrap_err();
    assert!(matches!(error, ncl_reader::ReadError::DotWithoutCdr));

    let error = read_from_string(&mut ctx, &runtime, "(item . tail extra)", &opts).unwrap_err();
    assert!(matches!(error, ncl_reader::ReadError::UnmatchedRightParen));
}

#[test]
fn lists_propagate_reader_errors_from_elements_and_dotted_cdrs() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let opts = standard(&runtime, &mut ctx);

    let error = read_from_string(&mut ctx, &runtime, "(#q)", &opts).unwrap_err();
    assert!(matches!(
        error,
        ncl_reader::ReadError::UndefinedDispatchMacro('q')
    ));

    let error = read_from_string(&mut ctx, &runtime, "(item . #q)", &opts).unwrap_err();
    assert!(matches!(
        error,
        ncl_reader::ReadError::UndefinedDispatchMacro('q')
    ));
}

#[test]
fn dot_followed_by_a_constituent_is_read_as_a_token() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();

    assert_eq!(read_names(&runtime, &mut ctx, "(.item)"), vec![".ITEM"]);
}

#[test]
fn unterminated_list_reports_eof() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let opts = standard(&runtime, &mut ctx);

    let error = read_from_string(&mut ctx, &runtime, "(item", &opts).unwrap_err();
    assert!(matches!(error, ncl_reader::ReadError::UnexpectedEof));
}

#[test]
fn eof_in_string_is_an_error() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let opts = standard(&runtime, &mut ctx);
    let error = read_from_string(&mut ctx, &runtime, "\"unterminated", &opts).unwrap_err();
    assert!(matches!(error, ncl_reader::ReadError::UnexpectedEof));
}

#[test]
fn undefined_dispatch_is_an_error() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let opts = standard(&runtime, &mut ctx);
    let error = read_from_string(&mut ctx, &runtime, "#q", &opts).unwrap_err();
    assert!(matches!(
        error,
        ncl_reader::ReadError::UndefinedDispatchMacro('q')
    ));
}
