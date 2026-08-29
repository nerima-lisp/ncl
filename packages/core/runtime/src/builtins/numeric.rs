use super::*;

pub(super) fn add(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = Number::Integer(0);
    for argument in arguments {
        let value = number_argument("+", argument)?;
        result = binary_number(result, value, '+')?;
    }
    number_to_value(result)
}

pub(super) fn subtract(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("-", "at least one", 0));
    }
    let values = arguments
        .iter()
        .map(|value| number_argument("-", value))
        .collect::<Result<Vec<_>, _>>()?;
    let mut result = values[0];
    if values.len() == 1 {
        result = negate_number(result)?;
    } else {
        for value in &values[1..] {
            result = binary_number(result, *value, '-')?;
        }
    }
    number_to_value(result)
}

pub(super) fn multiply(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = Number::Integer(1);
    for argument in arguments {
        let value = number_argument("*", argument)?;
        result = binary_number(result, value, '*')?;
    }
    number_to_value(result)
}

pub(super) fn divide(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("/", "at least one", 0));
    }
    let values = arguments
        .iter()
        .map(|value| number_argument("/", value))
        .collect::<Result<Vec<_>, _>>()?;
    let mut result;
    if values.len() == 1 {
        result = binary_number(Number::Integer(1), values[0], '/')?;
    } else {
        result = values[0];
        for value in &values[1..] {
            result = binary_number(result, *value, '/')?;
        }
    }
    number_to_value(result)
}

pub(super) fn exponentiate(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "expt", 2)?;
    let base = number_argument("expt", &arguments[0])?;
    let exponent = number_argument("expt", &arguments[1])?;

    if !base.is_complex() && !exponent.is_complex() && !base.is_float() {
        if let Some((exponent_numerator, exponent_denominator)) = exponent.exact_parts() {
            if exponent_denominator == 1 {
                return number_to_value(exact_power(base, exponent_numerator)?);
            }
        }
    }

    if base.is_complex() || exponent.is_complex() {
        return number_to_value(Number::Complex(
            base.as_complex().pow(exponent.as_complex())?,
        ));
    }

    let base_value = base.as_float();
    let exponent_value = exponent.as_float();
    if base_value < 0.0 && exponent_value.fract() != 0.0 {
        return number_to_value(Number::Complex(
            ComplexNumber::new(base_value, 0.0).pow(ComplexNumber::new(exponent_value, 0.0))?,
        ));
    }
    Ok(Value::Float(base_value.powf(exponent_value)))
}

pub(super) fn exponential(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "exp", 1)?;
    match number_argument("exp", &arguments[0])? {
        Number::Complex(value) => number_to_value(Number::Complex(value.exp())),
        value => Ok(Value::Float(value.as_float().exp())),
    }
}

pub(super) fn logarithm(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() != 1 && arguments.len() != 2 {
        return Err(arity("log", "one or two", arguments.len()));
    }
    let number = number_argument("log", &arguments[0])?;
    let logarithm = number.as_complex().ln()?;
    if arguments.len() == 1 {
        return number_to_value(Number::Complex(logarithm));
    }

    let base = number_argument("log", &arguments[1])?;
    if base.is_zero() {
        return Ok(Value::Integer(0));
    }
    let base_logarithm = base.as_complex().ln()?;
    number_to_value(Number::Complex(logarithm.divide(base_logarithm)?))
}

pub(super) fn sine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "sin", 1)?;
    match number_argument("sin", &arguments[0])? {
        Number::Complex(value) => number_to_value(Number::Complex(value.sin())),
        value => Ok(Value::Float(value.as_float().sin())),
    }
}

pub(super) fn cosine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "cos", 1)?;
    match number_argument("cos", &arguments[0])? {
        Number::Complex(value) => number_to_value(Number::Complex(value.cos())),
        value => Ok(Value::Float(value.as_float().cos())),
    }
}

pub(super) fn tangent(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "tan", 1)?;
    match number_argument("tan", &arguments[0])? {
        Number::Complex(value) => number_to_value(Number::Complex(value.sin().divide(value.cos())?)),
        value => Ok(Value::Float(value.as_float().tan())),
    }
}

pub(super) fn arctangent(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() || arguments.len() > 2 {
        return Err(arity("atan", "one or two", arguments.len()));
    }
    let number = if arguments.len() == 1 {
        number_argument("atan", &arguments[0])?
    } else {
        real_number_argument("atan", &arguments[0])?
    };
    if let Some(divisor) = arguments.get(1) {
        let divisor = real_number_argument("atan", divisor)?;
        return Ok(Value::Float(number.as_float().atan2(divisor.as_float())));
    }
    match number {
        Number::Complex(value) => number_to_value(Number::Complex(value.atan()?)),
        value => Ok(Value::Float(value.as_float().atan())),
    }
}

pub(super) fn arcsine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "asin", 1)?;
    match number_argument("asin", &arguments[0])? {
        Number::Complex(value) => number_to_value(Number::Complex(value.asin()?)),
        value => {
            let value = value.as_float();
            if (-1.0..=1.0).contains(&value) {
                Ok(Value::Float(value.asin()))
            } else {
                number_to_value(Number::Complex(ComplexNumber::new(value, 0.0).asin()?))
            }
        }
    }
}

pub(super) fn arccosine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "acos", 1)?;
    match number_argument("acos", &arguments[0])? {
        Number::Complex(value) => number_to_value(Number::Complex(value.acos()?)),
        value => {
            let value = value.as_float();
            if (-1.0..=1.0).contains(&value) {
                Ok(Value::Float(value.acos()))
            } else {
                number_to_value(Number::Complex(ComplexNumber::new(value, 0.0).acos()?))
            }
        }
    }
}

pub(super) fn hyperbolic_sine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "sinh", 1)?;
    match number_argument("sinh", &arguments[0])? {
        Number::Complex(value) => number_to_value(Number::Complex(value.sinh())),
        value => Ok(Value::Float(value.as_float().sinh())),
    }
}

pub(super) fn hyperbolic_cosine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "cosh", 1)?;
    match number_argument("cosh", &arguments[0])? {
        Number::Complex(value) => number_to_value(Number::Complex(value.cosh())),
        value => Ok(Value::Float(value.as_float().cosh())),
    }
}

pub(super) fn hyperbolic_tangent(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "tanh", 1)?;
    match number_argument("tanh", &arguments[0])? {
        Number::Complex(value) => number_to_value(Number::Complex(value.tanh()?)),
        value => Ok(Value::Float(value.as_float().tanh())),
    }
}

pub(super) fn hyperbolic_arcsine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "asinh", 1)?;
    match number_argument("asinh", &arguments[0])? {
        Number::Complex(value) => number_to_value(Number::Complex(value.asinh()?)),
        value => Ok(Value::Float(value.as_float().asinh())),
    }
}

