use crate::Value;
use crate::builtins::numbers::{number, numeric_equalp};
use std::collections::HashSet;

pub fn eql_value(left: &Value, right: &Value) -> bool {
    let numeric_equal = match (left, right) {
        (Value::Integer(left), Value::Integer(right)) => left == right,
        (Value::BigInteger(left), Value::BigInteger(right)) => left == right,
        (Value::Rational(left), Value::Rational(right)) => left == right,
        (Value::BigRational(left), Value::BigRational(right)) => left == right,
        #[expect(
            clippy::float_cmp,
            reason = "EQL requires exact floating-point equality"
        )]
        (Value::Float(left), Value::Float(right)) => left == right,
        (Value::Complex(left), Value::Complex(right)) => {
            eql_value(left.real(), right.real()) && eql_value(left.imaginary(), right.imaginary())
        }
        _ => false,
    };
    left.eq_value(right) || numeric_equal
}

pub fn equalp_value(left: &Value, right: &Value) -> bool {
    let mut visited = HashSet::new();
    let mut pending = vec![(left.clone(), right.clone())];
    while let Some((left, right)) = pending.pop() {
        if let (Ok(left), Ok(right)) = (number(&left), number(&right)) {
            if !numeric_equalp(&left, &right) {
                return false;
            }
            continue;
        }
        match (&left, &right) {
            (Value::String(left), Value::String(right)) => {
                if !left.eq_ignore_ascii_case(right) {
                    return false;
                }
            }
            (Value::Character(left), Value::Character(right)) => {
                if !left.eq_ignore_ascii_case(right) {
                    return false;
                }
            }
            (Value::Complex(left), Value::Complex(right)) => {
                pending.push((left.real().clone(), right.real().clone()));
                pending.push((left.imaginary().clone(), right.imaginary().clone()));
            }
            (Value::Cons(left), Value::Cons(right)) => {
                if visited.insert((left.identity(), right.identity())) {
                    pending.push((left.cdr(), right.cdr()));
                    pending.push((left.car(), right.car()));
                }
            }
            (Value::Vector(left), Value::Vector(right)) => {
                if !queue_elements(left, right, &mut visited, &mut pending) {
                    return false;
                }
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
                if ld != rd || !queue_elements(le, re, &mut visited, &mut pending) {
                    return false;
                }
            }
            _ if eql_value(&left, &right) => {}
            _ => return false,
        }
    }
    true
}

fn queue_elements(
    left: &crate::SharedElements,
    right: &crate::SharedElements,
    visited: &mut HashSet<(usize, usize)>,
    pending: &mut Vec<(Value, Value)>,
) -> bool {
    if left.sequence_len() != right.sequence_len() {
        return false;
    }
    if visited.insert((left.identity(), right.identity())) {
        pending.extend(
            left.visible_snapshot()
                .into_iter()
                .zip(right.visible_snapshot()),
        );
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cons_cycles_comparison_internal_safety() {
        for through_car in [false, true] {
            let left = Value::cons(Value::Integer(7), Value::Integer(7));
            let right = Value::cons(Value::Integer(7), Value::Integer(7));
            let (Value::Cons(lc), Value::Cons(rc)) = (&left, &right) else {
                unreachable!()
            };
            if through_car {
                lc.set_car(left.clone());
                rc.set_car(right.clone());
            } else {
                lc.set_cdr(left.clone());
                rc.set_cdr(right.clone());
            }
            assert!(!left.eq_value(&right));
            assert!(left.equal_value(&right));
            assert!(equalp_value(&left, &right));
            if through_car {
                rc.set_cdr(Value::Integer(8));
            } else {
                rc.set_car(Value::Integer(8));
            }
            assert!(!left.equal_value(&right));
            assert!(!equalp_value(&left, &right));
            lc.set_car(Value::Nil);
            lc.set_cdr(Value::Nil);
            rc.set_car(Value::Nil);
            rc.set_cdr(Value::Nil);
        }
    }

    #[test]
    fn mixed_cons_condition_cycles_equal_internal_safety() {
        let left = Value::cons(Value::Nil, Value::Integer(7));
        let right = Value::cons(Value::Nil, Value::Integer(7));
        let (Value::Cons(lc), Value::Cons(rc)) = (&left, &right) else {
            unreachable!()
        };
        lc.set_car(Value::condition_from_parts(
            "ERROR".into(),
            "cycle".into(),
            None,
            vec![left.clone()],
        ));
        rc.set_car(Value::condition_from_parts(
            "ERROR".into(),
            "cycle".into(),
            None,
            vec![right.clone()],
        ));
        assert!(left.equal_value(&right));
        rc.set_cdr(Value::Integer(8));
        assert!(!left.equal_value(&right));
        lc.set_car(Value::Nil);
        rc.set_car(Value::Nil);
    }

    #[test]
    fn mixed_cons_vector_cycles_equalp_internal_safety() {
        let left = Value::cons(Value::Nil, Value::Integer(7));
        let right = Value::cons(Value::Nil, Value::Integer(7));
        let (Value::Cons(lc), Value::Cons(rc)) = (&left, &right) else {
            unreachable!()
        };
        lc.set_car(Value::vector(vec![left.clone()]));
        rc.set_car(Value::vector(vec![right.clone()]));
        assert!(equalp_value(&left, &right));
        rc.set_cdr(Value::Integer(8));
        assert!(!equalp_value(&left, &right));
        lc.set_car(Value::Nil);
        rc.set_car(Value::Nil);
    }

    #[test]
    fn shared_cycles_equalp_internal_safety() {
        for array in [false, true] {
            let make = || {
                if array {
                    Value::array(vec![1, 2], vec![Value::Nil, Value::Integer(7)])
                } else {
                    Value::vector(vec![Value::Nil, Value::Integer(7)])
                }
            };
            let left = make();
            let right = make();
            let storage = |value: &Value| match value {
                Value::Vector(items)
                | Value::Array {
                    elements: items, ..
                } => items.clone(),
                _ => unreachable!(),
            };
            let ls = storage(&left);
            let rs = storage(&right);
            assert!(equalp_value(&left, &right));
            assert!(ls.set(0, left.clone()));
            assert!(rs.set(0, right.clone()));
            assert!(equalp_value(&left, &right));
            assert!(!left.equal_value(&right));
            assert!(rs.set(1, Value::Integer(8)));
            assert!(!equalp_value(&left, &right));
            assert!(ls.set(0, Value::Nil));
            assert!(rs.set(0, Value::Nil));
        }
    }

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
