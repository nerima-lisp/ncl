macro_rules! numeric_builtins {
    () => {
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

fn rational(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rational", 1)?;
    match number_argument("rational", &arguments[0])? {
        Number::Integer(value) => Ok(Value::Integer(value)),
        Number::Rational(value) => Value::rational(
            i128::from(value.numerator()),
            i128::from(value.denominator()),
        ),
        Number::Float(value) => rational_from_float(value),
    }
}

fn rational_from_float(value: f64) -> Result<Value, RuntimeError> {
    if !value.is_finite() {
        return Err(RuntimeError::InvalidForm {
            message: "rational requires a finite real".to_owned(),
            span: None,
        });
    }
    if value == 0.0 {
        return Ok(Value::Integer(0));
    }

    const FRACTION_MASK: u64 = (1 << 52) - 1;
    let bits = value.to_bits();
    let negative = bits >> 63 != 0;
    let exponent_bits = ((bits >> 52) & 0x7ff) as i32;
    let mut significand = bits & FRACTION_MASK;
    let mut exponent = if exponent_bits == 0 {
        -1074
    } else {
        significand |= 1 << 52;
        exponent_bits - 1023 - 52
    };

    if exponent < 0 {
        let canceled = significand.trailing_zeros().min((-exponent) as u32);
        significand >>= canceled;
        exponent += canceled as i32;
    }

    let mut numerator = i128::from(significand);
    if negative {
        numerator = -numerator;
    }
    let denominator = if exponent >= 0 {
        numerator = numerator
            .checked_shl(exponent as u32)
            .ok_or(RuntimeError::NumericOverflow)?;
        1
    } else {
        1i128
            .checked_shl((-exponent) as u32)
            .ok_or(RuntimeError::NumericOverflow)?
    };
    Value::rational(numerator, denominator)
}

fn rationalize(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rationalize", 1)?;
    match number_argument("rationalize", &arguments[0])? {
        Number::Integer(value) => Ok(Value::Integer(value)),
        Number::Rational(value) => Value::rational(
            i128::from(value.numerator()),
            i128::from(value.denominator()),
        ),
        Number::Float(value) => rationalize_float(value),
    }
}

fn rationalize_float(value: f64) -> Result<Value, RuntimeError> {
    if !value.is_finite() {
        return Err(RuntimeError::InvalidForm {
            message: "rationalize requires a finite real".to_owned(),
            span: None,
        });
    }
    if value == 0.0 {
        return Ok(Value::Integer(0));
    }

    let tolerance = (value.abs() * f64::EPSILON / 2.0).max(f64::MIN_POSITIVE);
    let (numerator, denominator) = simplest_rational(value - tolerance, value + tolerance)?;
    number_to_value(rational_number(numerator, denominator)?)
}

fn simplest_rational(lower: f64, upper: f64) -> Result<(i128, i128), RuntimeError> {
    if !lower.is_finite() || !upper.is_finite() || lower > upper {
        return Err(RuntimeError::NumericOverflow);
    }
    if lower <= 0.0 && upper >= 0.0 {
        return Ok((0, 1));
    }
    if upper < 0.0 {
        let (numerator, denominator) = simplest_positive_rational(-upper, -lower, 0)?;
        return Ok((-numerator, denominator));
    }
    simplest_positive_rational(lower, upper, 0)
}

fn simplest_positive_rational(
    lower: f64,
    upper: f64,
    depth: u32,
) -> Result<(i128, i128), RuntimeError> {
    if depth > 128 || !lower.is_finite() || !upper.is_finite() || lower <= 0.0 || lower > upper {
        return Err(RuntimeError::NumericOverflow);
    }

    let lower_floor = lower.floor();
    let upper_floor = upper.floor();
    if lower == lower_floor {
        return Ok((lower_floor as i128, 1));
    }
    if lower_floor < upper_floor {
        return Ok(((lower_floor as i128) + 1, 1));
    }

    let lower_fraction = lower - lower_floor;
    let upper_fraction = upper - lower_floor;
    let (reciprocal_numerator, reciprocal_denominator) =
        simplest_positive_rational(1.0 / upper_fraction, 1.0 / lower_fraction, depth + 1)?;
    let numerator = (lower_floor as i128)
        .checked_mul(reciprocal_numerator)
        .and_then(|value| value.checked_add(reciprocal_denominator))
        .ok_or(RuntimeError::NumericOverflow)?;
    Ok((numerator, reciprocal_numerator))
}

fn exact_power(base: Number, exponent: i64) -> Result<Number, RuntimeError> {
    let (mut numerator, mut denominator) =
        base.exact_parts().expect("exact power received a float");
    let negative_exponent = exponent < 0;
    if negative_exponent && numerator == 0 {
        return Err(RuntimeError::DivisionByZero);
    }
    if negative_exponent {
        std::mem::swap(&mut numerator, &mut denominator);
    }

    let magnitude = exponent.unsigned_abs();
    rational_number(
        checked_power(i128::from(numerator), magnitude)?,
        checked_power(i128::from(denominator), magnitude)?,
    )
}

fn checked_power(base: i128, mut exponent: u64) -> Result<i128, RuntimeError> {
    let mut result = 1i128;
    let mut factor = base;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = result
                .checked_mul(factor)
                .ok_or(RuntimeError::NumericOverflow)?;
        }
        exponent >>= 1;
        if exponent != 0 {
            factor = factor
                .checked_mul(factor)
                .ok_or(RuntimeError::NumericOverflow)?;
        }
    }
    Ok(result)
}

