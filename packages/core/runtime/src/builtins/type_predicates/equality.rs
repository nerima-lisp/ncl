use crate::Value;
use crate::builtins::numbers::{number, numeric_equalp};

pub fn eql_value(left: &Value, right: &Value) -> bool {
    let numeric_equal = match (left, right) {
        (Value::Integer(left), Value::Integer(right)) => left == right,
        (Value::BigInteger(left), Value::BigInteger(right)) => left == right,
        (Value::Rational(left), Value::Rational(right)) => left == right,
        #[expect(
            clippy::float_cmp,
            reason = "EQL requires exact floating-point equality"
        )]
        (Value::Float(left), Value::Float(right)) => left == right,
        _ => false,
    };
    left.eq_value(right) || numeric_equal
}

pub fn equalp_value(left: &Value, right: &Value) -> bool {
    if let (Ok(left), Ok(right)) = (number(left), number(right)) {
        return numeric_equalp(&left, &right);
    }
    match (left, right) {
        (Value::String(left), Value::String(right)) => left.eq_ignore_ascii_case(right),
        (Value::Character(left), Value::Character(right)) => left.eq_ignore_ascii_case(right),
        (Value::List(left), Value::List(right)) | (Value::Vector(left), Value::Vector(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(l, r)| equalp_value(l, r))
        }
        (
            Value::Array {
                dimensions: ld,
                elements: le,
            },
            Value::Array {
                dimensions: rd,
                elements: re,
            },
        ) => {
            ld == rd
                && le.len() == re.len()
                && le.iter().zip(re.iter()).all(|(l, r)| equalp_value(l, r))
        }
        (
            Value::DottedList {
                items: li,
                tail: lt,
            },
            Value::DottedList {
                items: ri,
                tail: rt,
            },
        ) => {
            li.len() == ri.len()
                && li.iter().zip(ri.iter()).all(|(l, r)| equalp_value(l, r))
                && equalp_value(lt, rt)
        }
        _ => eql_value(left, right),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equalp_value_compares_multi_dimensional_arrays_elementwise_ignoring_case() {
        let left = Value::array(
            vec![2, 2],
            vec![
                Value::Integer(1),
                Value::string("Ab"),
                Value::Integer(3),
                Value::Integer(4),
            ],
        );
        let right = Value::array(
            vec![2, 2],
            vec![
                Value::Integer(1),
                Value::string("aB"),
                Value::Integer(3),
                Value::Integer(4),
            ],
        );
        assert!(equalp_value(&left, &right));

        let different_elements = Value::array(
            vec![2, 2],
            vec![
                Value::Integer(1),
                Value::Integer(2),
                Value::Integer(3),
                Value::Integer(5),
            ],
        );
        assert!(!equalp_value(&left, &different_elements));

        let different_dimensions = Value::array(
            vec![4, 1],
            vec![
                Value::Integer(1),
                Value::string("Ab"),
                Value::Integer(3),
                Value::Integer(4),
            ],
        );
        assert!(!equalp_value(&left, &different_dimensions));
    }
}
