#![expect(
    clippy::unwrap_used,
    reason = "tests assert on numeric predicate behavior with known-valid integer fixtures"
)]
//! Integer parity predicates for values beyond i128.

use ncl_object::{FunctionObject, Runtime, ThreadContext, Word, make_bignum_from_limbs};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_conditions::register(&runtime).unwrap();
    ncl_lib_numbers::register(&runtime).unwrap();
    (runtime, ctx)
}

fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, value: Word) -> Word {
    let function =
        FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap();
    runtime.call_builtin(ctx, function, &[value]).unwrap()
}

fn integer_from_limbs(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    negative: bool,
    limbs: &[u32],
) -> Word {
    make_bignum_from_limbs(ctx, runtime, negative, limbs)
        .unwrap()
        .into()
}

#[test]
fn integer_predicates_handle_bignums_beyond_i128() {
    let (runtime, mut ctx) = setup();
    let even = integer_from_limbs(
        &mut ctx,
        &runtime,
        false,
        &[0xeb1f_0ad2, 0xab54_a98c, 0, 0, 2],
    );
    let positive_odd = integer_from_limbs(
        &mut ctx,
        &runtime,
        false,
        &[0xeb1f_0ad3, 0xab54_a98c, 0, 0, 2],
    );
    let negative_odd = integer_from_limbs(
        &mut ctx,
        &runtime,
        true,
        &[0xeb1f_0ad3, 0xab54_a98c, 0, 0, 2],
    );

    assert_eq!(call(&runtime, &mut ctx, "INTEGERP", even), Word::TRUE);
    assert_eq!(call(&runtime, &mut ctx, "EVENP", even), Word::TRUE);
    assert_eq!(call(&runtime, &mut ctx, "ODDP", positive_odd), Word::TRUE);
    assert_eq!(call(&runtime, &mut ctx, "ODDP", negative_odd), Word::TRUE);
}