pub(super) fn hyperbolic_arccosine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "acosh", 1)?;
    match number_argument("acosh", &arguments[0])? {
        Number::Complex(value) => number_to_value(Number::Complex(value.acosh()?)),
        value => {
            let value = value.as_float();
            if value >= 1.0 {
                Ok(Value::Float(value.acosh()))
            } else {
                number_to_value(Number::Complex(ComplexNumber::new(value, 0.0).acosh()?))
            }
        }
    }
}

pub(super) fn hyperbolic_arctangent(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "atanh", 1)?;
    match number_argument("atanh", &arguments[0])? {
        Number::Complex(value) => number_to_value(Number::Complex(value.atanh()?)),
        value => {
            let value = value.as_float();
            if value.abs() < 1.0 {
                Ok(Value::Float(value.atanh()))
            } else {
                number_to_value(Number::Complex(ComplexNumber::new(value, 0.0).atanh()?))
            }
        }
    }
}

pub(super) fn square_root(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "sqrt", 1)?;
    match number_argument("sqrt", &arguments[0])? {
        Number::Integer(value) if value >= 0 => {
            let root = integer_square_root(value as u128);
            if root * root == value as u128 {
                Ok(Value::Integer(root as i64))
            } else {
                Ok(Value::Float((value as f64).sqrt()))
            }
        }
        Number::Integer(value) => number_to_value(Number::Complex(
            ComplexNumber::new(value as f64, 0.0).sqrt(),
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
        Number::Rational(value) => number_to_value(Number::Complex(
            ComplexNumber::new(value.numerator() as f64 / value.denominator() as f64, 0.0).sqrt(),
        )),
        Number::Float(value) if value >= 0.0 => Ok(Value::Float(value.sqrt())),
        Number::Float(value) => {
            number_to_value(Number::Complex(ComplexNumber::new(value, 0.0).sqrt()))
        }
        Number::Complex(value) => number_to_value(Number::Complex(value.sqrt())),
    }
}

pub(super) fn integer_square_root_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "isqrt", 1)?;
    let value = integer_argument("isqrt", &arguments[0])?;
    if value < 0 {
        return Err(RuntimeError::InvalidForm {
            message: "isqrt requires a non-negative integer".to_owned(),
            span: None,
        });
    }
    Ok(Value::Integer(integer_square_root(value as u128) as i64))
}

pub(super) fn integer_square_root(value: u128) -> u128 {
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

pub(super) fn signum(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "signum", 1)?;
    match number_argument("signum", &arguments[0])? {
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
        Number::Complex(value) if value.is_zero() => Ok(Value::Integer(0)),
        Number::Complex(value) => {
            let magnitude = value.magnitude();
            number_to_value(Number::Complex(ComplexNumber::new(
                value.real / magnitude,
                value.imaginary / magnitude,
            )))
        }
    }
}

pub(super) fn phase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "phase", 1)?;
    match number_argument("phase", &arguments[0])? {
        Number::Complex(value) if value.is_zero() => Ok(Value::Float(0.0)),
        Number::Complex(value) => Ok(Value::Float(value.imaginary.atan2(value.real))),
        value => {
            let value = value.as_float();
            Ok(Value::Float(if value == 0.0 {
                0.0
            } else {
                0.0f64.atan2(value)
            }))
        }
    }
}

pub(super) fn cis(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "cis", 1)?;
    let radians = real_number_argument("cis", &arguments[0])?.as_float();
    Ok(Value::Complex {
        real: Box::new(Value::Float(radians.cos())),
        imaginary: Box::new(Value::Float(radians.sin())),
    })
}

pub(super) fn float_sign(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() || arguments.len() > 2 {
        return Err(arity("float-sign", "1 or 2", arguments.len()));
    }
    let Value::Float(number) = &arguments[0] else {
        return Err(type_error("float-sign", "a float", &arguments[0]));
    };
    let magnitude = match arguments.get(1) {
        Some(Value::Float(value)) => value.abs(),
        Some(value) => return Err(type_error("float-sign", "a float", value)),
        None => 1.0,
    };
    Ok(Value::Float(if number.is_sign_negative() {
        -magnitude
    } else {
        magnitude
    }))
}

pub(super) fn float_radix(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "float-radix", 1)?;
    if !matches!(arguments[0], Value::Float(_)) {
        return Err(type_error("float-radix", "a float", &arguments[0]));
    }
    Ok(Value::Integer(2))
}

pub(super) fn float_digits(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "float-digits", 1)?;
    if !matches!(arguments[0], Value::Float(_)) {
        return Err(type_error("float-digits", "a float", &arguments[0]));
    }
    Ok(Value::Integer(f64::MANTISSA_DIGITS as i64))
}

pub(super) fn float_precision(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "float-precision", 1)?;
    let Value::Float(value) = &arguments[0] else {
        return Err(type_error("float-precision", "a float", &arguments[0]));
    };
    let precision = if *value == 0.0 {
        0
    } else if value.is_subnormal() {
        let significand = value.to_bits() & ((1_u64 << 52) - 1);
        (64 - significand.leading_zeros()) as i64
    } else {
        f64::MANTISSA_DIGITS as i64
    };
    Ok(Value::Integer(precision))
}

pub(super) fn decode_float(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "decode-float", 1)?;
    let Value::Float(value) = &arguments[0] else {
        return Err(type_error("decode-float", "a float", &arguments[0]));
    };
    let bits = value.to_bits();
    let sign = if bits >> 63 == 0 { 1.0 } else { -1.0 };
    let exponent_bits = (bits >> 52) & 0x7ff;
    let fraction = bits & ((1_u64 << 52) - 1);
    if exponent_bits == 0x7ff {
        return Err(RuntimeError::InvalidForm {
            message: "decode-float of non-finite float is undefined".to_owned(),
            span: None,
        });
    }
    if exponent_bits == 0 && fraction == 0 {
        return Ok(Value::values(vec![
            Value::Float(0.0),
            Value::Integer(0),
            Value::Float(sign),
        ]));
    }
    let (significand, exponent) = if exponent_bits == 0 {
        let leading = 63 - fraction.leading_zeros() as i64;
        (
            fraction as f64 / 2.0_f64.powi((leading + 1) as i32),
            leading - 1073,
        )
    } else {
        let mantissa = (1_u64 << 52) | fraction;
        (
            mantissa as f64 / 2.0_f64.powi(53),
            exponent_bits as i64 - 1022,
        )
    };
    Ok(Value::values(vec![
        Value::Float(significand),
        Value::Integer(exponent),
        Value::Float(sign),
    ]))
}

pub(super) fn scale_float(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "scale-float", 2)?;
    let Value::Float(value) = &arguments[0] else {
        return Err(type_error("scale-float", "a float", &arguments[0]));
    };
    let exponent = integer_argument("scale-float", &arguments[1])?;
    if exponent == 0 || *value == 0.0 || !value.is_finite() {
        return Ok(Value::Float(*value));
    }
    let mut result = *value;
    let mut remaining = exponent.clamp(-4096, 4096);
    while remaining != 0 {
        let step = remaining.clamp(-512, 512);
        result *= 2.0_f64.powi(step as i32);
        remaining -= step;
        if result == 0.0 || result.is_infinite() {
            break;
        }
    }
    Ok(Value::Float(result))
}

