use crate::{
    Constant, SymbolTokenKind, normalize_name, parse_float_literal, parse_radix_integer_literal,
    parse_symbol_token,
};

pub fn literal_constant(atom: &str) -> Option<Constant> {
    let token = parse_symbol_token(atom).ok()?;
    match token.kind {
        SymbolTokenKind::Keyword => {
            if token.escaped {
                Some(Constant::KeywordExact(token.name))
            } else {
                Some(Constant::Keyword(normalize_name(&token.name)))
            }
        }
        SymbolTokenKind::Symbol if token.package.is_none() && !token.escaped => {
            if token.name.eq_ignore_ascii_case("nil") || token.name.eq_ignore_ascii_case("#f") {
                return Some(Constant::Nil);
            }
            if token.name.eq_ignore_ascii_case("t") || token.name.eq_ignore_ascii_case("#t") {
                return Some(Constant::Boolean(true));
            }
            if let Some(value) = parse_radix_integer_literal(&token.name) {
                return Some(Constant::Integer(value));
            }
            if let Ok(value) = token.name.parse::<i64>() {
                return Some(Constant::Integer(value));
            }
            if let Some((numerator, denominator)) = rational_literal_parts(&token.name) {
                return if denominator == 1 {
                    Some(Constant::Integer(numerator))
                } else {
                    Some(Constant::Rational {
                        numerator,
                        denominator,
                    })
                };
            }
            parse_float_literal(&token.name).map(Constant::Float)
        }
        _ => None,
    }
}

fn rational_literal_parts(name: &str) -> Option<(i64, i64)> {
    let (numerator, denominator) = name.split_once('/')?;
    if numerator.is_empty()
        || denominator.is_empty()
        || numerator.contains('/')
        || denominator.contains('/')
    {
        return None;
    }
    let numerator = numerator.parse::<i128>().ok()?;
    let denominator = denominator.parse::<i128>().ok()?;
    if denominator == 0 {
        return None;
    }
    let (numerator, denominator) = if denominator < 0 {
        (numerator.checked_neg()?, denominator.checked_neg()?)
    } else {
        (numerator, denominator)
    };
    let numerator_abs = numerator.unsigned_abs();
    let denominator_abs = u128::try_from(denominator).ok()?;
    let divisor = gcd(numerator_abs, denominator_abs);
    let reduced_numerator = i128::try_from(numerator_abs / divisor).ok()?;
    let reduced_numerator = if numerator < 0 {
        reduced_numerator.checked_neg()?
    } else {
        reduced_numerator
    };
    let reduced_denominator = i128::try_from(denominator_abs / divisor).ok()?;
    Some((
        i64::try_from(reduced_numerator).ok()?,
        i64::try_from(reduced_denominator).ok()?,
    ))
}

const fn gcd(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_constants_are_parsed_from_a_table() {
        let cases = [
            ("nil", Constant::Nil),
            ("#t", Constant::Boolean(true)),
            (":ready", Constant::Keyword("READY".to_string())),
            (
                "1/2",
                Constant::Rational {
                    numerator: 1,
                    denominator: 2,
                },
            ),
            ("-6/3", Constant::Integer(-2)),
            ("#xFF", Constant::Integer(255)),
            ("#b1010", Constant::Integer(10)),
            ("#o777", Constant::Integer(511)),
            ("#3r120", Constant::Integer(15)),
            ("1.25s0", Constant::Float(1.25)),
        ];

        for (source, expected) in cases {
            assert_eq!(
                literal_constant(source),
                Some(expected),
                "source={source:?}"
            );
        }
    }

    #[test]
    fn rational_literals_cover_invalid_and_reduced_forms() {
        let cases = [
            ("6/8", Some((3, 4))),
            ("6/-8", Some((-3, 4))),
            ("0/9", Some((0, 1))),
            ("1/0", None),
            ("1/2/3", None),
            ("9223372036854775808/1", None),
        ];

        for (source, expected) in cases {
            assert_eq!(
                rational_literal_parts(source),
                expected,
                "source={source:?}"
            );
        }
    }
}
