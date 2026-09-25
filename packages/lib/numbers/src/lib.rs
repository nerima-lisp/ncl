//! Builtins for the Common Lisp numeric tower.

use ncl_object::{
    Builtin, BuiltinImplementation, ObjectError, ObjectRef, Runtime, ThreadContext, classify_object,
};
use ncl_sys::Word;

const COMMON_LISP: &str = "COMMON-LISP";

fn one_arg(args: &[Word]) -> Result<Word, ObjectError> {
    args.first().copied().ok_or(ObjectError::TypeError)
}
fn numeric(ctx: &ThreadContext, value: Word) -> bool {
    matches!(
        classify_object(ctx, value),
        ObjectRef::Fixnum(_)
            | ObjectRef::Bignum(_)
            | ObjectRef::Ratio(_)
            | ObjectRef::DoubleFloat(_)
            | ObjectRef::Complex(_)
    )
}
fn integer(ctx: &ThreadContext, value: Word) -> bool {
    matches!(
        classify_object(ctx, value),
        ObjectRef::Fixnum(_) | ObjectRef::Bignum(_)
    )
}
fn rational(ctx: &ThreadContext, value: Word) -> bool {
    matches!(
        classify_object(ctx, value),
        ObjectRef::Fixnum(_) | ObjectRef::Bignum(_) | ObjectRef::Ratio(_)
    )
}
fn real(ctx: &ThreadContext, value: Word) -> bool {
    matches!(
        classify_object(ctx, value),
        ObjectRef::Fixnum(_)
            | ObjectRef::Bignum(_)
            | ObjectRef::Ratio(_)
            | ObjectRef::DoubleFloat(_)
    )
}
fn floating(ctx: &ThreadContext, value: Word) -> bool {
    matches!(classify_object(ctx, value), ObjectRef::DoubleFloat(_))
}
fn complex(ctx: &ThreadContext, value: Word) -> bool {
    matches!(classify_object(ctx, value), ObjectRef::Complex(_))
}

fn predicate<F>(ctx: &mut ThreadContext, args: &[Word], test: F) -> Result<Word, ObjectError>
where
    F: FnOnce(&ThreadContext, Word) -> bool,
{
    Ok(if test(ctx, one_arg(args)?) {
        Word::TRUE
    } else {
        Word::NIL
    })
}
fn numberp(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    predicate(ctx, args, numeric)
}
fn integerp(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    predicate(ctx, args, integer)
}
fn rationalp(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    predicate(ctx, args, rational)
}
fn realp(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    predicate(ctx, args, real)
}
fn floatp(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    predicate(ctx, args, floating)
}
fn complexp(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    predicate(ctx, args, complex)
}

fn sign_predicate(
    ctx: &mut ThreadContext,
    args: &[Word],
    test: fn(i64) -> bool,
) -> Result<Word, ObjectError> {
    match classify_object(ctx, one_arg(args)?) {
        ObjectRef::Fixnum(value) => Ok(if test(value) { Word::TRUE } else { Word::NIL }),
        _ => Err(ObjectError::Unsupported),
    }
}
fn zerop(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    sign_predicate(ctx, args, |v| v == 0)
}
fn plusp(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    sign_predicate(ctx, args, |v| v > 0)
}
fn minusp(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    sign_predicate(ctx, args, |v| v < 0)
}
fn parity(ctx: &mut ThreadContext, args: &[Word], odd: bool) -> Result<Word, ObjectError> {
    match classify_object(ctx, one_arg(args)?) {
        ObjectRef::Fixnum(value) => Ok(if (value.rem_euclid(2) == 1) == odd {
            Word::TRUE
        } else {
            Word::NIL
        }),
        _ => Err(ObjectError::TypeError),
    }
}
fn evenp(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    parity(ctx, args, false)
}
fn oddp(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    parity(ctx, args, true)
}

