use super::*;

fn ok_string(result: Result<Value, RuntimeError>) -> String {
    match result {
        Ok(value) => value.to_string(),
        Err(error) => panic!("expected Ok, got {error:?}"),
    }
}

#[test]
fn exponentiate_handles_exact_and_float_powers() {
    assert_eq!(
        ok_string(exponentiate(&[Value::Integer(2), Value::Integer(3)])),
        "8",
    );
    assert_eq!(
        ok_string(exponentiate(&[Value::Integer(2), Value::Integer(-1)])),
        "1/2",
    );
    assert_eq!(
        ok_string(exponentiate(&[Value::Integer(2), Value::Float(0.5)])),
        2f64.sqrt().to_string(),
    );
}

#[test]
fn exponentiate_handles_complex_exact_and_principal_powers() {
    let base = Value::complex(Value::Integer(2), Value::Integer(3));
    assert_eq!(
        ok_string(exponentiate(&[base.clone(), Value::Integer(2)])),
        "#C(-5 12)",
    );
    assert_eq!(
        ok_string(exponentiate(&[base.clone(), Value::Integer(-1)])),
        "#C(2/13 -3/13)",
    );

    let result = exponentiate(&[base, Value::Float(0.5)])
        .unwrap_or_else(|error| panic!("expected complex principal power, got {error:?}"));
    let Value::Complex(result) = result else {
        panic!("expected complex principal power")
    };
    let real = number_argument("test", result.real()).unwrap().as_float();
    let imaginary = number_argument("test", result.imaginary())
        .unwrap()
        .as_float();
    assert!((real - 1.674149234851).abs() < 1e-7, "real={real}");
    assert!(
        (imaginary - 0.895977476129).abs() < 1e-7,
        "imaginary={imaginary}"
    );
}

#[test]
fn exponentiate_negative_real_fractional_power_is_complex() {
    let exponent = Value::rational(1, 2).unwrap_or_else(|error| panic!("valid rational: {error}"));
    let result = exponentiate(&[Value::Integer(-4), exponent])
        .unwrap_or_else(|error| panic!("expected complex principal power, got {error:?}"));
    let Value::Complex(result) = result else {
        panic!("expected complex principal power")
    };
    assert!(
        number_argument("test", result.real())
            .unwrap()
            .as_float()
            .abs()
            < 1e-12
    );
    assert_eq!(
        number_argument("test", result.imaginary())
            .unwrap()
            .as_float(),
        2.0
    );
}

#[test]
fn exponentiate_rejects_invalid_arity_and_arguments() {
    assert!(exponentiate(&[Value::Integer(2)]).is_err());
    assert!(exponentiate(&[Value::Nil, Value::Integer(1)]).is_err());
}

#[test]
fn exponentiate_zero_to_negative_power_is_division_by_zero() {
    assert!(matches!(
        exponentiate(&[Value::Integer(0), Value::Integer(-1)]),
        Err(RuntimeError::DivisionByZero)
    ));
}

#[test]
fn exact_power_rejects_a_non_exact_base() {
    assert!(matches!(
        exact_power(Number::Float(2.0), 2),
        Err(RuntimeError::InvalidForm { .. })
    ));
}

#[test]
fn checked_power_reports_overflow() {
    assert!(matches!(
        checked_power(i128::from(i64::MAX), 4),
        Err(RuntimeError::NumericOverflow)
    ));
}

#[test]
fn square_root_handles_rational_and_complex_inputs() {
    let non_perfect_square =
        Value::rational(2, 3).unwrap_or_else(|error| panic!("valid rational: {error}"));
    assert_eq!(
        ok_string(square_root(&[non_perfect_square])),
        (2f64 / 3f64).sqrt().to_string(),
    );

    let perfect_square =
        Value::rational(4, 9).unwrap_or_else(|error| panic!("valid rational: {error}"));
    assert_eq!(ok_string(square_root(&[perfect_square])), "2/3");

    assert_eq!(ok_string(square_root(&[Value::Integer(-4)])), "#C(0.0 2.0)");
    assert_eq!(ok_string(square_root(&[Value::Float(-1.0)])), "#C(0.0 1.0)");

    let complex = Value::complex(Value::Integer(3), Value::Integer(4));
    assert_eq!(ok_string(square_root(&[complex])), "#C(2.0 1.0)");

    let conjugate = Value::complex(Value::Integer(3), Value::Integer(-4));
    assert_eq!(ok_string(square_root(&[conjugate])), "#C(2.0 -1.0)");

    let float_complex = Value::complex(Value::Float(3.0), Value::Float(0.0));
    assert!(matches!(float_complex, Value::Complex(_)));
    assert_eq!(
        ok_string(square_root(&[float_complex])),
        format!("#C({} 0.0)", 3f64.sqrt()),
    );
}

#[test]
fn negative_real_error_reports_the_offending_function() {
    let error = negative_real_error("sqrt");
    assert!(matches!(error, RuntimeError::InvalidForm { message, .. } if message.contains("sqrt")));
}
