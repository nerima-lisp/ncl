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
        exact(arguments, "log", 1)?;
        return match &arguments[0] {
            Value::Complex(value) => {
                let real = number_argument("log", &value.real)?.as_float();
                let imag = number_argument("log", &value.imag)?.as_float();
                Ok(Value::complex(
                    Value::Float(real.hypot(imag).ln()),
                    Value::Float(imag.atan2(real)),
                ))
            }
            value => Ok(Value::Float(number_argument("log", value)?.as_float().ln())),
        };
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

    #[test]
    fn logarithm_returns_the_complex_principal_value() {
        let result = logarithm(&[Value::complex(Value::Integer(-1), Value::Integer(0))])
            .unwrap()
            .to_string();
        assert_eq!(result, "#C(0.0 3.141592653589793)");
    }
}
