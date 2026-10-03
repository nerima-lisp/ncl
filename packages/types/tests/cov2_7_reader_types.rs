#![allow(clippy::unwrap_used, reason = "coverage tests assert on each result")]
#![allow(missing_docs)]

use ncl_object::{
    FunctionObject, Package, Runtime, ThreadContext, Word, make_bignum_from_i128, make_cons,
};
use ncl_types::{
    IntegerBound, NamedType, TypeError, TypeSpecifier, parse_type_specifier, subtypep, typep,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_types::register(&runtime).unwrap();
    ncl_types::builtins::register(&runtime).unwrap();
    (runtime, ctx)
}

fn intern(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
    let package = runtime.find_package(ctx, "COMMON-LISP").unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn list(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Word {
    values.iter().rev().fold(Word::NIL, |tail, value| {
        make_cons(ctx, runtime, *value, tail).unwrap()
    })
}

#[test]
fn builtins_coerce_negative_reals_and_report_uncertain_subtype_forms() {
    let (runtime, mut ctx) = setup();
    let coerce =
        FunctionObject::try_from(runtime.function(&mut ctx, "COMMON-LISP", "COERCE").unwrap())
            .unwrap();
    let subtypep_builtin = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "SUBTYPEP")
            .unwrap(),
    )
    .unwrap();
    let double = intern(&mut ctx, &runtime, "DOUBLE-FLOAT");
    let bignum: Word = make_bignum_from_i128(&mut ctx, &runtime, -(1_i128 << 70))
        .unwrap()
        .into();
    let converted = runtime
        .call_builtin(&mut ctx, coerce, &[bignum, double])
        .unwrap();
    assert!(matches!(
        ncl_object::classify_object(&ctx, converted),
        ncl_object::ObjectRef::DoubleFloat(_)
    ));
    assert_eq!(
        ncl_object::double_value(&ctx, ncl_object::DoubleFloat::from_word(converted)).unwrap(),
        -2_f64.powi(70)
    );

    let malformed = Word::fixnum(7);
    let result = runtime.call_builtin(&mut ctx, subtypep_builtin, &[malformed, Word::TRUE]);
    assert_eq!(result, Ok(Word::NIL));
    assert_eq!(ctx.values(), &[Word::NIL]);
}

#[test]
fn typep_compound_specs_preserve_short_circuit_and_error_order() {
    let (_runtime, mut ctx) = setup();
    let satisfies = |name: &str| TypeSpecifier::Satisfies(name.to_owned());

    let accepted = TypeSpecifier::Or(vec![
        TypeSpecifier::Named(NamedType::Integer),
        satisfies("MUST-NOT-RUN"),
    ]);
    assert_eq!(typep(&mut ctx, Word::fixnum(3), &accepted), Ok(true));

    let rejected = TypeSpecifier::And(vec![
        TypeSpecifier::Named(NamedType::String),
        satisfies("MUST-NOT-RUN"),
    ]);
    assert_eq!(typep(&mut ctx, Word::fixnum(3), &rejected), Ok(false));

    let error = TypeSpecifier::Or(vec![
        satisfies("PREDICATE"),
        TypeSpecifier::Named(NamedType::T),
    ]);
    assert_eq!(
        typep(&mut ctx, Word::NIL, &error),
        Err(TypeError::CannotInvoke("PREDICATE".to_owned()))
    );
}

#[test]
fn parser_and_subtypep_keep_exclusive_integer_boundaries_distinct() {
    let (runtime, mut ctx) = setup();
    let integer = intern(&mut ctx, &runtime, "INTEGER");
    let exclusive_low = list(&mut ctx, &runtime, &[Word::fixnum(0)]);
    let exclusive_high = list(&mut ctx, &runtime, &[Word::fixnum(3)]);
    let form = list(
        &mut ctx,
        &runtime,
        &[integer, exclusive_low, exclusive_high],
    );
    let parsed = parse_type_specifier(&mut ctx, form).unwrap();
    assert_eq!(
        parsed,
        TypeSpecifier::IntegerRange {
            low: IntegerBound::Exclusive(0),
            high: IntegerBound::Exclusive(3),
        }
    );
    assert!(!typep(&mut ctx, Word::fixnum(0), &parsed).unwrap());
    assert!(typep(&mut ctx, Word::fixnum(1), &parsed).unwrap());
    assert!(!typep(&mut ctx, Word::fixnum(3), &parsed).unwrap());

    let sup = TypeSpecifier::IntegerRange {
        low: IntegerBound::Inclusive(0),
        high: IntegerBound::Inclusive(3),
    };
    assert_eq!(subtypep(&parsed, &sup).unwrap(), (true, true));
    assert_eq!(subtypep(&sup, &parsed).unwrap(), (false, false));
}