pub(super) fn integer_decode_float(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "integer-decode-float", 1)?;
    let Value::Float(value) = &arguments[0] else {
        return Err(type_error("integer-decode-float", "a float", &arguments[0]));
    };
    let bits = value.to_bits();
    let sign = if bits >> 63 == 0 { 1 } else { -1 };
    let exponent_bits = (bits >> 52) & 0x7ff;
    let fraction = bits & ((1_u64 << 52) - 1);
    if exponent_bits == 0x7ff {
        return Err(RuntimeError::InvalidForm {
            message: "integer-decode-float of non-finite float is undefined".to_owned(),
            span: None,
        });
    }
    let (significand, exponent) = if exponent_bits == 0 {
        if fraction == 0 {
            (0, 0)
        } else {
            (fraction as i64, -1074)
        }
    } else {
        (
            ((1_u64 << 52) | fraction) as i64,
            exponent_bits as i64 - 1075,
        )
    };
    Ok(Value::values(vec![
        Value::Integer(significand),
        Value::Integer(exponent),
        Value::Integer(sign),
    ]))
}

pub(super) fn complex(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() || arguments.len() > 2 {
        return Err(arity("complex", "1 or 2", arguments.len()));
    }
    let real = real_value_argument("complex", &arguments[0])?;
    let imaginary = if let Some(value) = arguments.get(1) {
        real_value_argument("complex", value)?
    } else {
        Value::Integer(0)
    };
    Value::complex(real, imaginary)
}

pub(super) fn realpart(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "realpart", 1)?;
    match &arguments[0] {
        Value::Complex { real, .. } => Ok(real.as_ref().clone()),
        value if value.is_real_number() => Ok(value.clone()),
        value => Err(number_error("realpart", value)),
    }
}

pub(super) fn imagpart(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "imagpart", 1)?;
    match &arguments[0] {
        Value::Complex { imaginary, .. } => Ok(imaginary.as_ref().clone()),
        value if value.is_real_number() => Ok(Value::Integer(0)),
        value => Err(number_error("imagpart", value)),
    }
}

pub(super) fn conjugate(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "conjugate", 1)?;
    match &arguments[0] {
        Value::Complex { real, imaginary } => {
            let imaginary = negate_number(number_argument("conjugate", imaginary)?)?;
            Value::complex(real.as_ref().clone(), number_to_value(imaginary)?)
        }
        value if value.is_real_number() => Ok(value.clone()),
        value => Err(number_error("conjugate", value)),
    }
}

pub(super) fn float_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() || arguments.len() > 2 {
        return Err(arity("float", "1 to 2", arguments.len()));
    }
    let number = real_number_argument("float", &arguments[0])?;
    if let Some(prototype) = arguments.get(1) {
        if !matches!(prototype, Value::Float(_)) {
            return Err(type_error("float", "a float prototype", prototype));
        }
    }
    Ok(Value::Float(number.as_float()))
}

pub(super) fn rational(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rational", 1)?;
    match number_argument("rational", &arguments[0])? {
        Number::Integer(value) => Ok(Value::Integer(value)),
        Number::Rational(value) => Value::rational(
            i128::from(value.numerator()),
            i128::from(value.denominator()),
        ),
        Number::Float(value) => rational_from_float(value),
        Number::Complex(_) => Err(number_error("rational", &arguments[0])),
    }
}

pub(super) fn rational_from_float(value: f64) -> Result<Value, RuntimeError> {
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

pub(super) fn rationalize(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rationalize", 1)?;
    match number_argument("rationalize", &arguments[0])? {
        Number::Integer(value) => Ok(Value::Integer(value)),
        Number::Rational(value) => Value::rational(
            i128::from(value.numerator()),
            i128::from(value.denominator()),
        ),
        Number::Float(value) => rationalize_float(value),
        Number::Complex(_) => Err(number_error("rationalize", &arguments[0])),
    }
}

pub(super) fn rationalize_float(value: f64) -> Result<Value, RuntimeError> {
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

pub(super) fn simplest_rational(lower: f64, upper: f64) -> Result<(i128, i128), RuntimeError> {
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

pub(super) fn simplest_positive_rational(
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

pub(super) fn exact_power(base: Number, exponent: i64) -> Result<Number, RuntimeError> {
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

pub(super) fn checked_power(base: i128, mut exponent: u64) -> Result<i128, RuntimeError> {
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

pub(super) fn numeric_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers("=", arguments, |ordering| ordering == Ordering::Equal)
}

pub(super) fn numeric_not_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("/=", "at least one", 0));
    }
    let values = arguments
        .iter()
        .map(|value| number_argument("/=", value))
        .collect::<Result<Vec<_>, _>>()?;
    for (index, left) in values.iter().enumerate() {
        if values
            .iter()
            .skip(index + 1)
            .any(|right| numeric_equalp(*left, *right))
        {
            return Ok(Value::Nil);
        }
    }
    Ok(Value::boolean(true))
}

pub(super) fn less_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers("<", arguments, |ordering| ordering == Ordering::Less)
}

pub(super) fn greater_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers(">", arguments, |ordering| ordering == Ordering::Greater)
}

pub(super) fn less_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers("<=", arguments, |ordering| ordering != Ordering::Greater)
}

pub(super) fn greater_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers(">=", arguments, |ordering| ordering != Ordering::Less)
}

pub(super) fn zerop(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "zerop", 1)?;
    Ok(Value::boolean(
        number_argument("zerop", &arguments[0])?.is_zero(),
    ))
}

pub(super) fn plusp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "plusp", 1)?;
    Ok(Value::boolean(
        real_number_argument("plusp", &arguments[0])?.as_float() > 0.0,
    ))
}

pub(super) fn minusp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "minusp", 1)?;
    Ok(Value::boolean(
        real_number_argument("minusp", &arguments[0])?.as_float() < 0.0,
    ))
}

pub(super) fn evenp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "evenp", 1)?;
    Ok(Value::boolean(
        integer_argument("evenp", &arguments[0])? % 2 == 0,
    ))
}

pub(super) fn oddp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "oddp", 1)?;
    Ok(Value::boolean(
        integer_argument("oddp", &arguments[0])? % 2 != 0,
    ))
}

pub(super) fn minimum(arguments: &[Value]) -> Result<Value, RuntimeError> {
    extreme(arguments, "min", true)
}

pub(super) fn maximum(arguments: &[Value]) -> Result<Value, RuntimeError> {
    extreme(arguments, "max", false)
}

pub(super) fn extreme(
    arguments: &[Value],
    function: &str,
    choose_minimum: bool,
) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity(function, "at least one", 0));
    }
    let values = arguments
        .iter()
        .map(|value| real_number_argument(function, value))
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

