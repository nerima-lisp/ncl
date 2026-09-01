use super::{RuntimeError, Value, exact, number_argument};

fn unary_real(function: &str, arguments: &[Value], operation: impl FnOnce(f64) -> f64) -> Result<Value, RuntimeError> {
    exact(arguments, function, 1)?;
    Ok(Value::Float(operation(number_argument(function, &arguments[0])?.as_float())))
}

pub fn sine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    unary_complex("sin", arguments, |real, imag| {
        (real.sin() * imag.cosh(), real.cos() * imag.sinh())
    })
}

pub fn cosine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    unary_complex("cos", arguments, |real, imag| {
        (real.cos() * imag.cosh(), -real.sin() * imag.sinh())
    })
}

pub fn tangent(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "tan", 1)?;
    let (real, imag, complex) = match &arguments[0] {
        Value::Complex(value) => (
            number_argument("tan", &value.real)?.as_float(),
            number_argument("tan", &value.imag)?.as_float(),
            true,
        ),
        value => (number_argument("tan", value)?.as_float(), 0.0, false),
    };
    let denominator = (2.0 * real).cos() + (2.0 * imag).cosh();
    let real = (2.0 * real).sin() / denominator;
    let imag = (2.0 * imag).sinh() / denominator;
    Ok(if complex {
        Value::complex(Value::Float(real), Value::Float(imag))
    } else {
        Value::Float(real)
    })
}

fn unary_complex(
    function: &str,
    arguments: &[Value],
    operation: impl FnOnce(f64, f64) -> (f64, f64),
) -> Result<Value, RuntimeError> {
    exact(arguments, function, 1)?;
    match &arguments[0] {
        Value::Complex(value) => {
            let real = number_argument(function, &value.real)?.as_float();
            let imag = number_argument(function, &value.imag)?.as_float();
            let (real, imag) = operation(real, imag);
            Ok(Value::complex(Value::Float(real), Value::Float(imag)))
        }
        value => Ok(Value::Float(operation(number_argument(function, value)?.as_float(), 0.0).0)),
    }
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

    #[test]
    fn trigonometric_functions_return_complex_values() {
        let value = Value::complex(Value::Integer(0), Value::Integer(1));
        assert_eq!(
            sine(std::slice::from_ref(&value)).unwrap().to_string(),
            "#C(0.0 1.1752011936438014)"
        );
        assert_eq!(
            cosine(std::slice::from_ref(&value)).unwrap().to_string(),
            "#C(1.5430806348152437 -0.0)"
        );
        assert_eq!(
            tangent(&[value]).unwrap().to_string(),
            "#C(0.0 0.7615941559557649)"
        );
    }
}
