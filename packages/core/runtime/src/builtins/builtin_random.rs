use std::cell::RefCell;
use std::rc::Rc;

use crate::value::RandomState;
use crate::{RuntimeError, Value};

mod helpers;
mod sampling;

use helpers::{arity, exact, type_error};
use sampling::{random_limit, state_reference};

thread_local! {
    static DEFAULT_RANDOM_STATE: Rc<RefCell<RandomState>> =
        Rc::new(RefCell::new(RandomState::seeded()));
    static DYNAMIC_RANDOM_STATES: RefCell<Vec<Rc<RefCell<RandomState>>>> = RefCell::new(Vec::new());
}

pub fn default_random_state_value() -> Value {
    DEFAULT_RANDOM_STATE.with(|state| Value::random_state_from_reference(Rc::clone(state)))
}

pub(crate) fn dynamic_random_state_depth() -> usize {
    DYNAMIC_RANDOM_STATES.with(|states| states.borrow().len())
}

pub(crate) fn bind_dynamic_random_state(value: &Value) {
    if let Value::RandomState(state) = value {
        DYNAMIC_RANDOM_STATES.with(|states| states.borrow_mut().push(Rc::clone(state)));
    }
}

pub(crate) fn truncate_dynamic_random_states(depth: usize) {
    DYNAMIC_RANDOM_STATES.with(|states| states.borrow_mut().truncate(depth));
}

pub fn random(arguments: &[Value]) -> Result<Value, RuntimeError> {
    DYNAMIC_RANDOM_STATES.with(|states| {
        if let Some(state) = states.borrow().last() {
            random_with_state(arguments, state)
        } else {
            DEFAULT_RANDOM_STATE.with(|state| random_with_state(arguments, state))
        }
    })
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
    DYNAMIC_RANDOM_STATES.with(|states| {
        if let Some(state) = states.borrow().last() {
            make_random_state_with_state(arguments, state)
        } else {
            DEFAULT_RANDOM_STATE.with(|state| make_random_state_with_state(arguments, state))
        }
    })
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

    #[test]
    fn random_rejects_a_second_argument_that_is_not_a_random_state() {
        assert!(random(&[Value::Integer(10), Value::Integer(1)]).is_err());
    }
}