pub(super) fn absolute(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "abs", 1)?;
    match number_argument("abs", &arguments[0])? {
        Number::Integer(value) => value
            .checked_abs()
            .map(Value::Integer)
            .ok_or(RuntimeError::NumericOverflow),
        Number::Rational(value) => number_to_value(rational_number(
            i128::from(value.numerator()).abs(),
            i128::from(value.denominator()),
        )?),
        Number::Float(value) => Ok(Value::Float(value.abs())),
        Number::Complex(value) => {
            number_to_value(Number::Complex(ComplexNumber::new(value.magnitude(), 0.0)))
        }
    }
}

pub(super) fn compare_numbers(
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
    if function == "=" {
        return Ok(Value::boolean(
            values
                .windows(2)
                .all(|window| numeric_equalp(window[0], window[1])),
        ));
    }
    if values.iter().any(Number::is_complex) {
        return Err(type_error(function, "a real number", &arguments[0]));
    }
    Ok(Value::boolean(values.windows(2).all(|window| {
        comparison(compare_number_values(window[0], window[1]))
    })))
}

pub(super) fn increment(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "1+", 1)?;
    add(&[arguments[0].clone(), Value::Integer(1)])
}

pub(super) fn decrement(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "1-", 1)?;
    subtract(&[arguments[0].clone(), Value::Integer(1)])
}

#[derive(Clone, Copy)]
pub(super) enum RoundingMode {
    Floor,
    Ceiling,
    Truncate,
    Round,
}

pub(super) fn floor(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "floor", RoundingMode::Floor)
}

pub(super) fn ceiling(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "ceiling", RoundingMode::Ceiling)
}

pub(super) fn truncate(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "truncate", RoundingMode::Truncate)
}

pub(super) fn round(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "round", RoundingMode::Round)
}

pub(super) fn quotient_and_remainder(
    arguments: &[Value],
    function: &str,
    mode: RoundingMode,
) -> Result<Value, RuntimeError> {
    if arguments.len() != 1 && arguments.len() != 2 {
        return Err(arity(function, "one or two", arguments.len()));
    }
    let dividend = real_number_argument(function, &arguments[0])?;
    let divisor = if arguments.len() == 2 {
        real_number_argument(function, &arguments[1])?
    } else {
        Number::Integer(1)
    };
    if dividend.is_float() || divisor.is_float() {
        float_quotient_and_remainder(dividend, divisor, mode)
    } else {
        exact_quotient_and_remainder(dividend, divisor, mode)
    }
}

pub(super) fn exact_quotient_and_remainder(
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

pub(super) fn adjust_exact_quotient(
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

pub(super) fn float_quotient_and_remainder(
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

pub(super) fn round_float(value: f64) -> f64 {
    let truncated = value.trunc();
    let fraction = (value - truncated).abs();
    if fraction > 0.5 || (fraction == 0.5 && truncated % 2.0 != 0.0) {
        truncated + value.signum()
    } else {
        truncated
    }
}

pub(super) fn float_integer(value: f64) -> Result<i64, RuntimeError> {
    if !value.is_finite() || value < i64::MIN as f64 || value >= 9_223_372_036_854_775_808.0 {
        return Err(RuntimeError::NumericOverflow);
    }
    Ok(value as i64)
}

pub(super) fn greatest_common_divisor(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = 0i128;
    for argument in arguments {
        result = integer_gcd(result, i128::from(integer_argument("gcd", argument)?));
    }
    i64::try_from(result)
        .map(Value::Integer)
        .map_err(|_| RuntimeError::NumericOverflow)
}

pub(super) fn least_common_multiple(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(super) fn integer_gcd(mut left: i128, mut right: i128) -> i128 {
    left = left.abs();
    right = right.abs();
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

pub(super) fn numerator(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "numerator", 1)?;
    match arguments[0] {
        Value::Integer(value) => Ok(Value::Integer(value)),
        Value::Rational(value) => Ok(Value::Integer(value.numerator())),
        ref value => Err(type_error("numerator", "rational", value)),
    }
}

pub(super) fn denominator(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "denominator", 1)?;
    match arguments[0] {
        Value::Integer(_) => Ok(Value::Integer(1)),
        Value::Rational(value) => Ok(Value::Integer(value.denominator())),
        ref value => Err(type_error("denominator", "rational", value)),
    }
}

pub(super) fn modulo(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(super) fn remainder(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rem", 2)?;
    let left = integer_argument("rem", &arguments[0])?;
    let right = integer_argument("rem", &arguments[1])?;
    integer_remainder(left, right).map(Value::Integer)
}

pub(super) fn integer_remainder(left: i64, right: i64) -> Result<i64, RuntimeError> {
    if right == 0 {
        return Err(RuntimeError::DivisionByZero);
    }
    if left == i64::MIN && right == -1 {
        return Ok(0);
    }
    left.checked_rem(right).ok_or(RuntimeError::NumericOverflow)
}

pub(super) fn arithmetic_shift(arguments: &[Value]) -> Result<Value, RuntimeError> {
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
        if value < 0 {
            -1
        } else {
            0
        }
    } else {
        value >> shift as u32
    }))
}

pub(super) fn logand(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise(arguments, "logand", -1, |left, right| left & right)
}

pub(super) fn logior(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise(arguments, "logior", 0, |left, right| left | right)
}

pub(super) fn logxor(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise(arguments, "logxor", 0, |left, right| left ^ right)
}

pub(super) fn logeqv(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise_binary(arguments, "logeqv", |left, right| !(left ^ right))
}

pub(super) fn lognand(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise_binary(arguments, "lognand", |left, right| !(left & right))
}

pub(super) fn lognor(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise_binary(arguments, "lognor", |left, right| !(left | right))
}

pub(super) fn logandc1(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise_binary(arguments, "logandc1", |left, right| (!left) & right)
}

pub(super) fn logandc2(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise_binary(arguments, "logandc2", |left, right| left & (!right))
}

pub(super) fn logorc1(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise_binary(arguments, "logorc1", |left, right| (!left) | right)
}

pub(super) fn logorc2(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise_binary(arguments, "logorc2", |left, right| left | (!right))
}

pub(super) fn bitwise(
    arguments: &[Value],
    function: &str,
    identity: i64,
    operation: fn(i64, i64) -> i64,
) -> Result<Value, RuntimeError> {
    let mut result = identity;
    for argument in arguments {
        result = operation(result, integer_argument(function, argument)?);
    }
    Ok(Value::Integer(result))
}

pub(super) fn bitwise_binary(
    arguments: &[Value],
    function: &str,
    operation: fn(i64, i64) -> i64,
) -> Result<Value, RuntimeError> {
    exact(arguments, function, 2)?;
    let left = integer_argument(function, &arguments[0])?;
    let right = integer_argument(function, &arguments[1])?;
    Ok(Value::Integer(operation(left, right)))
}

pub(super) fn boole(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "boole", 3)?;
    let operation = integer_argument("boole", &arguments[0])?;
    let left = integer_argument("boole", &arguments[1])?;
    let right = integer_argument("boole", &arguments[2])?;
    let result = match operation {
        0 => 0,
        1 => -1,
        2 => left,
        3 => right,
        4 => !left,
        5 => !right,
        6 => left & right,
        7 => left | right,
        8 => left ^ right,
        9 => !(left ^ right),
        10 => !(left & right),
        11 => !(left | right),
        12 => (!left) & right,
        13 => left & (!right),
        14 => (!left) | right,
        15 => left | (!right),
        _ => {
            return Err(RuntimeError::InvalidForm {
                message: format!(
                    "boole operation must be an integer from 0 through 15, got {operation}"
                ),
                span: None,
            });
        }
    };
    Ok(Value::Integer(result))
}

