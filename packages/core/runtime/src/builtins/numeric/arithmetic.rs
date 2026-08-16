fn add(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = Numeric::Real(Number::Integer(0));
    for argument in arguments {
        result = add_numeric(result, numeric_argument("+", argument)?)?;
    }
    numeric_to_value(result)
}

fn subtract(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("-", "at least one", 0));
    }
    let values = arguments
        .iter()
        .map(|value| numeric_argument("-", value))
        .collect::<Result<Vec<_>, _>>()?;
    let mut result = values[0];
    if values.len() == 1 {
        result = negate_numeric(result)?;
    } else {
        for value in &values[1..] {
            result = subtract_numeric(result, *value)?;
        }
    }
    numeric_to_value(result)
}

fn multiply(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = Numeric::Real(Number::Integer(1));
    for argument in arguments {
        result = multiply_numeric(result, numeric_argument("*", argument)?)?;
    }
    numeric_to_value(result)
}

fn divide(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("/", "at least one", 0));
    }
    let values = arguments
        .iter()
        .map(|value| numeric_argument("/", value))
        .collect::<Result<Vec<_>, _>>()?;
    let mut result;
    if values.len() == 1 {
        result = divide_numeric(Numeric::Real(Number::Integer(1)), values[0])?;
    } else {
        result = values[0];
        for value in &values[1..] {
            result = divide_numeric(result, *value)?;
        }
    }
    numeric_to_value(result)
}

fn exponentiate(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "expt", 2)?;
    let base = numeric_argument("expt", &arguments[0])?;
    let exponent = numeric_argument("expt", &arguments[1])?;

    if let (Numeric::Real(base), Numeric::Real(exponent)) = (base, exponent) {
        if !base.is_float()
            && let Some((exponent_numerator, exponent_denominator)) = exponent.exact_parts()
            && exponent_denominator == 1
        {
            return number_to_value(exact_power(base, exponent_numerator)?);
        }

        if base.as_float() >= 0.0 || float_is_integer(exponent.as_float()) {
            return Ok(Value::Float(base.as_float().powf(exponent.as_float())));
        }

        return exponentiate_complex(Numeric::Real(base), Numeric::Real(exponent));
    }

    exponentiate_complex(base, exponent)
}

fn exponentiate_complex(base: Numeric, exponent: Numeric) -> Result<Value, RuntimeError> {
    let (base_real, base_imag) = base.into_complex();
    let (exponent_real, exponent_imag) = exponent.into_complex();

    if base_real.as_float() == 0.0 && base_imag.as_float() == 0.0 {
        return zero_power(exponent_real, exponent_imag);
    }

    let base_real = base_real.as_float();
    let base_imag = base_imag.as_float();
    let exponent_real = exponent_real.as_float();
    let exponent_imag = exponent_imag.as_float();

    let magnitude = base_real.hypot(base_imag);
    let angle = base_imag.atan2(base_real);
    let log_real = magnitude.ln();
    let log_imag = angle;

    let power_real = exponent_real * log_real - exponent_imag * log_imag;
    let power_imag = exponent_real * log_imag + exponent_imag * log_real;
    let scale = power_real.exp();
    let real_part = canonicalize_float(scale * power_imag.cos());
    let imag_part = canonicalize_float(scale * power_imag.sin());

    Ok(Value::complex(
        number_to_value(real_part)?,
        number_to_value(imag_part)?,
    ))
}

fn sine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "sin", 1)?;
    numeric_to_value(sine_numeric(numeric_argument("sin", &arguments[0])?))
}

fn sine_numeric(value: Numeric) -> Numeric {
    match value {
        Numeric::Real(value) => Numeric::Real(Number::Float(value.as_float().sin())),
        Numeric::Complex { real, imag } => {
            let real_part = canonicalize_float(real.as_float().sin() * imag.as_float().cosh());
            let imag_part = canonicalize_float(real.as_float().cos() * imag.as_float().sinh());
            if imag_part.as_float() == 0.0 {
                Numeric::Real(real_part)
            } else {
                Numeric::Complex {
                    real: real_part,
                    imag: imag_part,
                }
            }
        }
    }
}

fn cosine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "cos", 1)?;
    numeric_to_value(cosine_numeric(numeric_argument("cos", &arguments[0])?))
}

fn cosine_numeric(value: Numeric) -> Numeric {
    match value {
        Numeric::Real(value) => Numeric::Real(Number::Float(value.as_float().cos())),
        Numeric::Complex { real, imag } => {
            let real_part = canonicalize_float(real.as_float().cos() * imag.as_float().cosh());
            let imag_part = canonicalize_float(-(real.as_float().sin() * imag.as_float().sinh()));
            if imag_part.as_float() == 0.0 {
                Numeric::Real(real_part)
            } else {
                Numeric::Complex {
                    real: real_part,
                    imag: imag_part,
                }
            }
        }
    }
}

