//! The `subtypep` relation.

use ncl_object::Word;

use crate::{NamedType, TypeError, TypeSpecifier};

/// Decide whether every object of type `sub` is also of type `sup`.
///
/// The second element reports whether the answer is definite. This phase
/// implements a partial subtypep: the numeric tower, integer ranges, the
/// sequence and array hierarchy, and the `or`/`and` combinations are decided;
/// everything else returns `(false, false)` (uncertain).
///
/// # Errors
///
/// Returns [`TypeError::UnexpandedDeftype`] when either operand names a
/// `deftype`, and [`TypeError::CannotInvoke`] for a `satisfies` operand.
pub fn subtypep(sub: &TypeSpecifier, sup: &TypeSpecifier) -> Result<(bool, bool), TypeError> {
    if sub == sup {
        return Ok((true, true));
    }
    if let TypeSpecifier::Deftype { name, .. } = sub {
        return Err(TypeError::UnexpandedDeftype(*name));
    }
    if let TypeSpecifier::Deftype { name, .. } = sup {
        return Err(TypeError::UnexpandedDeftype(*name));
    }
    if let TypeSpecifier::Satisfies(word) = sub {
        return Err(TypeError::CannotInvoke(*word));
    }
    if let TypeSpecifier::Satisfies(word) = sup {
        return Err(TypeError::CannotInvoke(*word));
    }
    match (sub, sup) {
        (TypeSpecifier::Named(NamedType::Nil), _)
        | (_, TypeSpecifier::Named(NamedType::T))
        | (TypeSpecifier::IntegerRange { .. }, TypeSpecifier::Named(NamedType::Integer)) => {
            Ok((true, true))
        }
        (TypeSpecifier::Named(NamedType::T), _) | (_, TypeSpecifier::Named(NamedType::Nil)) => {
            Ok((false, true))
        }
        (TypeSpecifier::Named(sub), TypeSpecifier::Named(sup)) => Ok(named_subtype(*sub, *sup)),
        (
            TypeSpecifier::IntegerRange { low, high },
            TypeSpecifier::IntegerRange {
                low: sup_low,
                high: sup_high,
            },
        ) => Ok(range_subtype(*low, *high, *sup_low, *sup_high)),
        (TypeSpecifier::Or(subs), sup) => or_subtype(subs, sup),
        (sub, TypeSpecifier::Or(sups)) => or_supertype(sub, sups),
        (TypeSpecifier::And(subs), sup) => and_subtype(subs, sup),
        _ => Ok((false, false)),
    }
}

const fn named_subtype(sub: NamedType, sup: NamedType) -> (bool, bool) {
    if disjoint(sub, sup) {
        return (false, true);
    }
    if is_supertype(sup, sub) {
        return (true, true);
    }
    (false, false)
}

#[allow(clippy::similar_names)]
fn range_subtype(
    sub_low: Option<Word>,
    sub_high: Option<Word>,
    sup_low: Option<Word>,
    sup_high: Option<Word>,
) -> (bool, bool) {
    let sub_low = sub_low.and_then(Word::as_fixnum);
    let sub_high = sub_high.and_then(Word::as_fixnum);
    let sup_low = sup_low.and_then(Word::as_fixnum);
    let sup_high = sup_high.and_then(Word::as_fixnum);

    if let (Some(low), Some(high)) = (sub_low, sub_high)
        && low > high
    {
        return (true, true);
    }
    let low_ok = match (sup_low, sub_low) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(sup), Some(sub)) => sub >= sup,
    };
    let high_ok = match (sup_high, sub_high) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(sup), Some(sub)) => sub <= sup,
    };
    if low_ok && high_ok {
        (true, true)
    } else {
        (false, false)
    }
}

fn or_subtype(subs: &[TypeSpecifier], sup: &TypeSpecifier) -> Result<(bool, bool), TypeError> {
    let mut definite = true;
    for sub in subs {
        let (is_sub, is_definite) = subtypep(sub, sup)?;
        if !is_sub {
            return Ok((false, false));
        }
        definite = definite && is_definite;
    }
    Ok((true, definite))
}

fn or_supertype(sub: &TypeSpecifier, sups: &[TypeSpecifier]) -> Result<(bool, bool), TypeError> {
    for sup in sups {
        let (is_sub, is_definite) = subtypep(sub, sup)?;
        if is_sub {
            return Ok((true, is_definite));
        }
    }
    Ok((false, false))
}

fn and_subtype(subs: &[TypeSpecifier], sup: &TypeSpecifier) -> Result<(bool, bool), TypeError> {
    for sub in subs {
        if subtypep(sub, sup)?.0 {
            return Ok((true, true));
        }
    }
    Ok((false, false))
}

const fn is_supertype(sup: NamedType, sub: NamedType) -> bool {
    use NamedType::{
        Array, Bignum, BitVector, CompiledFunction, Complex, Cons, DoubleFloat, Fixnum, Float,
        Function, Integer, Keyword, List, LongFloat, Null, Number, Ratio, Rational, Real, Sequence,
        ShortFloat, SimpleArray, SimpleBitVector, SimpleString, SimpleVector, SingleFloat, String,
        Symbol, Vector,
    };
    matches!(
        (sup, sub),
        (
            Number,
            Real | Rational
                | Integer
                | Fixnum
                | Bignum
                | Ratio
                | Float
                | ShortFloat
                | SingleFloat
                | DoubleFloat
                | LongFloat
                | Complex
        ) | (
            Real,
            Rational
                | Integer
                | Fixnum
                | Bignum
                | Ratio
                | Float
                | ShortFloat
                | SingleFloat
                | DoubleFloat
                | LongFloat
        ) | (Rational, Integer | Fixnum | Bignum | Ratio)
            | (Integer, Fixnum | Bignum)
            | (Float, ShortFloat | SingleFloat | DoubleFloat | LongFloat)
            | (
                Sequence,
                List | Cons
                    | Null
                    | Vector
                    | String
                    | SimpleString
                    | BitVector
                    | SimpleBitVector
                    | SimpleVector
            )
            | (
                Array,
                Vector
                    | String
                    | SimpleString
                    | BitVector
                    | SimpleBitVector
                    | SimpleVector
                    | SimpleArray
            )
            | (
                Vector,
                String | SimpleString | BitVector | SimpleBitVector | SimpleVector
            )
            | (List, Cons | Null)
            | (String, SimpleString)
            | (BitVector, SimpleBitVector)
            | (Function, CompiledFunction)
            | (Symbol, Keyword)
    )
}

#[allow(clippy::unnested_or_patterns)]
const fn disjoint(a: NamedType, b: NamedType) -> bool {
    use NamedType::{
        Bignum, Complex, Cons, Fixnum, Float, Integer, Null, Number, Ratio, Rational, Real, Symbol,
    };
    matches!(
        (a, b),
        (Integer | Rational, Float)
            | (Float | Complex, Integer)
            | (Integer | Rational | Real, Complex)
            | (Float | Complex, Rational)
            | (Complex, Real)
            | (Fixnum | Bignum, Ratio)
            | (Ratio, Fixnum | Bignum)
            | (Cons | Number, Symbol)
            | (Symbol | Number | Null, Cons)
            | (Cons | Symbol, Number)
            | (Cons, Null)
    )
}