pub(super) fn lognot(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "lognot", 1)?;
    Ok(Value::Integer(!integer_argument("lognot", &arguments[0])?))
}

pub(super) fn logtest(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logtest", 2)?;
    let left = integer_argument("logtest", &arguments[0])?;
    let right = integer_argument("logtest", &arguments[1])?;
    Ok(Value::boolean((left & right) != 0))
}

pub(super) fn logbitp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logbitp", 2)?;
    let index = index_argument("logbitp", &arguments[0])?;
    let value = integer_argument("logbitp", &arguments[1])?;
    let bit_set = if index >= 63 {
        value < 0
    } else {
        value & (1_i64 << index) != 0
    };
    Ok(Value::boolean(bit_set))
}

pub(super) fn logcount(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logcount", 1)?;
    let value = integer_argument("logcount", &arguments[0])?;
    let count = if value < 0 {
        (!value).count_ones()
    } else {
        value.count_ones()
    };
    Ok(Value::Integer(count as i64))
}

pub(super) fn integer_length(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "integer-length", 1)?;
    let value = integer_argument("integer-length", &arguments[0])?;
    let magnitude = if value < 0 { !value } else { value } as u64;
    Ok(Value::Integer((64 - magnitude.leading_zeros()) as i64))
}

pub(super) fn byte(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "byte", 2)?;
    let size = byte_index_argument("byte", &arguments[0])?;
    let position = byte_index_argument("byte", &arguments[1])?;
    Ok(Value::list(vec![
        Value::Integer(size),
        Value::Integer(position),
    ]))
}

pub(super) fn byte_size(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "byte-size", 1)?;
    let (size, _) = byte_specifier_argument("byte-size", &arguments[0])?;
    Ok(Value::Integer(size))
}

pub(super) fn byte_position(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "byte-position", 1)?;
    let (_, position) = byte_specifier_argument("byte-position", &arguments[0])?;
    Ok(Value::Integer(position))
}

pub(super) fn ldb(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "ldb", 2)?;
    let (size, position) = byte_specifier_argument("ldb", &arguments[0])?;
    let value = integer_argument("ldb", &arguments[1])?;
    Ok(Value::Integer(ldb_value(size, position, value)?))
}

pub(super) fn ldb_test(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "ldb-test", 2)?;
    let (size, position) = byte_specifier_argument("ldb-test", &arguments[0])?;
    let value = integer_argument("ldb-test", &arguments[1])?;
    Ok(Value::boolean(ldb_test_value(size, position, value)))
}

pub(super) fn mask_field(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "mask-field", 2)?;
    let (size, position) = byte_specifier_argument("mask-field", &arguments[0])?;
    let value = integer_argument("mask-field", &arguments[1])?;
    Ok(Value::Integer(mask_field_value(size, position, value)?))
}

pub(super) fn dpb(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "dpb", 3)?;
    let newbyte = integer_argument("dpb", &arguments[0])?;
    let (size, position) = byte_specifier_argument("dpb", &arguments[1])?;
    let integer = integer_argument("dpb", &arguments[2])?;
    Ok(Value::Integer(replace_field_value(
        newbyte,
        size,
        position,
        integer,
        0,
        Some(size),
    )?))
}

pub(super) fn deposit_field(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "deposit-field", 3)?;
    let newbyte = integer_argument("deposit-field", &arguments[0])?;
    let (size, position) = byte_specifier_argument("deposit-field", &arguments[1])?;
    let integer = integer_argument("deposit-field", &arguments[2])?;
    Ok(Value::Integer(replace_field_value(
        newbyte,
        size,
        position,
        integer,
        position,
        position.checked_add(size),
    )?))
}

fn byte_index_argument(function: &str, value: &Value) -> Result<i64, RuntimeError> {
    i64::try_from(index_argument(function, value)?).map_err(|_| RuntimeError::NumericOverflow)
}

fn byte_specifier_argument(function: &str, value: &Value) -> Result<(i64, i64), RuntimeError> {
    let Some(items) = value.list_items() else {
        return Err(type_error(function, "a byte specifier", value));
    };
    if items.len() != 2 {
        return Err(type_error(function, "a byte specifier", value));
    }
    let size = byte_index_argument(function, &items[0])?;
    let position = byte_index_argument(function, &items[1])?;
    Ok((size, position))
}

fn low_bits_mask(width: i64) -> u64 {
    if width <= 0 {
        0
    } else if width >= 64 {
        u64::MAX
    } else {
        (1_u64 << width as u32) - 1
    }
}

fn integer_bits(value: i64, position: i64, width: i64) -> u64 {
    if width <= 0 {
        return 0;
    }
    let mask = low_bits_mask(width.min(64));
    if position >= 63 {
        return if value < 0 { mask } else { 0 };
    }
    let value = (value as i128) as u128;
    ((value >> position as u32) & u128::from(mask)) as u64
}

fn integer_bits_match(value: i64, start: i64, end: Option<i64>, expected: bool) -> bool {
    let low_end = end.unwrap_or(63).min(63);
    if low_end > start {
        let width = low_end - start;
        let bits = integer_bits(value, start, width);
        let mask = low_bits_mask(width);
        if (expected && bits != mask) || (!expected && bits != 0) {
            return false;
        }
    }
    if end.map_or(true, |end| end > 63) {
        (value < 0) == expected
    } else {
        true
    }
}

fn ldb_value(size: i64, position: i64, value: i64) -> Result<i64, RuntimeError> {
    if size == 0 {
        return Ok(0);
    }
    if size >= 64 && value < 0 {
        return Err(RuntimeError::NumericOverflow);
    }
    Ok(integer_bits(value, position, size.min(63)) as i64)
}

fn ldb_test_value(size: i64, position: i64, value: i64) -> bool {
    if size == 0 {
        return false;
    }
    if value < 0 && (position >= 63 || size > 63 - position.min(63)) {
        return true;
    }
    integer_bits(value, position, size.min(63)) != 0
}

fn mask_field_value(size: i64, position: i64, value: i64) -> Result<i64, RuntimeError> {
    if size == 0 {
        return Ok(0);
    }
    if value < 0 && (position >= 63 || size > 63 - position.min(63)) {
        return Err(RuntimeError::NumericOverflow);
    }
    if position >= 63 {
        return Ok(0);
    }
    let width = size.min(63 - position);
    let bits = integer_bits(value, position, width);
    Ok((bits << position as u32) as i64)
}

