#![allow(clippy::unwrap_used, reason = "tests assert on reader behavior")]

//! Boundary coverage for reader options, entry points, and malformed syntax.

use ncl_object::{ObjectRef, Runtime, ThreadContext, Word, car, cdr, classify, symbol_name};
use ncl_reader::{
    FloatFormat, PackageName, ReadBase, ReadError, ReadEvaluation, ReadOptions, ReadSuppression,
    read_delimited_list, read_from_string, read_preserving_whitespace,
};

fn standard(runtime: &Runtime, ctx: &mut ThreadContext) -> ReadOptions {
    ReadOptions::standard(ctx, runtime).unwrap()
}

fn symbol_text(ctx: &ThreadContext, word: Word) -> String {
    let name = symbol_name(ctx, word).unwrap();
    let length = ncl_object::string_length(ctx, name).unwrap();
    (0..length)
        .map(|index| ncl_object::string_ref(ctx, name, index).unwrap())
        .collect()
}

#[test]
fn validates_option_boundaries_and_exposes_configured_values() {
    assert_eq!(ReadBase::new(1), Err(ReadError::InvalidBase(1)));
    assert_eq!(ReadBase::new(37), Err(ReadError::InvalidBase(37)));
    assert_eq!(ReadBase::new(36).unwrap().value(), 36);
    assert_eq!(
        PackageName::new(""),
        Err(ReadError::InvalidSymbolToken(String::new()))
    );
    assert_eq!(PackageName::new("USER").unwrap().as_str(), "USER");

    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut opts = standard(&runtime, &mut ctx);
    opts.set_read_base(ReadBase::new(16).unwrap());
    opts.set_read_evaluation(ReadEvaluation::Disabled);
    opts.set_read_suppression(ReadSuppression::Discard);
    opts.set_default_float_format(FloatFormat::SingleFloat);
    opts.set_current_package("USER").unwrap();
    assert_eq!(opts.read_base().value(), 16);
    assert_eq!(opts.read_evaluation(), ReadEvaluation::Disabled);
    assert_eq!(opts.read_suppression(), ReadSuppression::Discard);
    assert_eq!(opts.default_float_format(), FloatFormat::SingleFloat);
    assert_eq!(opts.current_package().unwrap().as_str(), "USER");
}

#[test]
fn suppression_returns_nil_and_preserving_entry_point_reads_forms() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut opts = standard(&runtime, &mut ctx);
    opts.set_read_suppression(ReadSuppression::Discard);
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "42", &opts)
            .unwrap()
            .unwrap(),
        Word::NIL
    );

    opts.set_read_suppression(ReadSuppression::Keep);
    let mut source = ncl_reader::StringSource::new("17 23");
    let first = ncl_reader::read(&mut ctx, &runtime, &mut source, &opts)
        .unwrap()
        .unwrap();
    let second = read_preserving_whitespace(&mut ctx, &runtime, &mut source, &opts)
        .unwrap()
        .unwrap();
    assert_eq!(classify(first), ObjectRef::Fixnum(17));
    assert_eq!(classify(second), ObjectRef::Fixnum(23));
}

#[test]
fn delimited_list_and_dotted_list_preserve_structure() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let opts = standard(&runtime, &mut ctx);
    let mut source = ncl_reader::StringSource::new("alpha beta)");
    let list = read_delimited_list(&mut ctx, &runtime, &mut source, &opts).unwrap();
    assert_eq!(symbol_text(&ctx, car(&ctx, list).unwrap()), "ALPHA");
    let tail = cdr(&ctx, list).unwrap();
    assert_eq!(symbol_text(&ctx, car(&ctx, tail).unwrap()), "BETA");
    assert_eq!(cdr(&ctx, tail).unwrap(), Word::NIL);

    let pair = read_from_string(&mut ctx, &runtime, "(alpha . 42)", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(classify(cdr(&ctx, pair).unwrap()), ObjectRef::Fixnum(42));
}

#[test]
fn reports_malformed_lists_and_dispatch_forms() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let opts = standard(&runtime, &mut ctx);
    for (text, expected) in [
        ("(.)", ReadError::DotWithoutCdr),
        ("(a .)", ReadError::UnmatchedRightParen),
        ("(a b", ReadError::UnexpectedEof),
        ("#q", ReadError::UndefinedDispatchMacro('q')),
        ("#.", ReadError::ReadEvalUnavailable),
    ] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, text, &opts).unwrap_err(),
            expected
        );
    }
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#.", &{
            let mut disabled = opts.clone();
            disabled.set_read_evaluation(ReadEvaluation::Disabled);
            disabled
        })
        .unwrap_err(),
        ReadError::ReadEvalDisabled
    );
}

#[test]
fn reports_dispatch_boundary_errors_and_nested_comments() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let opts = standard(&runtime, &mut ctx);
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#\\NoSuchCharacter", &opts).unwrap_err(),
        ReadError::UnknownCharacterName("NoSuchCharacter".to_owned())
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#| unterminated", &opts).unwrap_err(),
        ReadError::UnexpectedEof
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#1r10", &opts).unwrap_err(),
        ReadError::InvalidBase(1)
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#37r1", &opts).unwrap_err(),
        ReadError::InvalidBase(37)
    );
    assert_eq!(
        classify(
            read_from_string(&mut ctx, &runtime, "#| outer #| inner |# done |# 99", &opts)
                .unwrap()
                .unwrap()
        ),
        ObjectRef::Fixnum(99)
    );
}

#[test]
fn evaluates_nested_feature_expressions_and_rejects_unknown_forms() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    runtime.add_feature("ALPHA");
    let opts = standard(&runtime, &mut ctx);
    assert_eq!(
        classify(
            read_from_string(&mut ctx, &runtime, "#+(and alpha (not beta)) 11 22", &opts)
                .unwrap()
                .unwrap()
        ),
        ObjectRef::Fixnum(11)
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#+(xor alpha beta) 11", &opts).unwrap_err(),
        ReadError::InvalidFeatureExpression
    );
}
