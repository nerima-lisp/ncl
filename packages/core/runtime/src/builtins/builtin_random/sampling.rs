use std::cell::RefCell;
use std::rc::Rc;

use super::helpers::type_error;
use crate::value::RandomState;
use crate::{RuntimeError, Value};

pub(super) fn random_limit(
    limit: &Value,
    state: &Rc<RefCell<RandomState>>,
) -> Result<Value, RuntimeError> {
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

pub(super) fn state_reference(
    function: &str,
    value: &Value,
) -> Result<Rc<RefCell<RandomState>>, RuntimeError> {
    value
        .random_state_reference()
        .ok_or_else(|| type_error(function, "a random-state", value))
}