fn replace_field_value(
    newbyte: i64,
    size: i64,
    position: i64,
    integer: i64,
    source_position: i64,
    source_end: Option<i64>,
) -> Result<i64, RuntimeError> {
    if size == 0 {
        return Ok(integer);
    }
    if field_overflows(position, size)
        && !integer_bits_match(
            newbyte,
            source_position_for_overflow(position, source_position),
            source_end,
            integer < 0,
        )
    {
        return Err(RuntimeError::NumericOverflow);
    }
    if position >= 63 {
        return Ok(integer);
    }
    Ok(merge_field_bits(
        integer,
        integer_bits(newbyte, source_position, size.min(63 - position)),
        size.min(63 - position),
        position,
    ))
}

fn field_overflows(position: i64, size: i64) -> bool {
    position.checked_add(size).map_or(true, |end| end > 63)
}

fn source_position_for_overflow(position: i64, source_position: i64) -> i64 {
    if position >= 63 {
        source_position
    } else {
        source_position + (63 - position)
    }
}

fn merge_field_bits(integer: i64, new_bits: u64, width: i64, position: i64) -> i64 {
    let mask = low_bits_mask(width) << position as u32;
    ((integer as u64 & !mask) | (new_bits << position as u32 & mask)) as i64
}

pub(super) fn parse_integer(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() || (arguments.len() - 1) % 2 != 0 {
        return Err(arity(
            "parse-integer",
            "a string and keyword/value pairs",
            arguments.len(),
        ));
    }
    let chars = match &arguments[0] {
        Value::String(value) => value.as_ref().chars().collect::<Vec<_>>(),
        value => return Err(type_error("parse-integer", "a string", value)),
    };
    let mut start = 0;
    let mut end = chars.len();
    let mut radix = 10_i64;
    let mut junk_allowed = false;
    for pair in arguments[1..].chunks_exact(2) {
        match array_option_name("parse-integer", &pair[0])?.as_str() {
            "START" => start = index_argument("parse-integer", &pair[1])?,
            "END" => end = index_argument("parse-integer", &pair[1])?,
            "RADIX" => radix = integer_argument("parse-integer", &pair[1])?,
            "JUNK-ALLOWED" => junk_allowed = pair[1].is_truthy(),
            option => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("parse-integer does not accept :{option}"),
                    span: None,
                });
            }
        }
    }
    if start > end || end > chars.len() {
        return Err(RuntimeError::InvalidForm {
            message: "parse-integer bounds are invalid".to_string(),
            span: None,
        });
    }
    if !(2..=36).contains(&radix) {
        return Err(RuntimeError::InvalidForm {
            message: format!("parse-integer radix must be between 2 and 36, got {radix}"),
            span: None,
        });
    }
    let radix = u32::try_from(radix).expect("parse-integer radix was checked");
    let mut cursor = start;
    while cursor < end && chars[cursor].is_whitespace() {
        cursor += 1;
    }
    let negative = match chars.get(cursor) {
        Some('+') => {
            cursor += 1;
            false
        }
        Some('-') => {
            cursor += 1;
            true
        }
        _ => false,
    };
    let digits_start = cursor;
    let mut magnitude = 0_i128;
    while cursor < end {
        let Some(digit) = parse_integer_digit(chars[cursor]) else {
            break;
        };
        if digit >= radix {
            break;
        }
        magnitude = magnitude
            .checked_mul(i128::from(radix))
            .and_then(|value| value.checked_add(i128::from(digit)))
            .ok_or(RuntimeError::NumericOverflow)?;
        cursor += 1;
    }
    if cursor == digits_start {
        if junk_allowed {
            let position = i64::try_from(cursor).map_err(|_| RuntimeError::NumericOverflow)?;
            return Ok(Value::values(vec![Value::Nil, Value::Integer(position)]));
        }
        return Err(RuntimeError::InvalidForm {
            message: "parse-integer found no integer".to_string(),
            span: None,
        });
    }
    let signed = if negative {
        magnitude
            .checked_neg()
            .ok_or(RuntimeError::NumericOverflow)?
    } else {
        magnitude
    };
    let integer = i64::try_from(signed).map_err(|_| RuntimeError::NumericOverflow)?;
    if junk_allowed {
        let position = i64::try_from(cursor).map_err(|_| RuntimeError::NumericOverflow)?;
        return Ok(Value::values(vec![
            Value::Integer(integer),
            Value::Integer(position),
        ]));
    }
    let mut trailing = cursor;
    while trailing < end && chars[trailing].is_whitespace() {
        trailing += 1;
    }
    if trailing != end {
        return Err(RuntimeError::InvalidForm {
            message: "parse-integer found junk after the integer".to_string(),
            span: None,
        });
    }
    let position = i64::try_from(end).map_err(|_| RuntimeError::NumericOverflow)?;
    Ok(Value::values(vec![
        Value::Integer(integer),
        Value::Integer(position),
    ]))
}

pub(super) fn parse_integer_digit(character: char) -> Option<u32> {
    match character {
        '0'..='9' => Some(u32::from(character as u8 - b'0')),
        'A'..='Z' => Some(u32::from(character as u8 - b'A') + 10),
        'a'..='z' => Some(u32::from(character as u8 - b'a') + 10),
        _ => None,
    }
}

#[derive(Clone, Copy)]
pub(super) struct ComplexNumber {
    real: f64,
    imaginary: f64,
}

impl ComplexNumber {
    fn new(real: f64, imaginary: f64) -> Self {
        Self { real, imaginary }
    }

    fn zero() -> Self {
        Self::new(0.0, 0.0)
    }

    fn one() -> Self {
        Self::new(1.0, 0.0)
    }

    fn is_zero(self) -> bool {
        self.real == 0.0 && self.imaginary == 0.0
    }

    fn magnitude(self) -> f64 {
        self.real.hypot(self.imaginary)
    }

    fn add(self, other: Self) -> Self {
        Self::new(self.real + other.real, self.imaginary + other.imaginary)
    }

    fn subtract(self, other: Self) -> Self {
        Self::new(self.real - other.real, self.imaginary - other.imaginary)
    }

    fn multiply(self, other: Self) -> Self {
        Self::new(
            self.real * other.real - self.imaginary * other.imaginary,
            self.real * other.imaginary + self.imaginary * other.real,
        )
    }

    fn reciprocal(self) -> Result<Self, RuntimeError> {
        let denominator = self.real * self.real + self.imaginary * self.imaginary;
        if denominator == 0.0 {
            return Err(RuntimeError::DivisionByZero);
        }
        Ok(Self::new(
            self.real / denominator,
            -self.imaginary / denominator,
        ))
    }

    fn divide(self, other: Self) -> Result<Self, RuntimeError> {
        Ok(self.multiply(other.reciprocal()?))
    }