fn numeric_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("=", "at least one", 0));
    }
    let values = arguments
        .iter()
        .map(|value| numeric_argument("=", value))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Value::boolean(
        values
            .windows(2)
            .all(|window| numeric_equal_values(window[0], window[1])),
    ))
}

fn less_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers("<", arguments, |ordering| ordering == Ordering::Less)
}

fn greater_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers(">", arguments, |ordering| ordering == Ordering::Greater)
}

fn less_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers("<=", arguments, |ordering| ordering != Ordering::Greater)
}

fn greater_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers(">=", arguments, |ordering| ordering != Ordering::Less)
}

fn zerop(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "zerop", 1)?;
    Ok(Value::boolean(
        match numeric_argument("zerop", &arguments[0])? {
            Numeric::Real(number) => number.as_float() == 0.0,
            Numeric::Complex { real, imag } => real.as_float() == 0.0 && imag.as_float() == 0.0,
        },
    ))
}

fn plusp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "plusp", 1)?;
    Ok(Value::boolean(
        number_argument("plusp", &arguments[0])?.as_float() > 0.0,
    ))
}

fn minusp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "minusp", 1)?;
    Ok(Value::boolean(
        number_argument("minusp", &arguments[0])?.as_float() < 0.0,
    ))
}

fn evenp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "evenp", 1)?;
    Ok(Value::boolean(
        integer_argument("evenp", &arguments[0])? % 2 == 0,
    ))
}

fn oddp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "oddp", 1)?;
    Ok(Value::boolean(
        integer_argument("oddp", &arguments[0])? % 2 != 0,
    ))
}

fn minimum(arguments: &[Value]) -> Result<Value, RuntimeError> {
    extreme(arguments, "min", true)
}

fn maximum(arguments: &[Value]) -> Result<Value, RuntimeError> {
    extreme(arguments, "max", false)
}

fn extreme(
    arguments: &[Value],
    function: &str,
    choose_minimum: bool,
) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity(function, "at least one", 0));
    }
    let values = arguments
        .iter()
        .map(|value| number_argument(function, value))
        .collect::<Result<Vec<_>, _>>()?;
    let mut result = values[0];
    for value in &values[1..] {
        let ordering = compare_number_values(*value, result);
        if (choose_minimum && ordering == Ordering::Less)
            || (!choose_minimum && ordering == Ordering::Greater)
        {
            result = *value;
        }
    }
    number_to_value(result)
}

