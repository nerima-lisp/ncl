#![allow(clippy::unwrap_used, reason = "tests assert on reader behavior")]

//! Reader acceptance tests: macro characters, dispatch characters, numbers,
//! readtable case, package prefixes, errors, and feature evaluation.

use ncl_object::{
    Bignum, ObjectRef, Package, Ratio, Runtime, ThreadContext, Word, bignum_limbs, car, cdr,
    classify, classify_object, make_cons, make_string, ratio_denominator, string_length,
    string_ref, symbol_name, symbol_package,
};
use ncl_reader::{ReadOptions, ReadtableCase, read, read_from_string};

const EVAL_PARAMETERS: &[ncl_object::Parameter] = &[ncl_object::Parameter {
    name: ncl_object::BuiltinName::new("FORM"),
    ty: ncl_object::ParameterType::Any,
}];

fn standard(runtime: &Runtime, ctx: &mut ThreadContext) -> ReadOptions {
    ReadOptions::standard(ctx, runtime).unwrap()
}

fn read_one(runtime: &Runtime, ctx: &mut ThreadContext, text: &str) -> Word {
    let opts = standard(runtime, ctx);
    read_from_string(ctx, runtime, text, &opts)
        .unwrap()
        .unwrap()
}

fn name_of(ctx: &ThreadContext, word: Word) -> String {
    let name = symbol_name(ctx, word).unwrap();
    let length = string_length(ctx, name).unwrap();
    (0..length)
        .map(|i| string_ref(ctx, name, i).unwrap())
        .collect()
}

fn read_name(runtime: &Runtime, ctx: &mut ThreadContext, text: &str) -> String {
    let word = read_one(runtime, ctx, text);
    name_of(ctx, word)
}

fn list_names(ctx: &ThreadContext, list: Word) -> Vec<String> {
    let mut names = Vec::new();
    let mut cursor = list;
    while cursor != Word::NIL {
        let item = car(ctx, cursor).unwrap();
        names.push(name_of(ctx, item));
        cursor = cdr(ctx, cursor).unwrap();
    }
    names
}

fn read_names(runtime: &Runtime, ctx: &mut ThreadContext, text: &str) -> Vec<String> {
    let list = read_one(runtime, ctx, text);
    list_names(ctx, list)
}

#[test]
fn reads_fixnums() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    assert_eq!(
        classify(read_one(&runtime, &mut ctx, "42")),
        ObjectRef::Fixnum(42)
    );
    assert_eq!(
        classify(read_one(&runtime, &mut ctx, "-7")),
        ObjectRef::Fixnum(-7)
    );
    assert_eq!(
        classify(read_one(&runtime, &mut ctx, "+3")),
        ObjectRef::Fixnum(3)
    );
}

#[test]
fn reads_a_bignum_beyond_fixnum() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let big = read_one(&runtime, &mut ctx, "4611686018427387904");
    assert!(matches!(classify_object(&ctx, big), ObjectRef::Bignum(_)));
}

#[test]
fn reads_bignums_and_ratios_beyond_i128() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let integer = read_one(
        &runtime,
        &mut ctx,
        "1234678123487612347896123467851234671234",
    );
    assert!(matches!(
        classify_object(&ctx, integer),
        ObjectRef::Bignum(_)
    ));
    assert_eq!(
        bignum_limbs(&ctx, Bignum::from_word(integer)).unwrap(),
        [
            3_806_722_690,
            1_768_204_404,
            1_787_948_446,
            2_698_926_946,
            3
        ],
    );

    let ratio = read_one(
        &runtime,
        &mut ctx,
        "189729874978126783786123/1234678123487612347896123467851234671234",
    );
    assert!(matches!(classify_object(&ctx, ratio), ObjectRef::Ratio(_)));
    let denominator = ratio_denominator(&ctx, Ratio::from_word(ratio)).unwrap();
    assert!(matches!(
        classify_object(&ctx, denominator),
        ObjectRef::Bignum(_)
    ));
}

