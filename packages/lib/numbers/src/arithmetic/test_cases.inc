use super::*;
use ncl_object::make_bignum_from_i128;

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    super::super::register(&runtime).unwrap();
    (runtime, ctx)
}

fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> Word {
    let function = runtime
        .function(ctx, "COMMON-LISP", name)
        .and_then(|word| ncl_object::FunctionObject::try_from(word).ok())
        .unwrap();
    runtime.call_builtin(ctx, function, args).unwrap()
}

fn bignum(ctx: &mut ThreadContext, runtime: &Runtime, value: i128) -> Word {
    make_bignum_from_i128(ctx, runtime, value).unwrap().into()
}

#[test]
fn arithmetic_preserves_fixnum_bignum_boundary_and_argument_order() {
    let (runtime, mut ctx) = context();
    let max = i128::from(i64::MAX >> 4);
    let fixnum = Word::fixnum(max as i64);
    let next = bignum(&mut ctx, &runtime, max + 1);

    let result = add(&mut ctx, &runtime, &[fixnum, Word::fixnum(1)]).unwrap();
    assert_eq!(integer(&ctx, result).unwrap(), max + 1);
    let result = sub(&mut ctx, &runtime, &[next, Word::fixnum(1)]).unwrap();
    assert_eq!(integer(&ctx, result).unwrap(), max);
    let result = mul(
        &mut ctx,
        &runtime,
        &[Word::fixnum(2), Word::fixnum(3), Word::fixnum(4)],
    )
    .unwrap();
    assert_eq!(integer(&ctx, result).unwrap(), 24);
    let result = sub(
        &mut ctx,
        &runtime,
        &[Word::fixnum(10), Word::fixnum(2), Word::fixnum(3)],
    )
    .unwrap();
    assert_eq!(integer(&ctx, result).unwrap(), 5);
    let result = one_plus(&mut ctx, &runtime, &[fixnum]).unwrap();
    assert_eq!(integer(&ctx, result).unwrap(), max + 1);
    let result = one_minus(&mut ctx, &runtime, &[next]).unwrap();
    assert_eq!(integer(&ctx, result).unwrap(), max);
}

#[test]
fn comparisons_keep_pairwise_and_chain_semantics_at_boundary() {
    let (runtime, mut ctx) = context();
    let max = i128::from(i64::MAX >> 4);
    let fixnum = Word::fixnum(max as i64);
    let next = bignum(&mut ctx, &runtime, max + 1);
    let after = bignum(&mut ctx, &runtime, max + 2);

    assert_eq!(equal(&ctx, &runtime, &[fixnum, next]).unwrap(), Word::NIL);
    assert_eq!(
        not_equal(&ctx, &runtime, &[fixnum, next, after]).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        less(&ctx, &runtime, &[fixnum, next, after]).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        greater(&ctx, &runtime, &[after, next, fixnum]).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        less_equal(&ctx, &runtime, &[fixnum, next, next]).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        greater_equal(&ctx, &runtime, &[after, next, next]).unwrap(),
        Word::TRUE
    );
}

#[test]
fn registered_arithmetic_and_comparisons_use_builtin_boundary() {
    let (runtime, mut ctx) = context();
    let max = i128::from(i64::MAX >> 4);
    let fixnum = Word::fixnum(max as i64);
    let next = bignum(&mut ctx, &runtime, max + 1);

    let addition = call(&runtime, &mut ctx, "+", &[fixnum, Word::fixnum(1)]);
    assert_eq!(integer(&ctx, addition).unwrap(), max + 1);
    let result = call(&runtime, &mut ctx, "-", &[next, Word::fixnum(1)]);
    assert_eq!(integer(&ctx, result).unwrap(), max);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "*",
            &[Word::fixnum(2), Word::fixnum(3), Word::fixnum(4)],
        ),
        Word::fixnum(24)
    );
    let result = call(&runtime, &mut ctx, "1+", &[fixnum]);
    assert_eq!(integer(&ctx, result).unwrap(), max + 1);
    let result = call(&runtime, &mut ctx, "1-", &[next]);
    assert_eq!(integer(&ctx, result).unwrap(), max);
    assert_eq!(call(&runtime, &mut ctx, "=", &[fixnum, next]), Word::NIL);
    assert_eq!(call(&runtime, &mut ctx, "/=", &[fixnum, next]), Word::TRUE);
    assert_eq!(call(&runtime, &mut ctx, "<", &[fixnum, next]), Word::TRUE);
    assert_eq!(call(&runtime, &mut ctx, ">", &[next, fixnum]), Word::TRUE);
    assert_eq!(call(&runtime, &mut ctx, "<=", &[fixnum, next]), Word::TRUE);
    assert_eq!(call(&runtime, &mut ctx, ">=", &[next, fixnum]), Word::TRUE);
}
