use crate::builtins::*;
use crate::RuntimeError;

#[test]
fn array_helpers_validate_dimensions_contents_and_indices() -> Result<(), RuntimeError> {
    assert_eq!(parse_array_dimensions("test", &Value::Nil), Ok(Vec::new()));
    assert!(
        matches!(parse_array_dimensions("test", &Value::Integer(2)), Ok(dimensions) if dimensions == vec![2])
    );
    assert!(parse_array_dimensions("test", &Value::Integer(-1)).is_err());
    assert!(parse_array_dimensions("test", &Value::string("bad")).is_err());
    assert!(parse_array_dimensions("test", &Value::list(vec![Value::Integer(-1)])).is_err());
    assert_eq!(
        parse_array_dimensions(
            "test",
            &Value::vector(vec![Value::Integer(2), Value::Integer(3)])
        ),
        Ok(vec![2, 3])
    );
    for option in [
        Value::keyword("initial-element"),
        Value::symbol("initial-contents"),
        Value::uninterned_symbol("adjustable"),
        Value::symbol_exact("fill-pointer"),
        Value::keyword_exact("element-type"),
    ] {
        assert!(!array_option_name("test", &option)?.is_empty());
    }
    assert!(array_option_name("test", &Value::Integer(1)).is_err());
    let mut output = Vec::new();
    flatten_array_contents(
        "test",
        &Value::list(vec![Value::list(vec![
            Value::Integer(1),
            Value::Integer(2),
        ])]),
        &[1, 2],
        &mut output,
    )?;
    assert!(matches!(
        output.as_slice(),
        [Value::Integer(1), Value::Integer(2)]
    ));
    output.clear();
    flatten_array_contents("test", &Value::Integer(1), &[], &mut output)?;
    assert!(matches!(output.as_slice(), [Value::Integer(1)]));
    assert!(flatten_array_contents("test", &Value::Integer(1), &[2], &mut output).is_err());
    assert!(
        flatten_array_contents(
            "test",
            &Value::list(vec![Value::Integer(1)]),
            &[2],
            &mut output
        )
        .is_err()
    );
    assert!(array_coordinate_index("test", &[2], &[Value::Integer(2)]).is_err());
    assert!(matches!(
        array_coordinate_index("test", &[2, 3], &[Value::Integer(1), Value::Integer(2)]),
        Ok(5)
    ));
    assert!(
        array_coordinate_index(
            "test",
            &[usize::MAX, usize::MAX],
            &[Value::Integer(1), Value::Integer(1)]
        )
        .is_err()
    );
    assert!(array_total_size_for("test", &[usize::MAX, 2]).is_err());
    Ok(())
}