#[test]
fn reads_ratios_and_floats() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    assert_eq!(
        standard(&runtime, &mut ctx).default_float_format(),
        ncl_reader::FloatFormat::SingleFloat
    );
    let ratio = read_one(&runtime, &mut ctx, "3/4");
    assert!(matches!(classify_object(&ctx, ratio), ObjectRef::Ratio(_)));
    let float = read_one(&runtime, &mut ctx, "3.14");
    assert!(matches!(
        classify_object(&ctx, float),
        ObjectRef::DoubleFloat(_)
    ));
    let exp = read_one(&runtime, &mut ctx, "1.5e3");
    assert!(matches!(
        classify_object(&ctx, exp),
        ObjectRef::DoubleFloat(_)
    ));
}

#[test]
fn exponent_marker_names_are_symbols_without_a_mantissa() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    for token in ["e1", "s1", "f1", "d1", "l1"] {
        assert_eq!(read_name(&runtime, &mut ctx, token), token.to_uppercase());
    }
    let opts = standard(&runtime, &mut ctx);
    let error = read_from_string(&mut ctx, &runtime, "+e1", &opts).unwrap_err();
    assert!(matches!(error, ncl_reader::ReadError::InvalidNumber(_)));
}

#[test]
fn readtable_case_upcases_symbols_by_default() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    assert_eq!(read_name(&runtime, &mut ctx, "hello"), "HELLO");
    assert_eq!(read_name(&runtime, &mut ctx, "HiThere"), "HITHERE");
}

#[test]
fn escapes_and_bars_preserve_case() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    assert_eq!(read_name(&runtime, &mut ctx, "|Hello|"), "Hello");
    assert_eq!(read_name(&runtime, &mut ctx, "\\H\\ello"), "HeLLO");
}

#[test]
fn downcase_readtable_case_folds_symbols() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let table = ncl_reader::standard_readtable(&mut ctx, &runtime).unwrap();
    ctx.write_object_slot(
        table.object().as_word(),
        ncl_object::readtable_offset::CASE,
        Word::fixnum(1),
    )
    .unwrap();
    let mut opts = standard(&runtime, &mut ctx);
    opts.set_readtable(table);
    let mut source = ncl_reader::StringSource::new("HELLO");
    let word = read(&mut ctx, &runtime, &mut source, &opts)
        .unwrap()
        .unwrap();
    assert_eq!(name_of(&ctx, word), "hello");
}

#[test]
fn reads_keyword_symbols() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let kw = read_one(&runtime, &mut ctx, ":foo");
    assert_eq!(name_of(&ctx, kw), "FOO");
    let pkg = ncl_object::symbol_package(&ctx, kw).unwrap();
    let pkg_name = ncl_object::Package::from_word(pkg).name(&ctx).unwrap();
    let len = string_length(&ctx, pkg_name).unwrap();
    let pkg_name: String = (0..len)
        .map(|i| string_ref(&ctx, pkg_name, i).unwrap())
        .collect();
    assert_eq!(pkg_name, "KEYWORD");

    let empty = read_one(&runtime, &mut ctx, ":||");
    assert_eq!(name_of(&ctx, empty), "");
    assert_eq!(
        symbol_package(&ctx, empty).unwrap(),
        runtime.find_package(&ctx, "KEYWORD").unwrap()
    );

    let empty_external = read_one(&runtime, &mut ctx, "COMMON-LISP-USER:||");
    assert_eq!(name_of(&ctx, empty_external), "");
    assert_eq!(
        symbol_package(&ctx, empty_external).unwrap(),
        runtime.find_package(&ctx, "COMMON-LISP-USER").unwrap()
    );
}

#[test]
fn reads_an_empty_symbol_name() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let empty = read_one(&runtime, &mut ctx, "||");
    assert_eq!(name_of(&ctx, empty), "");
    let package = read_one(&runtime, &mut ctx, "COMMON-LISP-USER||:");
    assert_eq!(
        package,
        runtime.find_package(&ctx, "COMMON-LISP-USER").unwrap()
    );
    assert_eq!(
        classify(read_one(&runtime, &mut ctx, "||123")),
        ObjectRef::Fixnum(123)
    );
}