fn compare(args: &[Word], relation: fn(i64, i64) -> bool) -> Result<Word, ObjectError> {
    if args.len() < 2 {
        return Err(ObjectError::TypeError);
    }
    let values: Option<Vec<i64>> = args.iter().map(|value| value.as_fixnum()).collect();
    let values = values.ok_or(ObjectError::Unsupported)?;
    Ok(
        if values.windows(2).all(|pair| relation(pair[0], pair[1])) {
            Word::TRUE
        } else {
            Word::NIL
        },
    )
}
fn equal(
    _: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    compare(args, |a, b| a == b)
}
fn not_equal(
    _: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    compare(args, |a, b| a != b)
}
fn less(
    _: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    compare(args, |a, b| a < b)
}
fn greater(
    _: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    compare(args, |a, b| a > b)
}
fn less_equal(
    _: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    compare(args, |a, b| a <= b)
}
fn greater_equal(
    _: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    compare(args, |a, b| a >= b)
}

fn identity(args: &[Word]) -> Result<Vec<Word>, ObjectError> {
    Ok(args.to_vec())
}
fn install(
    runtime: &Runtime,
    name: &str,
    function: ncl_object::RustBuiltin,
) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    runtime.register_builtin(
        &mut ctx,
        COMMON_LISP,
        name,
        BuiltinImplementation::adapted(
            Builtin {
                arity: 0,
                direct: false,
                lambda_list: "&rest args",
            },
            function,
            identity,
        ),
    )?;
    Ok(())
}

/// Register the implemented numeric predicates and fixnum comparisons.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    for (name, function) in [
        ("NUMBERP", numberp as ncl_object::RustBuiltin),
        ("INTEGERP", integerp),
        ("RATIONALP", rationalp),
        ("REALP", realp),
        ("FLOATP", floatp),
        ("COMPLEXP", complexp),
        ("ZEROP", zerop),
        ("PLUSP", plusp),
        ("MINUSP", minusp),
        ("EVENP", evenp),
        ("ODDP", oddp),
        ("=", equal),
        ("/=", not_equal),
        ("<", less),
        (">", greater),
        ("<=", less_equal),
        (">=", greater_equal),
    ] {
        install(runtime, name, function)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> Word {
        let function = runtime
            .function(ctx, COMMON_LISP, name)
            .expect("numeric builtin is registered");
        runtime
            .call_builtin(ctx, function.into(), args)
            .expect("numeric builtin succeeds")
    }

    #[test]
    fn predicates_and_comparisons_use_common_lisp_truth_values() {
        let runtime = Runtime::new().expect("runtime");
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("thread context");
        register(&runtime).expect("numeric registration");

        assert_eq!(call(&runtime, &mut ctx, "NUMBERP", &[Word::fixnum(7)]), Word::TRUE);
        assert_eq!(call(&runtime, &mut ctx, "INTEGERP", &[Word::fixnum(7)]), Word::TRUE);
        assert_eq!(call(&runtime, &mut ctx, "FLOATP", &[Word::fixnum(7)]), Word::NIL);
        assert_eq!(call(&runtime, &mut ctx, "ZEROP", &[Word::fixnum(0)]), Word::TRUE);
        assert_eq!(call(&runtime, &mut ctx, "PLUSP", &[Word::fixnum(2)]), Word::TRUE);
        assert_eq!(call(&runtime, &mut ctx, "MINUSP", &[Word::fixnum(-2)]), Word::TRUE);
        assert_eq!(call(&runtime, &mut ctx, "EVENP", &[Word::fixnum(-4)]), Word::TRUE);
        assert_eq!(call(&runtime, &mut ctx, "ODDP", &[Word::fixnum(-3)]), Word::TRUE);
        assert_eq!(call(&runtime, &mut ctx, "<", &[Word::fixnum(1), Word::fixnum(2)]), Word::TRUE);
        assert_eq!(call(&runtime, &mut ctx, "=", &[Word::fixnum(2), Word::fixnum(2)]), Word::TRUE);
        assert_eq!(call(&runtime, &mut ctx, ">=", &[Word::fixnum(2), Word::fixnum(3)]), Word::NIL);
    }

    #[test]
    fn comparison_requires_two_operands() {
        let runtime = Runtime::new().expect("runtime");
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("thread context");
        register(&runtime).expect("numeric registration");
        let function = runtime.function(&mut ctx, COMMON_LISP, "<").expect("builtin");
        assert_eq!(runtime.call_builtin(&mut ctx, function.into(), &[Word::fixnum(1)]), Err(ObjectError::TypeError));
    }
}
