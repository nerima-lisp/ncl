use std::cell::RefCell;
use std::rc::Rc;

use crate::value::RandomState;
use crate::{RuntimeError, Value};

thread_local! {
    static DEFAULT_RANDOM_STATE: Rc<RefCell<RandomState>> =
        Rc::new(RefCell::new(RandomState::seeded()));
}

pub fn random(arguments: &[Value]) -> Result<Value, RuntimeError> {
    DEFAULT_RANDOM_STATE.with(|state| random_with_state(arguments, state))
}

pub fn random_with_state(
    arguments: &[Value],
    default_state: &Rc<RefCell<RandomState>>,
) -> Result<Value, RuntimeError> {
    if arguments.len() != 1 && arguments.len() != 2 {
        return Err(arity("random", "one or two", arguments.len()));
    }
    let state = match arguments.get(1) {
        Some(value) => state_reference("random", value)?,
        None => Rc::clone(default_state),
    };
    random_limit(&arguments[0], &state)
}

pub fn make_random_state(arguments: &[Value]) -> Result<Value, RuntimeError> {
    DEFAULT_RANDOM_STATE.with(|state| make_random_state_with_state(arguments, state))
}

pub fn make_random_state_with_state(
    arguments: &[Value],
    default_state: &Rc<RefCell<RandomState>>,
) -> Result<Value, RuntimeError> {
    if arguments.len() > 1 {
        return Err(arity("make-random-state", "zero or one", arguments.len()));
    }
    let state = match arguments.first() {
        None | Some(Value::Nil | Value::Boolean(false)) => default_state.borrow().clone(),
        Some(Value::Boolean(true)) => RandomState::seeded(),
        Some(Value::RandomState(state)) => state.borrow().clone(),
        Some(value) => {
            return Err(type_error(
                "make-random-state",
                "a random-state, NIL, or T",
                value,
            ));
        }
    };
    Ok(Value::random_state(state))
}

pub fn random_state_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "random-state-p", 1)?;
    Ok(Value::boolean(matches!(
        arguments[0],
        Value::RandomState(_)
    )))
}

fn random_limit(limit: &Value, state: &Rc<RefCell<RandomState>>) -> Result<Value, RuntimeError> {
    match limit {
        Value::Integer(limit) if *limit > 0 => {
            let value = bounded_u64(state, limit.cast_unsigned());
            Ok(Value::Integer(value.cast_signed()))
        }
        Value::Float(limit) if limit.is_finite() && *limit > 0.0 => {
            let sample = state.borrow_mut().next_u64();
            // Deliberately keep only the top 53 bits: an f64 mantissa can't
            // hold more, and this is the standard technique for mapping a
            // 64-bit sample onto a uniform float in [0, 1).
            #[allow(clippy::cast_precision_loss)]
            let unit = (sample >> 11) as f64 / 9_007_199_254_740_992.0;
            Ok(Value::Float(unit * *limit))
        }
        value => Err(type_error(
            "random",
            "a positive integer or positive float",
            value,
        )),
    }
}

fn bounded_u64(state: &Rc<RefCell<RandomState>>, bound: u64) -> u64 {
    let threshold = bound.wrapping_neg() % bound;
    loop {
        let sample = state.borrow_mut().next_u64();
        if sample >= threshold {
            return sample % bound;
        }
    }
}

fn state_reference(
    function: &str,
    value: &Value,
) -> Result<Rc<RefCell<RandomState>>, RuntimeError> {
    value
        .random_state_reference()
        .ok_or_else(|| type_error(function, "a random-state", value))
}

fn exact(arguments: &[Value], function: &str, expected: usize) -> Result<(), RuntimeError> {
    if arguments.len() == expected {
        Ok(())
    } else {
        Err(arity(function, expected.to_string(), arguments.len()))
    }
}

fn arity(function: &str, expected: impl Into<String>, actual: usize) -> RuntimeError {
    RuntimeError::Arity {
        function: function.to_string(),
        expected: expected.into(),
        actual,
    }
}

fn type_error(function: &str, expected: &str, value: &Value) -> RuntimeError {
    RuntimeError::Type {
        expected: format!("{function} requires {expected}"),
        actual: value.type_name().to_string(),
        span: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{make_random_state, random, random_state_p};
    use crate::Value;

    #[test]
    fn random_rejects_wrong_arity_and_non_positive_limits() {
        assert!(random(&[]).is_err());
        assert!(random(&[Value::Integer(1), Value::Integer(2), Value::Integer(3)]).is_err());
        assert!(random(&[Value::Integer(0)]).is_err());
        assert!(random(&[Value::Integer(-5)]).is_err());
        assert!(random(&[Value::Float(0.0)]).is_err());
        assert!(random(&[Value::String("nope".into())]).is_err());
    }

    #[test]
    fn random_integer_stays_within_the_exclusive_upper_bound() {
        for _ in 0..200 {
            let Ok(Value::Integer(value)) = random(&[Value::Integer(10)]) else {
                panic!("random did not return an integer");
            };
            assert!((0..10).contains(&value), "value out of range: {value}");
        }
    }

    #[test]
    fn random_float_stays_within_the_exclusive_upper_bound() {
        for _ in 0..200 {
            let Ok(Value::Float(value)) = random(&[Value::Float(2.5)]) else {
                panic!("random did not return a float");
            };
            assert!((0.0..2.5).contains(&value), "value out of range: {value}");
        }
    }

    #[test]
    fn random_state_p_recognizes_only_random_states() {
        let Ok(state) = make_random_state(&[]) else {
            panic!("make-random-state failed");
        };
        assert!(matches!(random_state_p(&[state]), Ok(Value::Boolean(true))));
        assert!(matches!(
            random_state_p(&[Value::Integer(1)]),
            Ok(Value::Nil)
        ));
    }

    #[test]
    fn make_random_state_from_an_existing_state_reproduces_its_sequence() {
        let Ok(seed) = make_random_state(&[Value::boolean(true)]) else {
            panic!("make-random-state failed");
        };
        let Ok(copy) = make_random_state(std::slice::from_ref(&seed)) else {
            panic!("make-random-state failed");
        };

        let Ok(Value::Integer(from_seed)) = random(&[Value::Integer(1_000_000_000), seed]) else {
            panic!("random did not return an integer");
        };
        let Ok(Value::Integer(from_copy)) = random(&[Value::Integer(1_000_000_000), copy]) else {
            panic!("random did not return an integer");
        };
        assert_eq!(from_seed, from_copy);
    }

    #[test]
    fn make_random_state_rejects_wrong_arity_and_type() {
        assert!(make_random_state(&[Value::Integer(1), Value::Integer(2)]).is_err());
        assert!(make_random_state(&[Value::Integer(1)]).is_err());
    }
}
