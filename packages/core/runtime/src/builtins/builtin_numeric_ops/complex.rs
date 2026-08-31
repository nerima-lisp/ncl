use super::{RuntimeError, Value, exact};

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
}
