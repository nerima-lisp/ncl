use super::{
    Number, ObjectError, Ordering, Runtime, ThreadContext, Word, args_numbers, bool_word, number,
    word,
};
use ncl_object::{ObjectRef, classify_object};

fn comparison(
    ctx: &ThreadContext,
    args: &[Word],
    cmp: impl Fn(Ordering) -> bool,
) -> Result<Word, ObjectError> {
    let ns = args_numbers(ctx, args)?;
    for pair in ns.windows(2) {
        let [left, right] = pair else { continue };
        if !cmp(compare_numbers(*left, *right)?) {
            return Ok(Word::NIL);
        }
    }
    Ok(Word::TRUE)
}

fn compare_numbers(left: Number, right: Number) -> Result<Ordering, ObjectError> {
    match (left, right) {
        (Number::Integer(left), Number::Integer(right)) => Ok(left.cmp(&right)),
        (left, right) => Ok(left
            .to_f64()?
            .partial_cmp(&right.to_f64()?)
            .unwrap_or(Ordering::Equal)),
    }
}
pub fn equal(ctx: &ThreadContext, _: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    comparison(ctx, args, |ordering| ordering == Ordering::Equal)
}

pub fn eq(_: &ThreadContext, _: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    let [left, right] = args else {
        return Err(ObjectError::TypeError);
    };
    Ok(bool_word(left == right))
}

const fn eql_numbers(left: Number, right: Number) -> bool {
    match (left, right) {
        (Number::Integer(left), Number::Integer(right)) => left == right,
        (Number::Ratio(left_n, left_d), Number::Ratio(right_n, right_d)) => {
            left_n == right_n && left_d == right_d
        }
        (Number::Float(left), Number::Float(right)) => left.to_bits() == right.to_bits(),
        (Number::Complex(left_r, left_i), Number::Complex(right_r, right_i)) => {
            left_r.to_bits() == right_r.to_bits() && left_i.to_bits() == right_i.to_bits()
        }
        _ => false,
    }
}

pub fn eql(ctx: &ThreadContext, _: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    let [left, right] = args else {
        return Err(ObjectError::TypeError);
    };
    let result = if left == right {
        true
    } else {
        match (classify_object(ctx, *left), classify_object(ctx, *right)) {
            (
                ObjectRef::Fixnum(_) | ObjectRef::Bignum(_),
                ObjectRef::Fixnum(_) | ObjectRef::Bignum(_),
            )
            | (ObjectRef::Ratio(_), ObjectRef::Ratio(_))
            | (ObjectRef::DoubleFloat(_), ObjectRef::DoubleFloat(_))
            | (ObjectRef::Complex(_), ObjectRef::Complex(_)) => {
                match (number(ctx, *left), number(ctx, *right)) {
                    (Ok(left), Ok(right)) => eql_numbers(left, right),
                    _ => false,
                }
            }
            _ => false,
        }
    };
    Ok(bool_word(result))
}
pub fn not_equal(ctx: &ThreadContext, _: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    let ns = args_numbers(ctx, args)?;
    for (index, value) in ns.iter().enumerate() {
        for other in ns.iter().skip(index + 1) {
            if compare_numbers(*value, *other)? == Ordering::Equal {
                return Ok(Word::NIL);
            }
        }
    }
    Ok(Word::TRUE)
}
pub fn less(ctx: &ThreadContext, _: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    comparison(ctx, args, |ordering| ordering == Ordering::Less)
}
pub fn greater(ctx: &ThreadContext, _: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    comparison(ctx, args, |ordering| ordering == Ordering::Greater)
}
pub fn less_equal(ctx: &ThreadContext, _: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    comparison(ctx, args, |ordering| ordering != Ordering::Greater)
}
pub fn greater_equal(ctx: &ThreadContext, _: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    comparison(ctx, args, |ordering| ordering != Ordering::Less)
}
pub fn max(ctx: &mut ThreadContext, runtime: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    let ns = args_numbers(ctx, args)?;
    let mut values = ns.into_iter();
    let mut value = values.next().ok_or(ObjectError::TypeError)?;
    for next in values {
        if next.to_f64()? > value.to_f64()? {
            value = next;
        }
    }
    word(ctx, runtime, value)
}
pub fn min(ctx: &mut ThreadContext, runtime: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    let ns = args_numbers(ctx, args)?;
    let mut values = ns.into_iter();
    let mut value = values.next().ok_or(ObjectError::TypeError)?;
    for next in values {
        if next.to_f64()? < value.to_f64()? {
            value = next;
        }
    }
    word(ctx, runtime, value)
}