    fn sqrt(self) -> Self {
        if self.imaginary == 0.0 && self.real >= 0.0 {
            return Self::new(self.real.sqrt(), 0.0);
        }
        let magnitude = self.magnitude();
        let real = ((magnitude + self.real) / 2.0).sqrt();
        let imaginary = ((magnitude - self.real) / 2.0)
            .sqrt()
            .copysign(self.imaginary);
        Self::new(real, imaginary)
    }

    fn exp(self) -> Self {
        let magnitude = self.real.exp();
        Self::new(
            magnitude * self.imaginary.cos(),
            magnitude * self.imaginary.sin(),
        )
    }

    fn sin(self) -> Self {
        Self::new(
            self.real.sin() * self.imaginary.cosh(),
            self.real.cos() * self.imaginary.sinh(),
        )
    }

    fn cos(self) -> Self {
        Self::new(
            self.real.cos() * self.imaginary.cosh(),
            -self.real.sin() * self.imaginary.sinh(),
        )
    }

    fn sinh(self) -> Self {
        Self::new(
            self.real.sinh() * self.imaginary.cos(),
            self.real.cosh() * self.imaginary.sin(),
        )
    }

    fn cosh(self) -> Self {
        Self::new(
            self.real.cosh() * self.imaginary.cos(),
            self.real.sinh() * self.imaginary.sin(),
        )
    }

    fn tanh(self) -> Result<Self, RuntimeError> {
        self.sinh().divide(self.cosh())
    }

    fn asinh(self) -> Result<Self, RuntimeError> {
        self.add(Self::one().add(self.multiply(self)).sqrt()).ln()
    }

    fn acosh(self) -> Result<Self, RuntimeError> {
        let positive = self
            .add(Self::one())
            .multiply(Self::new(0.5, 0.0))
            .sqrt();
        let negative = self
            .subtract(Self::one())
            .multiply(Self::new(0.5, 0.0))
            .sqrt();
        let logarithm = positive.add(negative).ln()?;
        Ok(Self::new(2.0, 0.0).multiply(logarithm))
    }

    fn atanh(self) -> Result<Self, RuntimeError> {
        let result = Self::new(0.0, 1.0).multiply(self).atan()?;
        Ok(Self::new(result.imaginary, -result.real))
    }

    fn asin(self) -> Result<Self, RuntimeError> {
        let imaginary_unit = Self::new(0.0, 1.0);
        let root = Self::one().subtract(self.multiply(self)).sqrt();
        let logarithm = imaginary_unit.multiply(self).add(root).ln()?;
        Ok(Self::new(logarithm.imaginary, -logarithm.real))
    }

    fn acos(self) -> Result<Self, RuntimeError> {
        Ok(Self::new(std::f64::consts::FRAC_PI_2, 0.0).subtract(self.asin()?))
    }

    fn atan(self) -> Result<Self, RuntimeError> {
        let imaginary_unit = Self::new(0.0, 1.0);
        let positive = Self::one()
            .add(imaginary_unit.multiply(self))
            .ln_for_atan()?;
        let negative = Self::one()
            .subtract(imaginary_unit.multiply(self))
            .ln_for_atan()?;
        positive
            .subtract(negative)
            .divide(Self::new(0.0, 2.0))
    }

    fn ln_for_atan(self) -> Result<Self, RuntimeError> {
        if self.is_zero() {
            return Err(RuntimeError::DivisionByZero);
        }
        let angle = if self.real < 0.0 && self.imaginary == 0.0 {
            -std::f64::consts::PI
        } else {
            self.imaginary.atan2(self.real)
        };
        Ok(Self::new(self.magnitude().ln(), angle))
    }

    fn ln(self) -> Result<Self, RuntimeError> {
        if self.is_zero() {
            return Err(RuntimeError::DivisionByZero);
        }
        Ok(Self::new(
            self.magnitude().ln(),
            self.imaginary.atan2(self.real),
        ))
    }

    fn pow(self, exponent: Self) -> Result<Self, RuntimeError> {
        if exponent.imaginary == 0.0
            && exponent.real.is_finite()
            && exponent.real.fract() == 0.0
            && exponent.real >= i64::MIN as f64
            && exponent.real <= i64::MAX as f64
        {
            return self.powi(exponent.real as i64);
        }
        if self.is_zero() {
            if exponent.real > 0.0 {
                return Ok(Self::zero());
            }
            return Err(RuntimeError::DivisionByZero);
        }
        let log_magnitude = self.magnitude().ln();
        let angle = self.imaginary.atan2(self.real);
        let real = exponent.real * log_magnitude - exponent.imaginary * angle;
        let imaginary = exponent.real * angle + exponent.imaginary * log_magnitude;
        let magnitude = real.exp();
        Ok(Self::new(
            magnitude * imaginary.cos(),
            magnitude * imaginary.sin(),
        ))
    }

    fn powi(self, exponent: i64) -> Result<Self, RuntimeError> {
        if exponent == 0 {
            return Ok(Self::one());
        }
        let mut factor = if exponent < 0 {
            self.reciprocal()?
        } else {
            self
        };
        let mut remaining = exponent.unsigned_abs();
        let mut result = Self::one();
        while remaining != 0 {
            if remaining & 1 == 1 {
                result = result.multiply(factor);
            }
            remaining >>= 1;
            if remaining != 0 {
                factor = factor.multiply(factor);
            }
        }
        Ok(result)
    }
}

#[derive(Clone, Copy)]
pub(super) enum Number {
    Integer(i64),
    Rational(Rational),
    Float(f64),
    Complex(ComplexNumber),
}

impl Number {
    pub(super) fn as_float(self) -> f64 {
        match self {
            Self::Integer(value) => value as f64,
            Self::Rational(value) => value.numerator() as f64 / value.denominator() as f64,
            Self::Float(value) => value,
            Self::Complex(_) => unreachable!("complex number is not a real number"),
        }
    }

    fn as_complex(self) -> ComplexNumber {
        match self {
            Self::Integer(value) => ComplexNumber::new(value as f64, 0.0),
            Self::Rational(value) => {
                ComplexNumber::new(value.numerator() as f64 / value.denominator() as f64, 0.0)
            }
            Self::Float(value) => ComplexNumber::new(value, 0.0),
            Self::Complex(value) => value,
        }
    }

    fn is_float(&self) -> bool {
        matches!(self, Self::Float(_))
    }

    fn is_complex(&self) -> bool {
        matches!(self, Self::Complex(_))
    }

    fn is_zero(self) -> bool {
        match self {
            Self::Integer(value) => value == 0,
            Self::Rational(value) => value.numerator() == 0,
            Self::Float(value) => value == 0.0,
            Self::Complex(value) => value.is_zero(),
        }
    }

    fn exact_parts(self) -> Option<(i64, i64)> {
        match self {
            Self::Integer(value) => Some((value, 1)),
            Self::Rational(value) => Some((value.numerator(), value.denominator())),
            Self::Float(_) | Self::Complex(_) => None,
        }
    }
}