fn absolute(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "abs", 1)?;
    match numeric_argument("abs", &arguments[0])? {
        Numeric::Real(number) => absolute_real(number),
        Numeric::Complex { real, imag } => absolute_complex(real, imag),
    }
}

fn absolute_real(number: Number) -> Result<Value, RuntimeError> {
    match number {
        Number::Integer(value) => value
            .checked_abs()
            .map(Value::Integer)
            .ok_or(RuntimeError::NumericOverflow),
        Number::Rational(value) => number_to_value(rational_number(
            i128::from(value.numerator()).abs(),
            i128::from(value.denominator()),
        )?),
        Number::Float(value) => Ok(Value::Float(value.abs())),
    }
}

fn absolute_complex(real: Number, imag: Number) -> Result<Value, RuntimeError> {
    let magnitude_squared =
        add_numbers(multiply_numbers(real, real)?, multiply_numbers(imag, imag)?)?;
    square_root_real(magnitude_squared)
}

fn compare_numbers(
    function: &str,
    arguments: &[Value],
    comparison: fn(Ordering) -> bool,
) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity(function, "at least one", 0));
    }
    let values = arguments
        .iter()
        .map(|value| number_argument(function, value))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Value::boolean(values.windows(2).all(|window| {
        comparison(compare_number_values(window[0], window[1]))
    })))
}

fn increment(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "1+", 1)?;
    add(&[arguments[0].clone(), Value::Integer(1)])
}

fn decrement(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "1-", 1)?;
    subtract(&[arguments[0].clone(), Value::Integer(1)])
}

#[derive(Clone, Copy)]
enum RoundingMode {
    Floor,
    Ceiling,
    Truncate,
    Round,
}

fn floor(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "floor", RoundingMode::Floor)
}

fn ceiling(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "ceiling", RoundingMode::Ceiling)
}

fn truncate(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "truncate", RoundingMode::Truncate)
}

fn round(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "round", RoundingMode::Round)
}

fn quotient_and_remainder(
    arguments: &[Value],
    function: &str,
    mode: RoundingMode,
) -> Result<Value, RuntimeError> {
    if arguments.len() != 1 && arguments.len() != 2 {
        return Err(arity(function, "one or two", arguments.len()));
    }
    let dividend = number_argument(function, &arguments[0])?;
    let divisor = if arguments.len() == 2 {
        number_argument(function, &arguments[1])?
    } else {
        Number::Integer(1)
    };
    if dividend.is_float() || divisor.is_float() {
        float_quotient_and_remainder(dividend, divisor, mode)
    } else {
        exact_quotient_and_remainder(dividend, divisor, mode)
    }
}

fn exact_quotient_and_remainder(
    dividend: Number,
    divisor: Number,
    mode: RoundingMode,
) -> Result<Value, RuntimeError> {
    let (dividend_numerator, dividend_denominator) = dividend
        .exact_parts()
        .expect("exact quotient received a float");
    let (divisor_numerator, divisor_denominator) = divisor
        .exact_parts()
        .expect("exact quotient received a float");
    if divisor_numerator == 0 {
        return Err(RuntimeError::DivisionByZero);
    }

    let dividend_numerator = i128::from(dividend_numerator);
    let dividend_denominator = i128::from(dividend_denominator);
    let divisor_numerator = i128::from(divisor_numerator);
    let divisor_denominator = i128::from(divisor_denominator);
    let mut quotient_numerator = dividend_numerator * divisor_denominator;
    let mut quotient_denominator = dividend_denominator * divisor_numerator;
    if quotient_denominator < 0 {
        quotient_numerator = -quotient_numerator;
        quotient_denominator = -quotient_denominator;
    }
    let truncated = quotient_numerator / quotient_denominator;
    let quotient =
        adjust_exact_quotient(truncated, quotient_numerator, quotient_denominator, mode)?;
    let quotient = i64::try_from(quotient).map_err(|_| RuntimeError::NumericOverflow)?;
    let remainder = rational_number(
        dividend_numerator * divisor_denominator
            - i128::from(quotient) * divisor_numerator * dividend_denominator,
        dividend_denominator * divisor_denominator,
    )?;
    Ok(Value::values(vec![
        Value::Integer(quotient),
        number_to_value(remainder)?,
    ]))
}

