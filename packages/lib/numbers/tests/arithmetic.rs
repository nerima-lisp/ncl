#![allow(
    clippy::unwrap_used,
    missing_docs,
    reason = "tests assert on numeric builtin behavior"
)]

use ncl_object::{
    bignum_limbs, bignum_sign, classify_object, make_bignum_from_i128, Bignum, FunctionObject,
    ObjectError, ObjectRef, Runtime, ThreadContext, Word,
};

const MAX_FIXNUM: i64 = i64::MAX >> 4;
const MIN_FIXNUM: i64 = i64::MIN >> 4;

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
                -magnitude
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
