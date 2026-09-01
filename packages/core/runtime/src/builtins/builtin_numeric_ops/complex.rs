use super::{RuntimeError, Value, exact, number_argument};

pub fn complex(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(super::arity("complex", "one or two", arguments.len()));
    }
    let (real, _) = components("complex", &arguments[0])?;
    let imag = arguments
        .get(1)
        .map(|value| components("complex", value).map(|(real, _)| real))
        .transpose()?
        .unwrap_or(Value::Integer(0));
    Ok(Value::complex(real, imag))
}

pub fn complex_add(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut real = Value::Integer(0);
    let mut imag = Value::Integer(0);
    for argument in arguments {
        let (argument_real, argument_imag) = components("+", argument)?;
        real = super::add(&[real, argument_real])?;
        imag = super::add(&[imag, argument_imag])?;
    }
    Ok(Value::complex(real, imag))
}

pub fn complex_subtract(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let (first_real, first_imag) = components("-", &arguments[0])?;
    let (mut real, mut imag) = if arguments.len() == 1 {
        (
            super::subtract(&[first_real])?,
            super::subtract(&[first_imag])?,
        )
    } else {
        (first_real, first_imag)
    };
    for argument in &arguments[1..] {
        let (argument_real, argument_imag) = components("-", argument)?;
        real = super::subtract(&[real, argument_real])?;
        imag = super::subtract(&[imag, argument_imag])?;
    }
    Ok(Value::complex(real, imag))
}

pub fn complex_multiply(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = (Value::Integer(1), Value::Integer(0));
    for argument in arguments {
        let (left_real, left_imag) = result;
        let (right_real, right_imag) = components("*", argument)?;
        let real = super::subtract(&[
            super::multiply(&[left_real.clone(), right_real.clone()])?,
            super::multiply(&[left_imag.clone(), right_imag.clone()])?,
        ])?;
        let imag = super::add(&[
            super::multiply(&[left_real, right_imag.clone()])?,
            super::multiply(&[left_imag, right_real])?,
        ])?;
        result = (real, imag);
    }
    Ok(Value::complex(result.0, result.1))
}

pub fn complex_divide(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let (mut real, mut imag) = components("/", &arguments[0])?;
    if arguments.len() == 1 {
        let denominator = super::add(&[
            super::multiply(&[real.clone(), real.clone()])?,
            super::multiply(&[imag.clone(), imag.clone()])?,
        ])?;
        real = super::divide(&[real, denominator.clone()])?;
        imag = super::divide(&[Value::Integer(-1), denominator])?;
        imag = super::multiply(&[imag, components("/", &arguments[0])?.1])?;
        return Ok(Value::complex(real, imag));
    }
    for argument in &arguments[1..] {
        let (right_real, right_imag) = components("/", argument)?;
        let denominator = super::add(&[
            super::multiply(&[right_real.clone(), right_real.clone()])?,
            super::multiply(&[right_imag.clone(), right_imag.clone()])?,
        ])?;
        let next_real = super::divide(&[
            super::add(&[
                super::multiply(&[real.clone(), right_real.clone()])?,
                super::multiply(&[imag.clone(), right_imag.clone()])?,
            ])?,
            denominator.clone(),
        ])?;
        let next_imag = super::divide(&[
            super::subtract(&[
                super::multiply(&[imag, right_real])?,
                super::multiply(&[real, right_imag])?,
            ])?,
            denominator,
        ])?;
        real = next_real;
        imag = next_imag;
    }
    Ok(Value::complex(real, imag))
}

fn components(function: &str, value: &Value) -> Result<(Value, Value), RuntimeError> {
    match value {
        Value::Complex(value) => {
            if value.real.is_complex() || value.imag.is_complex() {
                return Err(RuntimeError::Type {
                    expected: "REAL".to_owned(),
                    actual: "COMPLEX".to_owned(),
                    span: None,
                });
            }
            Ok((value.real.clone(), value.imag.clone()))
        }
        value if value.is_number() => Ok((value.clone(), Value::Integer(0))),
        value => Err(RuntimeError::Type {
            expected: "NUMBER".to_owned(),
            actual: format!("{} in {function}", value.type_name()),
            span: None,
        }),
    }
}