fn adjust_exact_quotient(
    truncated: i128,
    numerator: i128,
    denominator: i128,
    mode: RoundingMode,
) -> Result<i128, RuntimeError> {
    let remainder = numerator % denominator;
    if remainder == 0 {
        return Ok(truncated);
    }
    let direction = if numerator < 0 { -1 } else { 1 };
    match mode {
        RoundingMode::Truncate => Ok(truncated),
        RoundingMode::Floor if direction < 0 => truncated
            .checked_sub(1)
            .ok_or(RuntimeError::NumericOverflow),
        RoundingMode::Ceiling if direction > 0 => truncated
            .checked_add(1)
            .ok_or(RuntimeError::NumericOverflow),
        RoundingMode::Round => {
            let distance = remainder.abs() * 2;
            if distance > denominator || (distance == denominator && truncated % 2 != 0) {
                truncated
                    .checked_add(direction)
                    .ok_or(RuntimeError::NumericOverflow)
            } else {
                Ok(truncated)
            }
        }
        _ => Ok(truncated),
    }
}

fn float_quotient_and_remainder(
    dividend: Number,
    divisor: Number,
    mode: RoundingMode,
) -> Result<Value, RuntimeError> {
    let dividend = dividend.as_float();
    let divisor = divisor.as_float();
    if divisor == 0.0 {
        return Err(RuntimeError::DivisionByZero);
    }
    let ratio = dividend / divisor;
    let rounded = match mode {
        RoundingMode::Floor => ratio.floor(),
        RoundingMode::Ceiling => ratio.ceil(),
        RoundingMode::Truncate => ratio.trunc(),
        RoundingMode::Round => round_float(ratio),
    };
    let quotient = float_integer(rounded)?;
    let remainder = Value::Float(dividend - quotient as f64 * divisor);
    Ok(Value::values(vec![Value::Integer(quotient), remainder]))
}

fn round_float(value: f64) -> f64 {
    let truncated = value.trunc();
    let fraction = (value - truncated).abs();
    if fraction > 0.5 || (fraction == 0.5 && truncated % 2.0 != 0.0) {
        truncated + value.signum()
    } else {
        truncated
    }
}

fn float_integer(value: f64) -> Result<i64, RuntimeError> {
    if !value.is_finite() || value < i64::MIN as f64 || value >= 9_223_372_036_854_775_808.0 {
        return Err(RuntimeError::NumericOverflow);
    }
    Ok(value as i64)
}

fn greatest_common_divisor(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = 0i128;
    for argument in arguments {
        result = integer_gcd(result, i128::from(integer_argument("gcd", argument)?));
    }
    i64::try_from(result)
        .map(Value::Integer)
        .map_err(|_| RuntimeError::NumericOverflow)
}

fn least_common_multiple(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = 1i128;
    for argument in arguments {
        let value = i128::from(integer_argument("lcm", argument)?);
        if result == 0 || value == 0 {
            result = 0;
            continue;
        }
        let divisor = integer_gcd(result, value);
        result = (result / divisor)
            .checked_mul(value.abs())
            .ok_or(RuntimeError::NumericOverflow)?;
    }
    i64::try_from(result)
        .map(Value::Integer)
        .map_err(|_| RuntimeError::NumericOverflow)
}

fn integer_gcd(mut left: i128, mut right: i128) -> i128 {
    left = left.abs();
    right = right.abs();
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn numerator(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "numerator", 1)?;
    match arguments[0] {
        Value::Integer(value) => Ok(Value::Integer(value)),
        Value::Rational(value) => Ok(Value::Integer(value.numerator())),
        ref value => Err(type_error("numerator", "rational", value)),
    }
}

