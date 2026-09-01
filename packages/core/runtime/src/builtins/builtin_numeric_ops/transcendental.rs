use super::{RuntimeError, Value, exact, number_argument};

fn unary_real(function: &str, arguments: &[Value], operation: impl FnOnce(f64) -> f64) -> Result<Value, RuntimeError> {
    exact(arguments, function, 1)?;
    Ok(Value::Float(operation(number_argument(function, &arguments[0])?.as_float())))
}

pub fn sine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    unary_real("sin", arguments, f64::sin)
}

pub fn cosine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    unary_real("cos", arguments, f64::cos)
}

pub fn tangent(arguments: &[Value]) -> Result<Value, RuntimeError> {
    unary_real("tan", arguments, f64::tan)
}

pub fn exponential(arguments: &[Value]) -> Result<Value, RuntimeError> {
    unary_real("exp", arguments, f64::exp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcendental_functions_coerce_exact_numbers_to_floats() {
        assert_eq!(sine(&[Value::Integer(0)]).unwrap().to_string(), "0.0");
        assert_eq!(
            exponential(&[Value::Integer(1)]).unwrap().to_string(),
            std::f64::consts::E.to_string()
        );
    }
}
