use std::cell::RefCell;
use std::rc::Rc;

use crate::value::RandomState;
use crate::{RuntimeError, Value};

thread_local! {
    static DEFAULT_RANDOM_STATE: Rc<RefCell<RandomState>> =
        Rc::new(RefCell::new(RandomState::seeded()));
}

pub(crate) fn random(arguments: &[Value]) -> Result<Value, RuntimeError> {
    DEFAULT_RANDOM_STATE.with(|state| random_with_state(arguments, state))
}

pub(crate) fn random_with_state(
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

pub(crate) fn make_random_state(arguments: &[Value]) -> Result<Value, RuntimeError> {
    DEFAULT_RANDOM_STATE.with(|state| make_random_state_with_state(arguments, state))
}

pub(crate) fn make_random_state_with_state(
    arguments: &[Value],
    default_state: &Rc<RefCell<RandomState>>,
) -> Result<Value, RuntimeError> {
    if arguments.len() > 1 {
        return Err(arity("make-random-state", "zero or one", arguments.len()));
    }
    let state = match arguments.first() {
        None | Some(Value::Nil) | Some(Value::Boolean(false)) => default_state.borrow().clone(),
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

pub(crate) fn random_state_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "random-state-p", 1)?;
    Ok(Value::boolean(matches!(
        arguments[0],
        Value::RandomState(_)
    )))
}

fn random_limit(limit: &Value, state: &Rc<RefCell<RandomState>>) -> Result<Value, RuntimeError> {
    match limit {
        Value::Integer(limit) if *limit > 0 => {
            let value = bounded_u64(state, *limit as u64);
            Ok(Value::Integer(value as i64))
        }
        Value::Float(limit) if limit.is_finite() && *limit > 0.0 => {
            let sample = state.borrow_mut().next_u64();
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
