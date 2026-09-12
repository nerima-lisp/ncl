use std::time::Duration;

use super::builtin_helpers::{exact, type_error};
use super::numbers::number_argument;
use super::*;

pub(crate) fn sleep(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "sleep", 1)?;
    let seconds = match &arguments[0] {
        Value::Complex(_) => return Err(type_error("sleep", "a real number", &arguments[0])),
        value => number_argument("sleep", value)?.as_float(),
    };

    if !seconds.is_finite() {
        return Err(RuntimeError::InvalidForm {
            message: "sleep requires a finite real number".to_string(),
            span: None,
        });
    }
    if seconds < 0.0 {
        return Err(RuntimeError::InvalidForm {
            message: "sleep requires a non-negative real number".to_string(),
            span: None,
        });
    }

    let duration = Duration::try_from_secs_f64(seconds).map_err(|_| RuntimeError::InvalidForm {
        message: "sleep duration is too large".to_string(),
        span: None,
    })?;
    std::thread::sleep(duration);
    Ok(Value::Nil)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sleep_requires_one_argument() {
        assert!(matches!(sleep(&[]), Err(RuntimeError::Arity { .. })));
    }

    #[test]
    fn sleep_rejects_negative_seconds() {
        assert!(matches!(
            sleep(&[Value::Integer(-1)]),
            Err(RuntimeError::InvalidForm { .. })
        ));
    }

    #[test]
    fn sleep_rejects_non_real_values() {
        assert!(matches!(
            sleep(&[Value::Nil]),
            Err(RuntimeError::Type { .. })
        ));
    }

    #[test]
    fn sleep_rejects_non_finite_seconds() {
        assert!(matches!(
            sleep(&[Value::Float(f64::NAN)]),
            Err(RuntimeError::InvalidForm { .. })
        ));
    }

    #[test]
    fn sleep_rejects_too_large_seconds() {
        assert!(matches!(
            sleep(&[Value::Float(f64::MAX)]),
            Err(RuntimeError::InvalidForm { .. })
        ));
    }
}