impl Value {
    fn as_integer(&self) -> Option<i64> {
        match self {
            Self::Integer(value) => Some(*value),
            _ => None,
        }
    }
}

pub(super) fn number(value: &Value) -> Result<Number, RuntimeError> {
    match value {
        Value::Integer(value) => Ok(Number::Integer(*value)),
        Value::Rational(value) => Ok(Number::Rational(*value)),
        Value::Float(value) => Ok(Number::Float(*value)),
        Value::Complex { real, imaginary } => Ok(Number::Complex(ComplexNumber::new(
            real_number_value(real)?,
            real_number_value(imaginary)?,
        ))),
        value => Err(number_error("numeric operation", value)),
    }
}

pub(super) fn number_argument(function: &str, value: &Value) -> Result<Number, RuntimeError> {
    match value {
        Value::Integer(value) => Ok(Number::Integer(*value)),
        Value::Rational(value) => Ok(Number::Rational(*value)),
        Value::Float(value) => Ok(Number::Float(*value)),
        Value::Complex { real, imaginary } => Ok(Number::Complex(ComplexNumber::new(
            real_number_value(real)?,
            real_number_value(imaginary)?,
        ))),
        value => Err(number_error(function, value)),
    }
}

pub(super) fn real_number_value(value: &Value) -> Result<f64, RuntimeError> {
    match value {
        Value::Integer(value) => Ok(*value as f64),
        Value::Rational(value) => Ok(value.numerator() as f64 / value.denominator() as f64),
        Value::Float(value) => Ok(*value),
        value => Err(number_error("numeric operation", value)),
    }
}

pub(super) fn real_number_argument(function: &str, value: &Value) -> Result<Number, RuntimeError> {
    let number = number_argument(function, value)?;
    if number.is_complex() {
        return Err(type_error(function, "a real number", value));
    }
    Ok(number)
}

pub(super) fn real_value_argument(function: &str, value: &Value) -> Result<Value, RuntimeError> {
    if value.is_real_number() {
        Ok(value.clone())
    } else {
        Err(type_error(function, "a real number", value))
    }
}

pub(super) fn number_to_value(number: Number) -> Result<Value, RuntimeError> {
    match number {
        Number::Integer(value) => Ok(Value::Integer(value)),
        Number::Rational(value) => Value::rational(
            i128::from(value.numerator()),
            i128::from(value.denominator()),
        ),
        Number::Float(value) => Ok(Value::Float(value)),
        Number::Complex(value) => {
            Value::complex(Value::Float(value.real), Value::Float(value.imaginary))
        }
    }
}

pub(super) fn binary_number(
    left: Number,
    right: Number,
    operation: char,
) -> Result<Number, RuntimeError> {
    if left.is_complex() || right.is_complex() {
        let left = left.as_complex();
        let right = right.as_complex();
        return match operation {
            '+' => Ok(Number::Complex(left.add(right))),
            '-' => Ok(Number::Complex(left.subtract(right))),
            '*' => Ok(Number::Complex(left.multiply(right))),
            '/' => Ok(Number::Complex(left.divide(right)?)),
            _ => unreachable!("unsupported complex numeric operation"),
        };
    }
    if left.is_float() || right.is_float() {
        let left = left.as_float();
        let right = right.as_float();
        return match operation {
            '+' => Ok(Number::Float(left + right)),
            '-' => Ok(Number::Float(left - right)),
            '*' => Ok(Number::Float(left * right)),
            '/' if right == 0.0 => Err(RuntimeError::DivisionByZero),
            '/' => Ok(Number::Float(left / right)),
            _ => unreachable!("unsupported floating-point operation"),
        };
    }
    exact_binary(left, right, operation)
}

pub(super) fn rational_number(numerator: i128, denominator: i128) -> Result<Number, RuntimeError> {
    let value = Rational::new(numerator, denominator)?;
    if value.denominator() == 1 {
        Ok(Number::Integer(value.numerator()))
    } else {
        Ok(Number::Rational(value))
    }
}

pub(super) fn exact_binary(
    left: Number,
    right: Number,
    operation: char,
) -> Result<Number, RuntimeError> {
    let (left_numerator, left_denominator) = left
        .exact_parts()
        .expect("exact numeric operation received a float");
    let (right_numerator, right_denominator) = right
        .exact_parts()
        .expect("exact numeric operation received a float");
    let left_numerator = i128::from(left_numerator);
    let left_denominator = i128::from(left_denominator);
    let right_numerator = i128::from(right_numerator);
    let right_denominator = i128::from(right_denominator);
    let (numerator, denominator) = match operation {
        '+' => (
            left_numerator * right_denominator + right_numerator * left_denominator,
            left_denominator * right_denominator,
        ),
        '-' => (
            left_numerator * right_denominator - right_numerator * left_denominator,
            left_denominator * right_denominator,
        ),
        '*' => (
            left_numerator * right_numerator,
            left_denominator * right_denominator,
        ),
        '/' => (
            left_numerator * right_denominator,
            left_denominator * right_numerator,
        ),
        _ => unreachable!("unsupported exact numeric operation"),
    };
    rational_number(numerator, denominator)
}

pub(super) fn negate_number(value: Number) -> Result<Number, RuntimeError> {
    match value {
        Number::Integer(value) => value
            .checked_neg()
            .map(Number::Integer)
            .ok_or(RuntimeError::NumericOverflow),
        Number::Rational(value) => rational_number(
            -i128::from(value.numerator()),
            i128::from(value.denominator()),
        ),
        Number::Float(value) => Ok(Number::Float(-value)),
        Number::Complex(value) => Ok(Number::Complex(ComplexNumber::new(
            -value.real,
            -value.imaginary,
        ))),
    }
}

pub(super) fn compare_number_values(left: Number, right: Number) -> Ordering {
    if left.is_float() || right.is_float() {
        return left
            .as_float()
            .partial_cmp(&right.as_float())
            .unwrap_or(Ordering::Equal);
    }
    let (left_numerator, left_denominator) = left
        .exact_parts()
        .expect("exact numeric comparison received a float");
    let (right_numerator, right_denominator) = right
        .exact_parts()
        .expect("exact numeric comparison received a float");
    (i128::from(left_numerator) * i128::from(right_denominator))
        .cmp(&(i128::from(right_numerator) * i128::from(left_denominator)))
}

pub(super) fn numeric_equalp(left: Number, right: Number) -> bool {
    if left.is_complex() || right.is_complex() {
        let left = left.as_complex();
        let right = right.as_complex();
        return left.real == right.real && left.imaginary == right.imaginary;
    }
    compare_number_values(left, right) == Ordering::Equal
}

pub(super) fn integer_argument(function: &str, value: &Value) -> Result<i64, RuntimeError> {
    value
        .as_integer()
        .ok_or_else(|| type_error(function, "integer", value))
}

pub(super) fn number_error(function: &str, value: &Value) -> RuntimeError {
    type_error(function, "number", value)
}