#[test]
fn current_package_local_nickname_resolves_before_global_lookup() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut current = runtime.ensure_package(&mut ctx, "CURRENT").unwrap();
    let current_token = ncl_object::push_root(&mut ctx, &mut current);
    let mut target = runtime.ensure_package(&mut ctx, "TARGET").unwrap();
    let target_token = ncl_object::push_root(&mut ctx, &mut target);
    let mut global = runtime.ensure_package(&mut ctx, "GLOBAL").unwrap();
    let global_token = ncl_object::push_root(&mut ctx, &mut global);
    let nickname = make_string(&mut ctx, &runtime, &['L', 'O', 'C', 'A', 'L']).unwrap();
    let entry = make_cons(&mut ctx, &runtime, nickname, target).unwrap();
    let local_nicknames = make_cons(&mut ctx, &runtime, entry, Word::NIL).unwrap();
    Package::from_word(current)
        .set_local_nicknames(&mut ctx, local_nicknames)
        .unwrap();
    let mut opts = standard(&runtime, &mut ctx);
    opts.set_current_package("CURRENT").unwrap();
    assert_eq!(runtime.find_package(&ctx, "GLOBAL"), Some(global));

    let symbol = read_from_string(&mut ctx, &runtime, "LOCAL:ITEM", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_package(&ctx, symbol).unwrap(), target);
    assert_eq!(name_of(&ctx, symbol), "ITEM");

    let symbol = read_from_string(&mut ctx, &runtime, "GLOBAL:ITEM", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_package(&ctx, symbol).unwrap(), global);
    assert!(ncl_object::pop_root(&mut ctx, global_token));
    assert!(ncl_object::pop_root(&mut ctx, target_token));
    assert!(ncl_object::pop_root(&mut ctx, current_token));
}

#[test]
fn reads_quote_and_quasiquote() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let quoted = read_one(&runtime, &mut ctx, "'x");
    let head = car(&ctx, quoted).unwrap();
    assert_eq!(name_of(&ctx, head), "QUOTE");
    let rest = cdr(&ctx, quoted).unwrap();
    let inner = car(&ctx, rest).unwrap();
    assert_eq!(name_of(&ctx, inner), "X");
}

#[test]
fn reads_lists_and_dotted_lists() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    assert_eq!(read_names(&runtime, &mut ctx, "(a b c)"), ["A", "B", "C"]);
    let dotted = read_one(&runtime, &mut ctx, "(a . b)");
    let head = car(&ctx, dotted).unwrap();
    assert_eq!(name_of(&ctx, head), "A");
    let tail = cdr(&ctx, dotted).unwrap();
    assert_eq!(name_of(&ctx, tail), "B");
}

#[test]
fn reads_strings_with_escapes() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let s = read_one(&runtime, &mut ctx, "\"he\\\"llo\"");
    assert!(matches!(classify_object(&ctx, s), ObjectRef::String(_)));
    let len = string_length(&ctx, s).unwrap();
    let text: String = (0..len).map(|i| string_ref(&ctx, s, i).unwrap()).collect();
    assert_eq!(text, "he\"llo");
}

#[test]
fn reads_character_literals() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    assert_eq!(read_one(&runtime, &mut ctx, "#\\a"), Word::character(97));
    assert_eq!(
        read_one(&runtime, &mut ctx, "#\\Space"),
        Word::character(32)
    );
    assert_eq!(
        read_one(&runtime, &mut ctx, "#\\Newline"),
        Word::character(10)
    );
    assert_eq!(read_one(&runtime, &mut ctx, "#\\("), Word::character(40));
}

#[test]
fn reads_vector_and_bit_vector_literals() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let v = read_one(&runtime, &mut ctx, "#(1 2 3)");
    assert!(matches!(
        classify_object(&ctx, v),
        ObjectRef::SimpleVector(_)
    ));
    let bits = read_one(&runtime, &mut ctx, "#*101");
    assert!(matches!(
        classify_object(&ctx, bits),
        ObjectRef::SpecializedArray(_)
    ));
}

#[test]
fn reads_uninterned_symbols() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let sym = read_one(&runtime, &mut ctx, "#:foo");
    assert_eq!(name_of(&ctx, sym), "FOO");
}

#[test]
fn reads_quote_with_the_common_lisp_symbol() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let common = Package::from_word(runtime.find_package(&ctx, "COMMON-LISP").unwrap());
    let (quote, status) = common.intern(&mut ctx, &runtime, "QUOTE").unwrap();
    assert_eq!(status, ncl_object::package::FindStatus::External);
    let form = read_one(&runtime, &mut ctx, "(quote a)");
    let operator = car(&ctx, form).unwrap();
    assert_eq!(operator, quote);
    assert_eq!(symbol_package(&ctx, operator), Ok(common.as_word()));
}

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