fn tangent(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "tan", 1)?;
    let value = numeric_argument("tan", &arguments[0])?;
    numeric_to_value(divide_numeric(sine_numeric(value), cosine_numeric(value))?)
}

fn arc_sine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "asin", 1)?;
    numeric_to_value(arc_sine_numeric(numeric_argument("asin", &arguments[0])?)?)
}

fn arc_sine_numeric(value: Numeric) -> Result<Numeric, RuntimeError> {
    let imaginary_unit = Numeric::Complex {
        real: Number::Integer(0),
        imag: Number::Integer(1),
    };
    let one = Numeric::Real(Number::Integer(1));
    let negative_imaginary_unit = Numeric::Complex {
        real: Number::Integer(0),
        imag: Number::Integer(-1),
    };
    let value_squared = multiply_numeric(value, value)?;
    let radicand = subtract_numeric(one, value_squared)?;
    let root = square_root_numeric(radicand)?;
    let sum = add_numeric(multiply_numeric(imaginary_unit, value)?, root)?;

    Ok(canonicalize_numeric(multiply_numeric(
        negative_imaginary_unit,
        logarithm_numeric(sum),
    )?))
}

fn arc_cosine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "acos", 1)?;
    numeric_to_value(arc_cosine_numeric(numeric_argument(
        "acos",
        &arguments[0],
    )?)?)
}

fn arc_cosine_numeric(value: Numeric) -> Result<Numeric, RuntimeError> {
    Ok(canonicalize_numeric(subtract_numeric(
        Numeric::Real(Number::Float(PI / 2.0)),
        arc_sine_numeric(value)?,
    )?))
}

fn arc_tangent(arguments: &[Value]) -> Result<Value, RuntimeError> {
    match arguments.len() {
        1 => numeric_to_value(arc_tangent_numeric(numeric_argument(
            "atan",
            &arguments[0],
        )?)?),
        2 => {
            let y = number_argument("atan", &real_number_argument("atan", &arguments[0])?)?;
            let x = number_argument("atan", &real_number_argument("atan", &arguments[1])?)?;
            number_to_value(arc_tangent_real(y, x))
        }
        _ => Err(arity("atan", "1 or 2", arguments.len())),
    }
}

fn arc_tangent_real(y: Number, x: Number) -> Number {
    canonicalize_number(Number::Float(y.as_float().atan2(x.as_float())))
}

fn arc_tangent_numeric(value: Numeric) -> Result<Numeric, RuntimeError> {
    let imaginary_unit = Numeric::Complex {
        real: Number::Integer(0),
        imag: Number::Integer(1),
    };
    let one = Numeric::Real(Number::Integer(1));
    let difference = subtract_numeric(
        logarithm_numeric(add_numeric(one, multiply_numeric(imaginary_unit, value)?)?),
        logarithm_numeric(subtract_numeric(
            one,
            multiply_numeric(imaginary_unit, value)?,
        )?),
    )?;

    Ok(canonicalize_numeric(multiply_numeric(
        Numeric::Complex {
            real: Number::Integer(0),
            imag: Number::Float(-0.5),
        },
        difference,
    )?))
}

fn hyperbolic_sine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "sinh", 1)?;
    numeric_to_value(hyperbolic_sine_numeric(numeric_argument(
        "sinh",
        &arguments[0],
    )?))
}

fn hyperbolic_sine_numeric(value: Numeric) -> Numeric {
    match value {
        Numeric::Real(value) => Numeric::Real(Number::Float(value.as_float().sinh())),
        Numeric::Complex { real, imag } => {
            let real_part = canonicalize_float(real.as_float().sinh() * imag.as_float().cos());
            let imag_part = canonicalize_float(real.as_float().cosh() * imag.as_float().sin());
            if imag_part.as_float() == 0.0 {
                Numeric::Real(real_part)
            } else {
                Numeric::Complex {
                    real: real_part,
                    imag: imag_part,
                }
            }
        }
    }
}

fn hyperbolic_cosine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "cosh", 1)?;
    numeric_to_value(hyperbolic_cosine_numeric(numeric_argument(
        "cosh",
        &arguments[0],
    )?))
}

fn hyperbolic_cosine_numeric(value: Numeric) -> Numeric {
    match value {
        Numeric::Real(value) => Numeric::Real(Number::Float(value.as_float().cosh())),
        Numeric::Complex { real, imag } => {
            let real_part = canonicalize_float(real.as_float().cosh() * imag.as_float().cos());
            let imag_part = canonicalize_float(real.as_float().sinh() * imag.as_float().sin());
            if imag_part.as_float() == 0.0 {
                Numeric::Real(real_part)
            } else {
                Numeric::Complex {
                    real: real_part,
                    imag: imag_part,
                }
            }
        }
    }
}

