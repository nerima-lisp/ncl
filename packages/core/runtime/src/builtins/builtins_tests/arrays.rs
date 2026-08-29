use crate::RuntimeError;
use crate::builtins::*;

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

#[test]
fn array_accessors_report_dimension_and_arity_mismatches() {
    assert!(matches!(
        bit(&[Value::vector(vec![Value::Integer(0)]), Value::Integer(0), Value::Integer(1)]),
        Err(RuntimeError::Arity { function, expected, actual })
            if function == "bit" && expected == "2" && actual == 3
    ));
    assert!(matches!(
        array_row_major_index(&[]),
        Err(RuntimeError::Arity { function, .. }) if function == "array-row-major-index"
    ));
    assert!(matches!(
        array_in_bounds_p(&[]),
        Err(RuntimeError::Arity { function, .. }) if function == "array-in-bounds-p"
    ));
}

#[test]
fn make_array_rejects_invalid_keyword_pairs() {
    assert!(matches!(
        make_array(&[Value::Integer(2), Value::keyword("initial-element")]),
        Err(RuntimeError::Arity { function, .. }) if function == "make-array"
    ));
    let error = make_array(&[
        Value::Integer(2),
        Value::keyword("initial-element"),
        Value::Integer(1),
        Value::keyword("initial-contents"),
        Value::list(vec![Value::Integer(1), Value::Integer(2)]),
    ])
    .map_or_else(
        |error| error,
        |value| panic!("combining :initial-element and :initial-contents must fail, got {value:?}"),
    );
    assert!(matches!(
        error,
        RuntimeError::InvalidForm { message, .. }
            if message.contains("cannot combine :initial-element and :initial-contents")
    ));
}

#[test]
fn array_coordinate_index_reports_overflow_in_stride_and_contribution() {
    let stride_overflow = array_coordinate_index(
        "test",
        &[2, usize::MAX, 2],
        &[Value::Integer(0), Value::Integer(0), Value::Integer(0)],
    )
    .map_or_else(
        |error| error,
        |value| {
            panic!("stride multiplication across remaining dimensions must overflow, got {value:?}")
        },
    );
    assert!(matches!(
        stride_overflow,
        RuntimeError::InvalidForm { message, .. } if message.contains("index is too large")
    ));

    let contribution_overflow = array_coordinate_index(
        "test",
        &[usize::MAX, 4],
        &[Value::Integer(i64::MAX), Value::Integer(0)],
    )
    .map_or_else(
        |error| error,
        |value| panic!("index multiplied by stride must overflow, got {value:?}"),
    );
    assert!(matches!(
        contribution_overflow,
        RuntimeError::InvalidForm { message, .. } if message.contains("index is too large")
    ));
}

#[test]
fn flatten_array_contents_propagates_nested_dimension_mismatch() {
    let mut output = Vec::new();
    let error = flatten_array_contents(
        "test",
        &Value::list(vec![Value::list(vec![Value::Integer(1)])]),
        &[1, 2],
        &mut output,
    )
    .map_or_else(
        |error| error,
        |value| panic!("inner row has the wrong length for its dimension, got {value:?}"),
    );
    assert!(matches!(
        error,
        RuntimeError::InvalidForm { message, .. }
            if message.contains("expected 2 elements, got 1")
    ));
}
