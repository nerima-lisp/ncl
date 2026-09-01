use super::{RuntimeError, Value, exact, number_argument};

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
    unary_complex("exp", arguments, |real, imag| {
        let magnitude = real.exp();
        (magnitude * imag.cos(), magnitude * imag.sin())
    })
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
    exact(arguments, "asin", 1)?;
    match complex_argument("asin", &arguments[0])? {
        Some((real, imag)) => {
            let (square_real, square_imag) = complex_multiply(real, imag, real, imag);
            let (root_real, root_imag) = complex_sqrt(1.0 - square_real, -square_imag);
            let (log_real, log_imag) = complex_log(root_real - imag, root_imag + real);
            Ok(Value::complex(Value::Float(log_imag), Value::Float(-log_real)))
        }
        None => Ok(Value::Float(number_argument("asin", &arguments[0])?.as_float().asin())),
    }
}

pub fn arc_cosine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "acos", 1)?;
    match complex_argument("acos", &arguments[0])? {
        Some((real, imag)) => {
            let asin = arc_sine(&[Value::complex(Value::Float(real), Value::Float(imag))])?;
            let Value::Complex(value) = asin else { unreachable!() };
            Ok(Value::complex(
                Value::Float(std::f64::consts::FRAC_PI_2 - number_argument("acos", &value.real)?.as_float()),
                Value::Float(-number_argument("acos", &value.imag)?.as_float()),
            ))
        }
        None => Ok(Value::Float(number_argument("acos", &arguments[0])?.as_float().acos())),
    }
}

pub fn arc_tangent(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "atan", 1)?;
    match complex_argument("atan", &arguments[0])? {
        Some((real, imag)) => {
            let (left_real, left_imag) = complex_log(1.0 + imag, -real);
            let (right_real, right_imag) = complex_log(1.0 - imag, real);
            Ok(Value::complex(
                Value::Float((right_imag - left_imag) / 2.0),
                Value::Float((left_real - right_real) / 2.0),
            ))
        }
        None => Ok(Value::Float(number_argument("atan", &arguments[0])?.as_float().atan())),
    }
}

fn complex_argument(function: &str, value: &Value) -> Result<Option<(f64, f64)>, RuntimeError> {
    match value {
        Value::Complex(value) => Ok(Some((
            number_argument(function, &value.real)?.as_float(),
            number_argument(function, &value.imag)?.as_float(),
        ))),
        _ => Ok(None),
    }
}

fn complex_multiply(left_real: f64, left_imag: f64, right_real: f64, right_imag: f64) -> (f64, f64) {
    (left_real * right_real - left_imag * right_imag, left_real * right_imag + left_imag * right_real)
}

fn complex_sqrt(real: f64, imag: f64) -> (f64, f64) {
    let magnitude = real.hypot(imag);
    let root_real = ((magnitude + real) / 2.0).sqrt();
    let root_imag = imag.signum() * ((magnitude - real) / 2.0).sqrt();
    (root_real, root_imag)
}

fn complex_log(real: f64, imag: f64) -> (f64, f64) {
    (real.hypot(imag).ln(), imag.atan2(real))
}

pub fn hyperbolic_sine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    unary_complex("sinh", arguments, |real, imag| {
        (real.sinh() * imag.cos(), real.cosh() * imag.sin())
    })
}

pub fn hyperbolic_cosine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    unary_complex("cosh", arguments, |real, imag| {
        (real.cosh() * imag.cos(), real.sinh() * imag.sin())
    })
}

pub fn hyperbolic_tangent(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "tanh", 1)?;
    let (real, imag, complex) = match &arguments[0] {
        Value::Complex(value) => (
            number_argument("tanh", &value.real)?.as_float(),
            number_argument("tanh", &value.imag)?.as_float(),
            true,
        ),
        value => (number_argument("tanh", value)?.as_float(), 0.0, false),
    };
    let denominator = (2.0 * real).cosh() + (2.0 * imag).cos();
    let real = (2.0 * real).sinh() / denominator;
    let imag = (2.0 * imag).sin() / denominator;
    Ok(if complex {
        Value::complex(Value::Float(real), Value::Float(imag))
    } else {
        Value::Float(real)
    })
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

    #[test]
    fn exponential_and_hyperbolic_functions_return_complex_values() {
        let value = Value::complex(Value::Integer(0), Value::Integer(1));
        assert_eq!(
            exponential(std::slice::from_ref(&value)).unwrap().to_string(),
            "#C(0.5403023058681398 0.8414709848078965)"
        );
        assert_eq!(
            hyperbolic_sine(std::slice::from_ref(&value))
                .unwrap()
                .to_string(),
            "#C(0.0 0.8414709848078965)"
        );
        assert_eq!(
            hyperbolic_cosine(std::slice::from_ref(&value))
                .unwrap()
                .to_string(),
            "#C(0.5403023058681398 0.0)"
        );
        assert_eq!(
            hyperbolic_tangent(&[value]).unwrap().to_string(),
            "#C(0.0 1.557407724654902)"
        );
    }

    #[test]
    fn inverse_trigonometric_functions_return_complex_principal_values() {
        let value = Value::complex(Value::Integer(0), Value::Integer(1));
        assert_eq!(
            arc_sine(std::slice::from_ref(&value)).unwrap().to_string(),
            "#C(0.0 0.8813735870195428)"
        );
        assert_eq!(
            arc_cosine(std::slice::from_ref(&value)).unwrap().to_string(),
            "#C(1.5707963267948966 -0.8813735870195428)"
        );
        assert_eq!(
            arc_tangent(&[Value::complex(Value::Integer(0), Value::Integer(0))])
                .unwrap()
                .to_string(),
            "#C(0.0 0.0)"
        );
    }
}
