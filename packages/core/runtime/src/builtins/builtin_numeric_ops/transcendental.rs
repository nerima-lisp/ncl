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

pub fn logarithm(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() == 1 {
        return unary_real("log", arguments, f64::ln);
    }
    exact(arguments, "log", 2)?;
    let value = number_argument("log", &arguments[0])?.as_float();
    let base = number_argument("log", &arguments[1])?.as_float();
    Ok(Value::Float(value.ln() / base.ln()))
}

pub fn arc_sine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    unary_real("asin", arguments, f64::asin)
}

pub fn arc_cosine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    unary_real("acos", arguments, f64::acos)
}

pub fn arc_tangent(arguments: &[Value]) -> Result<Value, RuntimeError> {
    unary_real("atan", arguments, f64::atan)
}

pub fn hyperbolic_sine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    unary_real("sinh", arguments, f64::sinh)
}

pub fn hyperbolic_cosine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    unary_real("cosh", arguments, f64::cosh)
}

pub fn hyperbolic_tangent(arguments: &[Value]) -> Result<Value, RuntimeError> {
    unary_real("tanh", arguments, f64::tanh)
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
        assert_eq!(logarithm(&[Value::Integer(1)]).unwrap().to_string(), "0.0");
        assert_eq!(
            logarithm(&[Value::Integer(8), Value::Integer(2)])
                .unwrap()
                .to_string(),
            "3.0"
        );
    }
}
