use crate::RuntimeError;
use crate::builtins::*;

mod dimensions;

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
fn make_array_tracks_vector_fill_pointer_and_adjustability() {
    let vector = make_array(&[
        Value::Integer(4),
        Value::keyword("fill-pointer"),
        Value::Integer(2),
        Value::keyword("adjustable"),
        Value::Boolean(true),
    ])
    .expect("make-array should construct a vector");

    assert_eq!(vector.vector_length(), Some(2));
    assert_eq!(vector.vector_adjustable(), Some(true));
    assert!(matches!(
        make_array(&[
            Value::list(vec![Value::Integer(2), Value::Integer(2)]),
            Value::keyword("fill-pointer"),
            Value::Integer(1),
        ]),
        Err(RuntimeError::InvalidForm { message, .. })
            if message.contains("fill pointer requires a vector")
    ));
}

#[test]
fn adjust_array_resizes_adjustable_vector_in_place() {
    let vector = make_array(&[
        Value::Integer(2),
        Value::keyword("initial-contents"),
        Value::list(vec![Value::Integer(7), Value::Integer(8)]),
        Value::keyword("adjustable"),
        Value::Boolean(true),
        Value::keyword("fill-pointer"),
        Value::Integer(1),
    ])
    .expect("make-array should construct an adjustable vector");
    let adjusted = adjust_array(&[vector.clone(), Value::Integer(3)])
        .expect("adjust-array should resize an adjustable vector");
    assert!(matches!(adjusted, Value::Vector(_)));
    assert_eq!(adjusted.vector_items().unwrap().len(), 3);
    assert_eq!(adjusted.vector_length(), Some(1));
    assert_eq!(vector.vector_items().unwrap().len(), 3);
}

#[test]
fn adjust_array_updates_vector_fill_pointer() {
    let vector = make_array(&[
        Value::Integer(3),
        Value::keyword("adjustable"),
        Value::Boolean(true),
        Value::keyword("fill-pointer"),
        Value::Integer(1),
    ])
    .expect("make-array should construct a vector");

    let adjusted = adjust_array(&[
        vector.clone(),
        Value::Integer(4),
        Value::keyword("fill-pointer"),
        Value::Integer(3),
    ])
    .expect("adjust-array should accept :fill-pointer");

    assert_eq!(adjusted.vector_length(), Some(3));
    assert_eq!(vector.vector_length(), Some(3));
    assert!(matches!(
        adjust_array(&[
            vector,
            Value::Integer(2),
            Value::keyword("fill-pointer"),
            Value::Integer(3),
        ]),
        Err(RuntimeError::InvalidForm { message, .. })
            if message.contains("fill pointer exceeds vector length")
    ));
}

#[test]
fn vector_push_uses_and_extends_fill_pointer() {
    let vector = make_array(&[
        Value::Integer(3),
        Value::keyword("fill-pointer"),
        Value::Integer(0),
        Value::keyword("adjustable"),
        Value::Boolean(true),
    ])
    .unwrap();
    assert!(vector_push(&[Value::Integer(7), vector.clone()]).unwrap().equal_value(&Value::Integer(0)));
    assert!(fill_pointer(&[vector.clone()]).unwrap().equal_value(&Value::Integer(1)));
    assert!(vector_push_extend(&[Value::Integer(8), vector.clone()]).unwrap().equal_value(&Value::Integer(1)));
    assert!(vector.vector_items().unwrap()[1].equal_value(&Value::Integer(8)));
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