fn hyperbolic_tangent(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "tanh", 1)?;
    let value = numeric_argument("tanh", &arguments[0])?;
    numeric_to_value(divide_numeric(
        hyperbolic_sine_numeric(value),
        hyperbolic_cosine_numeric(value),
    )?)
}

fn arc_hyperbolic_sine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "asinh", 1)?;
    numeric_to_value(arc_hyperbolic_sine_numeric(numeric_argument(
        "asinh",
        &arguments[0],
    )?)?)
}

fn arc_hyperbolic_sine_numeric(value: Numeric) -> Result<Numeric, RuntimeError> {
    let one = Numeric::Real(Number::Integer(1));
    let value_squared = multiply_numeric(value, value)?;
    let radicand = add_numeric(one, value_squared)?;
    let root = square_root_numeric(radicand)?;
    let sum = add_numeric(value, root)?;
    Ok(canonicalize_numeric(logarithm_numeric(sum)))
}

fn arc_hyperbolic_cosine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "acosh", 1)?;
    numeric_to_value(arc_hyperbolic_cosine_numeric(numeric_argument(
        "acosh",
        &arguments[0],
    )?)?)
}

fn arc_hyperbolic_cosine_numeric(value: Numeric) -> Result<Numeric, RuntimeError> {
    let one = Numeric::Real(Number::Integer(1));
    let lower = square_root_numeric(subtract_numeric(value, one)?)?;
    let upper = square_root_numeric(add_numeric(value, one)?)?;
    let sum = add_numeric(value, multiply_numeric(lower, upper)?)?;
    Ok(canonicalize_numeric(logarithm_numeric(sum)))
}

fn arc_hyperbolic_tangent(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "atanh", 1)?;
    numeric_to_value(arc_hyperbolic_tangent_numeric(numeric_argument(
        "atanh",
        &arguments[0],
    )?)?)
}

fn arc_hyperbolic_tangent_numeric(value: Numeric) -> Result<Numeric, RuntimeError> {
    let one = Numeric::Real(Number::Integer(1));
    let numerator = logarithm_numeric(add_numeric(one, value)?);
    let denominator = logarithm_numeric(subtract_numeric(one, value)?);
    Ok(canonicalize_numeric(multiply_numeric(
        Numeric::Real(Number::Float(0.5)),
        subtract_numeric(numerator, denominator)?,
    )?))
}

fn exponential(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "exp", 1)?;
    match numeric_argument("exp", &arguments[0])? {
        Numeric::Real(value) => Ok(Value::Float(value.as_float().exp())),
        Numeric::Complex { real, imag } => {
            let scale = real.as_float().exp();
            let angle = imag.as_float();
            Ok(Value::complex(
                number_to_value(canonicalize_float(scale * angle.cos()))?,
                number_to_value(canonicalize_float(scale * angle.sin()))?,
            ))
        }
    }
}

fn logarithm(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("log", "1 or 2", arguments.len()));
    }

    let value = logarithm_numeric(numeric_argument("log", &arguments[0])?);
    if arguments.len() == 1 {
        return numeric_to_value(value);
    }

    let base = logarithm_numeric(numeric_argument("log", &arguments[1])?);
    numeric_to_value(divide_numeric(value, base)?)
}

fn logarithm_numeric(value: Numeric) -> Numeric {
    let (real, imag) = value.into_complex();
    let magnitude = real.as_float().hypot(imag.as_float());
    let angle = imag.as_float().atan2(real.as_float());
    let real_part = canonicalize_float(magnitude.ln());
    let imag_part = canonicalize_float(angle);

    if imag_part.as_float() == 0.0 {
        Numeric::Real(real_part)
    } else {
        Numeric::Complex {
            real: real_part,
            imag: imag_part,
        }
    }
}

fn cis(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "cis", 1)?;
    let angle = number_argument("cis", &arguments[0])?.as_float();
    Ok(Value::complex(
        number_to_value(canonicalize_float(angle.cos()))?,
        number_to_value(canonicalize_float(angle.sin()))?,
    ))
}

fn zero_power(exponent_real: Number, exponent_imag: Number) -> Result<Value, RuntimeError> {
    if exponent_imag.as_float() != 0.0 {
        return Err(RuntimeError::DivisionByZero);
    }

    if exponent_real.as_float() == 0.0 {
        return Ok(Value::Integer(1));
    }

    if exponent_real.as_float() < 0.0 {
        return Err(RuntimeError::DivisionByZero);
    }

    if let Some((exponent_numerator, exponent_denominator)) = exponent_real.exact_parts()
        && exponent_numerator > 0
        && exponent_denominator == 1
    {
        return Ok(Value::Integer(0));
    }

    Ok(Value::Float(0.0))
}