fn denominator(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "denominator", 1)?;
    match arguments[0] {
        Value::Integer(_) => Ok(Value::Integer(1)),
        Value::Rational(value) => Ok(Value::Integer(value.denominator())),
        ref value => Err(type_error("denominator", "rational", value)),
    }
}

fn modulo(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "mod", 2)?;
    let left = integer_argument("mod", &arguments[0])?;
    let right = integer_argument("mod", &arguments[1])?;
    let remainder = integer_remainder(left, right)?;
    if remainder != 0 && (left < 0) != (right < 0) {
        remainder
            .checked_add(right)
            .map(Value::Integer)
            .ok_or(RuntimeError::NumericOverflow)
    } else {
        Ok(Value::Integer(remainder))
    }
}

fn remainder(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rem", 2)?;
    let left = integer_argument("rem", &arguments[0])?;
    let right = integer_argument("rem", &arguments[1])?;
    integer_remainder(left, right).map(Value::Integer)
}

fn integer_remainder(left: i64, right: i64) -> Result<i64, RuntimeError> {
    if right == 0 {
        return Err(RuntimeError::DivisionByZero);
    }
    if left == i64::MIN && right == -1 {
        return Ok(0);
    }
    left.checked_rem(right).ok_or(RuntimeError::NumericOverflow)
}

fn arithmetic_shift(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "ash", 2)?;
    let value = integer_argument("ash", &arguments[0])?;
    let count = integer_argument("ash", &arguments[1])?;
    if count >= 0 {
        if count >= 64 {
            return if value == 0 {
                Ok(Value::Integer(0))
            } else {
                Err(RuntimeError::NumericOverflow)
            };
        }
        return value
            .checked_shl(count as u32)
            .map(Value::Integer)
            .ok_or(RuntimeError::NumericOverflow);
    }

    let shift = if count == i64::MIN {
        u64::MAX
    } else {
        (-count) as u64
    };
    Ok(Value::Integer(if shift >= 64 {
        if value < 0 { -1 } else { 0 }
    } else {
        value >> shift as u32
    }))
}

fn byte(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "byte", 2)?;
    let size = integer_argument("byte", &arguments[0])?;
    let position = integer_argument("byte", &arguments[1])?;
    validate_byte_bounds("byte", size, position)?;
    Ok(byte_spec_value(size, position))
}

fn byte_size(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "byte-size", 1)?;
    let (size, _) = parse_byte_spec("byte-size", &arguments[0])?;
    Ok(Value::Integer(i64::from(size)))
}

fn byte_position(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "byte-position", 1)?;
    let (_, position) = parse_byte_spec("byte-position", &arguments[0])?;
    Ok(Value::Integer(i64::from(position)))
}

fn ldb(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "ldb", 2)?;
    ldb_value("ldb", &arguments[0], &arguments[1])
}

fn ldb_test(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "ldb-test", 2)?;
    let (size, position) = parse_byte_spec("ldb-test", &arguments[0])?;
    let integer = integer_argument("ldb-test", &arguments[1])? as u64;
    Ok(Value::boolean(
        extract_byte_field(integer, size, position) != 0,
    ))
}

fn dpb(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "dpb", 3)?;
    dpb_value("dpb", &arguments[0], &arguments[1], &arguments[2])
}

fn mask_field(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "mask-field", 2)?;
    let (size, position) = parse_byte_spec("mask-field", &arguments[0])?;
    let integer = integer_argument("mask-field", &arguments[1])? as u64;
    Ok(Value::Integer((integer & byte_mask(size, position)) as i64))
}

fn deposit_field(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "deposit-field", 3)?;
    let (size, position) = parse_byte_spec("deposit-field", &arguments[1])?;
    let newbyte = integer_argument("deposit-field", &arguments[0])? as u64;
    let integer = integer_argument("deposit-field", &arguments[2])? as u64;
    let mask = byte_mask(size, position);
    Ok(Value::Integer(
        ((integer & !mask) | (newbyte & mask)) as i64,
    ))
}


    };
}