pub fn real_part(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "realpart", 1)?;
    match &arguments[0] {
        Value::Complex(value) => Ok(value.real.clone()),
        value if value.is_number() => Ok(value.clone()),
        value => Err(RuntimeError::Type {
            expected: "NUMBER".to_owned(),
            actual: value.type_name().to_owned(),
            span: None,
        }),
    }
}

pub fn imaginary_part(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "imagpart", 1)?;
    match &arguments[0] {
        Value::Complex(value) => Ok(value.imag.clone()),
        value if value.is_number() => Ok(Value::Integer(0)),
        value => Err(RuntimeError::Type {
            expected: "NUMBER".to_owned(),
            actual: value.type_name().to_owned(),
            span: None,
        }),
    }
}

pub fn conjugate(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "conjugate", 1)?;
    match &arguments[0] {
        Value::Complex(value) => Ok(Value::complex(
            value.real.clone(),
            negate_real(&value.imag)?,
        )),
        value if value.is_number() => Ok(value.clone()),
        value => Err(RuntimeError::Type {
            expected: "NUMBER".to_owned(),
            actual: value.type_name().to_owned(),
            span: None,
        }),
    }
}

pub fn phase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "phase", 1)?;
    let (real, imag) = match &arguments[0] {
        Value::Complex(value) => (&value.real, &value.imag),
        value if value.is_number() => (value, &Value::Integer(0)),
        value => {
            return Err(RuntimeError::Type {
                expected: "NUMBER".to_owned(),
                actual: value.type_name().to_owned(),
                span: None,
            });
        }
    };
    Ok(Value::Float(
        number_argument("phase", imag)?.as_float().atan2(number_argument("phase", real)?.as_float()),
    ))
}

fn negate_real(value: &Value) -> Result<Value, RuntimeError> {
    super::negate_number(super::number_argument("conjugate", value)?)
        .and_then(super::number_to_value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn realpart_and_imagpart_extract_complex_components() {
        let value = Value::complex(Value::Integer(2), Value::Integer(3));

        assert_eq!(real_part(&[value.clone()]).unwrap().to_string(), "2");
        assert_eq!(imaginary_part(&[value]).unwrap().to_string(), "3");
    }

    #[test]
    fn real_numbers_have_zero_imaginary_part() {
        assert_eq!(
            imaginary_part(&[Value::Integer(7)]).unwrap().to_string(),
            "0"
        );
        assert_eq!(real_part(&[Value::Integer(7)]).unwrap().to_string(), "7");
    }

    #[test]
    fn conjugate_negates_the_imaginary_component() {
        let value = Value::complex(Value::Integer(2), Value::Integer(3));

        assert_eq!(conjugate(&[value]).unwrap().to_string(), "#C(2 -3)");
    }

    #[test]
    fn phase_uses_the_complex_argument_quadrant() {
        let result = phase(&[Value::complex(Value::Integer(-1), Value::Integer(1))]).unwrap();
        let Value::Float(result) = result else {
            panic!("phase did not return a float");
        };
        assert!((result - std::f64::consts::FRAC_PI_2 * 1.5).abs() < 1e-12);
    }

    #[test]
    fn arithmetic_combines_complex_components() {
        let left = Value::complex(Value::Integer(2), Value::Integer(3));
        let right = Value::complex(Value::Integer(4), Value::Integer(-1));

        assert_eq!(
            super::complex_add(&[left.clone(), right.clone()])
                .unwrap()
                .to_string(),
            "#C(6 2)"
        );
        assert_eq!(
            super::complex_multiply(&[left, right]).unwrap().to_string(),
            "#C(11 10)"
        );
    }
}
