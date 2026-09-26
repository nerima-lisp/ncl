#![allow(
    clippy::unwrap_used,
    missing_docs,
    reason = "tests assert on numeric builtin behavior"
)]

use ncl_object::{
    Bignum, FunctionObject, ObjectError, ObjectRef, Runtime, ThreadContext, Word, bignum_limbs,
    bignum_sign, classify_object, make_bignum_from_i128, make_ratio, ratio_denominator,
    ratio_numerator,
};

const MAX_FIXNUM: i64 = i64::MAX >> ncl_sys::FIXNUM_TAG_BITS;
const MIN_FIXNUM: i64 = i64::MIN >> ncl_sys::FIXNUM_TAG_BITS;

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_numbers::register(&runtime).unwrap();
    (runtime, ctx)
}

fn function(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap()
}

fn call(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let function = function(runtime, ctx, name);
    runtime.call_builtin(ctx, function, args)
}

fn integer(ctx: &ThreadContext, word: Word) -> i128 {
    match classify_object(ctx, word) {
        ObjectRef::Fixnum(value) => i128::from(value),
        ObjectRef::Bignum(value) => {
            let magnitude = bignum_limbs(ctx, Bignum::from_word(value))
                .unwrap()
                .into_iter()
                .enumerate()
                .fold(0_i128, |value, (index, limb)| {
                    value | (i128::from(limb) << (index * 32))
                });
            if bignum_sign(ctx, Bignum::from_word(value)).unwrap() {
                if magnitude == (1_i128 << 127) {
                    i128::MIN
                } else {
                    -magnitude
                }
            } else {
                magnitude
            }
        }
        other => panic!("expected integer, got {other:?}"),
    }
}

fn assert_integer(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
    expected: i128,
) {
    let result = call(runtime, ctx, name, args).unwrap();
    assert_eq!(integer(ctx, result), expected, "{name}");
}

fn assert_boolean(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
    expected: bool,
) {
    assert_eq!(
        call(runtime, ctx, name, args).unwrap(),
        if expected { Word::TRUE } else { Word::NIL },
        "{name}"
    );
}

#[test]
fn arithmetic_builtins_call_through_runtime() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);

    assert_integer(&runtime, &mut ctx, "+", &[], 0);
    assert_integer(&runtime, &mut ctx, "*", &[], 1);
    for name in ["=", "/=", "<", ">", "<=", ">="] {
        assert_boolean(&runtime, &mut ctx, name, &[], true);
        assert_boolean(&runtime, &mut ctx, name, &[two], true);
    }

    assert_integer(&runtime, &mut ctx, "+", &[one], 1);
    assert_integer(&runtime, &mut ctx, "-", &[one], -1);
    assert_integer(&runtime, &mut ctx, "*", &[three], 3);
    assert_integer(&runtime, &mut ctx, "1+", &[three], 4);
    assert_integer(&runtime, &mut ctx, "1-", &[three], 2);
    assert_boolean(&runtime, &mut ctx, "=", &[two, two, two], true);
    assert_boolean(&runtime, &mut ctx, "/=", &[one, two, three], true);

    assert_integer(&runtime, &mut ctx, "+", &[one, two, three], 6);
    assert_integer(&runtime, &mut ctx, "-", &[three, two, one], 0);
    assert_integer(&runtime, &mut ctx, "*", &[two, three, two], 12);
    assert_boolean(&runtime, &mut ctx, "<", &[one, two, three], true);
    assert_boolean(&runtime, &mut ctx, ">", &[three, two, one], true);
    assert_boolean(&runtime, &mut ctx, "<=", &[one, two, two], true);
    assert_boolean(&runtime, &mut ctx, ">=", &[three, two, two], true);

    assert_integer(
        &runtime,
        &mut ctx,
        "+",
        &[Word::fixnum(MAX_FIXNUM), one],
        i128::from(MAX_FIXNUM) + 1,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "-",
        &[Word::fixnum(MIN_FIXNUM), one],
        i128::from(MIN_FIXNUM) - 1,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "*",
        &[Word::fixnum(MAX_FIXNUM), two],
        i128::from(MAX_FIXNUM) * 2,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "1+",
        &[Word::fixnum(MAX_FIXNUM)],
        i128::from(MAX_FIXNUM) + 1,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "1-",
        &[Word::fixnum(MIN_FIXNUM)],
        i128::from(MIN_FIXNUM) - 1,
    );

    let big = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 70)
        .unwrap()
        .into();
    let bigger = make_bignum_from_i128(&mut ctx, &runtime, (1_i128 << 70) + 1)
        .unwrap()
        .into();
    assert_integer(&runtime, &mut ctx, "+", &[big, one], (1_i128 << 70) + 1);
    assert_integer(&runtime, &mut ctx, "-", &[big, one], (1_i128 << 70) - 1);
    assert_integer(&runtime, &mut ctx, "*", &[big, two], 2_i128 << 70);
    assert_boolean(&runtime, &mut ctx, "=", &[big, big], true);
    assert_boolean(&runtime, &mut ctx, "/=", &[big, bigger], true);
    assert_boolean(&runtime, &mut ctx, "<", &[big, bigger], true);
    assert_boolean(&runtime, &mut ctx, ">", &[bigger, big], true);
    assert_boolean(&runtime, &mut ctx, "<=", &[big, bigger], true);
    assert_boolean(&runtime, &mut ctx, ">=", &[bigger, big], true);
}

#[test]
fn ratio_and_complex_results_survive_gc_stress_and_strict_forwarding() {
    let (runtime, mut ctx) = setup();
    let ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2))
        .unwrap()
        .into();
    let real = ncl_object::make_double(&mut ctx, &runtime, 2.0)
        .unwrap()
        .into();
    let imag = ncl_object::make_double(&mut ctx, &runtime, 3.0)
        .unwrap()
        .into();
    let complex = ncl_object::make_complex(&mut ctx, &runtime, real, imag)
        .unwrap()
        .into();
    let function = function(&runtime, &mut ctx, "+");
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);

    let mut ratio = ratio;
    let token = ncl_object::push_root(&mut ctx, &mut ratio);
    let mut complex = complex;
    let complex_token = ncl_object::push_root(&mut ctx, &mut complex);
    let result = runtime
        .call_builtin(&mut ctx, function, &[ratio, Word::fixnum(1)])
        .unwrap();
    let ObjectRef::Ratio(value) = classify_object(&ctx, result) else {
        panic!(
            "expected ratio result, got {:?}",
            classify_object(&ctx, result)
        );
    };
    let ratio = ncl_object::Ratio::from_word(value);
    assert_eq!(integer(&ctx, ratio_numerator(&ctx, ratio).unwrap()), 3);
    assert_eq!(integer(&ctx, ratio_denominator(&ctx, ratio).unwrap()), 2);
    let complex_result = runtime
        .call_builtin(&mut ctx, function, &[complex, Word::fixnum(1)])
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, complex_result),
        ObjectRef::Complex(_)
    ));
    assert!(ncl_object::pop_root(&mut ctx, complex_token));
    assert!(ncl_object::pop_root(&mut ctx, token));
}