fn canonicalize_float(value: f64) -> Number {
    if value.abs() < 1e-12 {
        Number::Integer(0)
    } else {
        Number::Float(value)
    }
}

fn float_is_integer(value: f64) -> bool {
    value.is_finite() && value.fract() == 0.0
}

fn square_root(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "sqrt", 1)?;
    match numeric_argument("sqrt", &arguments[0])? {
        Numeric::Real(number) => square_root_real(number),
        Numeric::Complex { real, imag } => square_root_complex(real, imag),
    }
}

fn square_root_real(number: Number) -> Result<Value, RuntimeError> {
    match number {
        Number::Integer(value) if value >= 0 => {
            let root = integer_square_root(value as u128);
            if root * root == value as u128 {
                Ok(Value::Integer(root as i64))
            } else {
                Ok(Value::Float((value as f64).sqrt()))
            }
        }
        Number::Integer(value) => Ok(Value::complex(
            Value::Integer(0),
            square_root_real(Number::Integer(
                value.checked_neg().ok_or(RuntimeError::NumericOverflow)?,
            ))?,
        )),
        Number::Rational(value) if value.numerator() >= 0 => {
            let numerator = value.numerator() as u128;
            let denominator = value.denominator() as u128;
            let numerator_root = integer_square_root(numerator);
            let denominator_root = integer_square_root(denominator);
            if numerator_root * numerator_root == numerator
                && denominator_root * denominator_root == denominator
            {
                rational_number(numerator_root as i128, denominator_root as i128)
                    .and_then(number_to_value)
            } else {
                Ok(Value::Float(
                    (value.numerator() as f64 / value.denominator() as f64).sqrt(),
                ))
            }
        }
        Number::Rational(value) => Ok(Value::complex(
            Value::Integer(0),
            square_root_real(Number::Rational(Rational::new(
                -i128::from(value.numerator()),
                i128::from(value.denominator()),
            )?))?,
        )),
        Number::Float(value) if value >= 0.0 => Ok(Value::Float(value.sqrt())),
        Number::Float(value) => Ok(Value::complex(
            Value::Integer(0),
            Value::Float((-value).sqrt()),
        )),
    }
}

fn integer_square_root(value: u128) -> u128 {
    if value < 2 {
        return value;
    }
    let bits = 128 - value.leading_zeros();
    let mut root = 1u128 << (bits / 2 + 1);
    loop {
        let next = (root + value / root) / 2;
        if next >= root {
            return root;
        }
        root = next;
    }
}

fn square_root_complex(real: Number, imag: Number) -> Result<Value, RuntimeError> {
    let real = real.as_float();
    let imag = imag.as_float();
    let magnitude = real.hypot(imag);
    let real_part = ((magnitude + real) / 2.0).sqrt();
    let imag_magnitude = ((magnitude - real) / 2.0).sqrt();
    let imag_part = if imag < 0.0 {
        -imag_magnitude
    } else {
        imag_magnitude
    };

    Ok(Value::complex(
        Value::Float(real_part),
        Value::Float(imag_part),
    ))
}

fn signum(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "signum", 1)?;
    match numeric_argument("signum", &arguments[0])? {
        Numeric::Real(number) => signum_real(number),
        Numeric::Complex { real, imag } => signum_complex(real, imag),
    }
}

fn signum_real(number: Number) -> Result<Value, RuntimeError> {
    match number {
        Number::Integer(value) => Ok(Value::Integer(value.signum())),
        Number::Rational(value) => Ok(Value::Integer(value.numerator().signum())),
        Number::Float(value) if value.is_nan() => Err(RuntimeError::InvalidForm {
            message: "signum of NaN is undefined".to_owned(),
            span: None,
        }),
        Number::Float(value) if value == 0.0 => Ok(Value::Float(value)),
        Number::Float(value) => Ok(Value::Float(if value.is_sign_negative() {
            -1.0
        } else {
            1.0
        })),
    }
}

fn signum_complex(real: Number, imag: Number) -> Result<Value, RuntimeError> {
    if real.as_float() == 0.0 && imag.as_float() == 0.0 {
        return numeric_to_value(Numeric::Complex { real, imag });
    }

    let magnitude = absolute_complex(real, imag)?;
    let magnitude = numeric_argument("signum", &magnitude)?;
    let value = Numeric::Complex { real, imag };
    numeric_to_value(divide_numeric(value, magnitude)?)
}

fn float_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() || arguments.len() > 2 {
        return Err(arity("float", "1 to 2", arguments.len()));
    }
    let number = number_argument("float", &arguments[0])?;
    if let Some(prototype) = arguments.get(1)
        && !matches!(prototype, Value::Float(_))
    {
        return Err(type_error("float", "a float prototype", prototype));
    }
    Ok(Value::Float(number.as_float()))
}

