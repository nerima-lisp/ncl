use super::core::gcd_i128;
use super::*;
use ncl_object::make_bignum_from_i128;

fn context() -> Result<(Runtime, ThreadContext), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    super::super::register(&runtime)?;
    Ok((runtime, ctx))
}

fn call(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let function = runtime
        .function(ctx, "COMMON-LISP", name)
        .and_then(|word| ncl_object::FunctionObject::try_from(word).ok())
        .ok_or(ObjectError::TypeError)?;
    runtime.call_builtin(ctx, function, args)
}

fn bignum(ctx: &mut ThreadContext, runtime: &Runtime, value: i128) -> Result<Word, ObjectError> {
    Ok(make_bignum_from_i128(ctx, runtime, value)?.into())
}

#[test]
fn arithmetic_preserves_fixnum_bignum_boundary_and_argument_order() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = context()?;
    let max = i128::from(crate::MOST_POSITIVE_FIXNUM);
    let fixnum = Word::fixnum(crate::MOST_POSITIVE_FIXNUM);
    let next = bignum(&mut ctx, &runtime, max + 1)?;

    let result = add(&mut ctx, &runtime, &[fixnum, Word::fixnum(1)])?;
    assert_eq!(integer(&ctx, result)?, max + 1);
    let result = sub(&mut ctx, &runtime, &[next, Word::fixnum(1)])?;
    assert_eq!(integer(&ctx, result)?, max);
    let result = mul(
        &mut ctx,
        &runtime,
        &[Word::fixnum(2), Word::fixnum(3), Word::fixnum(4)],
    )?;
    assert_eq!(integer(&ctx, result)?, 24);
    let result = sub(
        &mut ctx,
        &runtime,
        &[Word::fixnum(10), Word::fixnum(2), Word::fixnum(3)],
    )?;
    assert_eq!(integer(&ctx, result)?, 5);
    let result = one_plus(&mut ctx, &runtime, &[fixnum])?;
    assert_eq!(integer(&ctx, result)?, max + 1);
    let result = one_minus(&mut ctx, &runtime, &[next])?;
    assert_eq!(integer(&ctx, result)?, max);
    Ok(())
}

#[test]
fn arithmetic_handles_i64_and_i128_boundaries_without_wrapping() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = context()?;
    let min_i64 = bignum(&mut ctx, &runtime, i128::from(i64::MIN))?;
    assert_eq!(integer(&ctx, min_i64)?, i128::from(i64::MIN));
    let min_i64_minus_one = sub(&mut ctx, &runtime, &[min_i64, Word::fixnum(1)])?;
    assert_eq!(integer(&ctx, min_i64_minus_one)?, i128::from(i64::MIN) - 1);

    let max = bignum(&mut ctx, &runtime, i128::MAX)?;
    let min = bignum(&mut ctx, &runtime, i128::MIN)?;
    assert_eq!(integer(&ctx, max)?, i128::MAX);
    assert_eq!(integer(&ctx, min)?, i128::MIN);

    assert_eq!(
        word(
            &mut ctx,
            &runtime,
            add_pair(Number::Integer(i128::MAX), Number::Integer(1))
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        word(
            &mut ctx,
            &runtime,
            sub_pair(Number::Integer(i128::MIN), Number::Integer(1))
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        word(
            &mut ctx,
            &runtime,
            mul_pair(Number::Integer(i128::MAX), Number::Integer(2))
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        div_pair(Number::Integer(i128::MIN), Number::Integer(-1))
            .and_then(|value| word(&mut ctx, &runtime, value)),
        Err(ObjectError::TypeError)
    );

    assert_eq!(gcd_i128(i128::MIN, -1), 1);
    Ok(())
}

#[test]
fn arithmetic_rejects_i128_overflow_and_abs_minimum() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = context()?;
    let max = bignum(&mut ctx, &runtime, i128::MAX)?;
    let one = Word::fixnum(1);
    let near_min = bignum(&mut ctx, &runtime, i128::MIN + 1)?;

    assert_eq!(
        add(&mut ctx, &runtime, &[max, one]),
        Err(ObjectError::TypeError)
    );
    let result = sub(&mut ctx, &runtime, &[near_min, one])?;
    assert_eq!(integer(&ctx, result)?, i128::MIN);
    assert_eq!(
        mul(&mut ctx, &runtime, &[max, Word::fixnum(2)]),
        Err(ObjectError::TypeError)
    );
    assert!(matches!(
        abs_number(Number::Integer(i128::MIN)),
        Err(ObjectError::TypeError)
    ));
    Ok(())
}

#[test]
fn comparisons_keep_pairwise_and_chain_semantics_at_boundary() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = context()?;
    let max = i128::from(crate::MOST_POSITIVE_FIXNUM);
    let fixnum = Word::fixnum(crate::MOST_POSITIVE_FIXNUM);
    let next = bignum(&mut ctx, &runtime, max + 1)?;
    let after = bignum(&mut ctx, &runtime, max + 2)?;

    assert_eq!(equal(&ctx, &runtime, &[fixnum, next])?, Word::NIL);
    assert_eq!(
        not_equal(&ctx, &runtime, &[fixnum, next, after])?,
        Word::TRUE
    );
    assert_eq!(less(&ctx, &runtime, &[fixnum, next, after])?, Word::TRUE);
    assert_eq!(greater(&ctx, &runtime, &[after, next, fixnum])?, Word::TRUE);
    assert_eq!(
        less_equal(&ctx, &runtime, &[fixnum, next, next])?,
        Word::TRUE
    );
    assert_eq!(
        greater_equal(&ctx, &runtime, &[after, next, next])?,
        Word::TRUE
    );
    Ok(())
}

#[test]
fn registered_arithmetic_and_comparisons_use_builtin_boundary() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = context()?;
    let max = i128::from(crate::MOST_POSITIVE_FIXNUM);
    let fixnum = Word::fixnum(crate::MOST_POSITIVE_FIXNUM);
    let next = bignum(&mut ctx, &runtime, max + 1)?;

    let addition = call(&runtime, &mut ctx, "+", &[fixnum, Word::fixnum(1)])?;
    assert_eq!(integer(&ctx, addition)?, max + 1);
    let result = call(&runtime, &mut ctx, "-", &[next, Word::fixnum(1)])?;
    assert_eq!(integer(&ctx, result)?, max);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "*",
            &[Word::fixnum(2), Word::fixnum(3), Word::fixnum(4)],
        )?,
        Word::fixnum(24)
    );
    let result = call(&runtime, &mut ctx, "1+", &[fixnum])?;
    assert_eq!(integer(&ctx, result)?, max + 1);
    let result = call(&runtime, &mut ctx, "1-", &[next])?;
    assert_eq!(integer(&ctx, result)?, max);
    assert_eq!(call(&runtime, &mut ctx, "=", &[fixnum, next])?, Word::NIL);
    assert_eq!(call(&runtime, &mut ctx, "/=", &[fixnum, next])?, Word::TRUE);
    assert_eq!(call(&runtime, &mut ctx, "<", &[fixnum, next])?, Word::TRUE);
    assert_eq!(call(&runtime, &mut ctx, ">", &[next, fixnum])?, Word::TRUE);
    assert_eq!(call(&runtime, &mut ctx, "<=", &[fixnum, next])?, Word::TRUE);
    assert_eq!(call(&runtime, &mut ctx, ">=", &[next, fixnum])?, Word::TRUE);
    Ok(())
}
