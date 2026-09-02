use crate::builtins::*;
use crate::RuntimeError;

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
fn make_array_validates_element_type_against_initial_contents() {
    let array = make_array(&[
        Value::Integer(2),
        Value::keyword("element-type"),
        Value::symbol("integer"),
        Value::keyword("initial-contents"),
        Value::list(vec![Value::Integer(1), Value::Integer(2)]),
    ])
    .expect("integer initial contents should satisfy integer element type");
    assert_eq!(array.array_element_type().map(|value| value.to_string()), Some("INTEGER".to_string()));

    let error = make_array(&[
        Value::Integer(1),
        Value::keyword("element-type"),
        Value::symbol("integer"),
        Value::keyword("initial-element"),
        Value::symbol("not-an-integer"),
    ])
    .expect_err("non-integer initial element must be rejected");
    assert!(matches!(error, RuntimeError::InvalidForm { message, .. } if message.contains("element type")));
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
    assert!(matches!(
        make_array(&[
            Value::Integer(2),
            Value::keyword("fill-pointer"),
            Value::Integer(3),
        ]),
        Err(RuntimeError::InvalidForm { message, .. })
            if message.contains("fill pointer exceeds vector length")
    ));
}

#[test]
fn array_has_fill_pointer_p_distinguishes_vectors_and_arrays() {
    let vector = make_array(&[
        Value::Integer(2),
        Value::keyword("fill-pointer"),
        Value::Integer(1),
    ])
    .expect("make-array should construct a vector");
    let simple_vector =
        make_array(&[Value::Integer(2)]).expect("make-array should construct a vector");
    let array = make_array(&[Value::list(vec![Value::Integer(1), Value::Integer(2)])])
        .expect("make-array should construct an array");

    assert!(array_has_fill_pointer_p(&[vector])
        .unwrap()
        .equal_value(&Value::Boolean(true)));
    assert!(array_has_fill_pointer_p(&[simple_vector])
        .unwrap()
        .equal_value(&Value::Boolean(false)));
    assert!(array_has_fill_pointer_p(&[array])
        .unwrap()
        .equal_value(&Value::Boolean(false)));
    assert!(matches!(
        array_has_fill_pointer_p(&[Value::Integer(1)]),
        Err(RuntimeError::Type { expected, .. }) if expected.contains("array")
    ));
}

#[test]
fn array_metadata_reports_adjustability_and_displacement() {
    let base = make_array(&[Value::Integer(4)]).expect("make-array should construct a vector");
    let displaced = make_array(&[
        Value::Integer(2),
        Value::keyword("displaced-to"),
        base.clone(),
        Value::keyword("displaced-index-offset"),
        Value::Integer(1),
    ])
    .expect("make-array should construct a displaced vector");

    assert!(adjustable_array_p(&[base.clone()])
        .unwrap()
        .equal_value(&Value::Boolean(false)));
    assert!(array_displacement(&[base.clone()])
        .unwrap()
        .equal_value(&Value::values(vec![Value::Nil, Value::Integer(0)])));
    let displacement = array_displacement(&[displaced]).expect("displacement should be reported");
    assert!(displacement.equal_value(&Value::values(vec![base, Value::Integer(1)])));
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
fn adjust_array_accepts_and_validates_element_type() {
    let vector = make_array(&[
        Value::Integer(2),
        Value::keyword("initial-element"),
        Value::Integer(1),
        Value::keyword("adjustable"),
        Value::Boolean(true),
    ])
    .expect("make-array should construct an adjustable vector");
    let adjusted = adjust_array(&[
        vector.clone(),
        Value::Integer(3),
        Value::keyword("element-type"),
        Value::symbol("bit"),
        Value::keyword("initial-element"),
        Value::Integer(0),
    ])
    .expect("adjust-array should accept :element-type");

    assert!(adjusted
        .array_element_type()
        .is_some_and(|value| value.equal_value(&Value::symbol("BIT"))));
    assert_eq!(adjusted.vector_items().unwrap().len(), 3);
}

#[test]
fn adjust_array_rejects_element_type_without_mutating_adjustable_vector() {
    let vector = make_array(&[
        Value::Integer(2),
        Value::keyword("initial-element"),
        Value::Integer(1),
        Value::keyword("adjustable"),
        Value::Boolean(true),
    ])
    .expect("make-array should construct an adjustable vector");
    let result = adjust_array(&[
        vector.clone(),
        Value::Integer(3),
        Value::keyword("element-type"),
        Value::symbol("bit"),
        Value::keyword("initial-element"),
        Value::String("invalid".into()),
    ]);

    assert!(result.is_err());
    assert!(vector
        .array_element_type()
        .is_some_and(|value| value.equal_value(&Value::symbol("T"))));
    assert_eq!(vector.vector_items().unwrap().len(), 2);
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
fn adjust_array_updates_adjustable_metadata() {
    let vector = make_array(&[Value::Integer(2)]).expect("make-array should construct a vector");
    let adjusted = adjust_array(&[
        vector,
        Value::Integer(3),
        Value::keyword("adjustable"),
        Value::Boolean(true),
    ])
    .expect("adjust-array should accept :adjustable");

    assert!(adjustable_array_p(&[adjusted]).unwrap().is_truthy());

    let adjustable = make_array(&[
        Value::Integer(2),
        Value::keyword("adjustable"),
        Value::Boolean(true),
    ])
    .expect("make-array should construct an adjustable vector");
    let adjusted = adjust_array(&[
        adjustable,
        Value::Integer(3),
        Value::keyword("adjustable"),
        Value::Nil,
    ])
    .expect("adjust-array should update :adjustable");
    assert!(!adjustable_array_p(&[adjusted]).unwrap().is_truthy());
}

#[test]
fn adjust_array_preserves_displacement_aliasing() {
    let base = make_array(&[
        Value::Integer(4),
        Value::keyword("initial-contents"),
        Value::list(vec![
            Value::Integer(1),
            Value::Integer(2),
            Value::Integer(3),
            Value::Integer(4),
        ]),
    ])
    .expect("make-array should construct a base vector");
    let displaced = adjust_array(&[
        make_array(&[Value::Integer(2)]).expect("make-array should construct a vector"),
        Value::Integer(2),
        Value::keyword("displaced-to"),
        base.clone(),
        Value::keyword("displaced-index-offset"),
        Value::Integer(1),
    ])
    .expect("adjust-array should construct a displaced vector");

    assert!(array_displacement(&[displaced.clone()])
        .unwrap()
        .equal_value(&Value::values(vec![base.clone(), Value::Integer(1)])));
    assert!(aref(&[displaced.clone(), Value::Integer(0)])
        .unwrap()
        .equal_value(&Value::Integer(2)));
    displaced.set_vector_item(0, Value::Integer(9));
    assert!(aref(&[base, Value::Integer(1)])
        .unwrap()
        .equal_value(&Value::Integer(9)));
}

#[test]
fn adjust_array_keeps_adjustable_displacement_in_place() {
    let base = Value::vector(vec![
        Value::Integer(1),
        Value::Integer(2),
        Value::Integer(3),
        Value::Integer(4),
    ]);
    let displaced = adjust_array(&[
        make_array(&[
            Value::Integer(2),
            Value::keyword("adjustable"),
            Value::Boolean(true),
            Value::keyword("displaced-to"),
            base.clone(),
            Value::keyword("displaced-index-offset"),
            Value::Integer(1),
        ])
        .unwrap(),
        Value::Integer(3),
    ])
    .unwrap();

    assert!(array_displacement(&[displaced.clone()])
        .unwrap()
        .equal_value(&Value::values(vec![base.clone(), Value::Integer(1)])));
    assert_eq!(displaced.vector_items().unwrap().len(), 3);
    displaced.set_vector_item(2, Value::Integer(9));
    assert!(aref(&[base, Value::Integer(3)])
        .unwrap()
        .equal_value(&Value::Integer(9)));
}

#[test]
fn adjust_array_writes_displaced_initial_contents_to_the_target() -> Result<(), RuntimeError> {
    let base = make_array(&[Value::Integer(5), Value::keyword("initial-element"), Value::Integer(0)])?;
    let displaced = make_array(&[
        Value::Integer(3),
        Value::keyword("displaced-to"),
        base.clone(),
        Value::keyword("displaced-index-offset"),
        Value::Integer(1),
        Value::keyword("adjustable"),
        Value::Boolean(true),
    ])?;

    adjust_array(&[
        displaced.clone(),
        Value::Integer(3),
        Value::keyword("initial-contents"),
        Value::list(vec![Value::Integer(7), Value::Integer(8), Value::Integer(9)]),
    ])?;

    assert_eq!(aref(&[base, Value::Integer(1)])?.to_string(), "7");
    assert_eq!(aref(&[displaced, Value::Integer(2)])?.to_string(), "9");
    Ok(())
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
    assert!(vector_push(&[Value::Integer(7), vector.clone()])
        .unwrap()
        .equal_value(&Value::Integer(0)));
    assert!(fill_pointer(&[vector.clone()])
        .unwrap()
        .equal_value(&Value::Integer(1)));
    assert!(vector_push_extend(&[Value::Integer(8), vector.clone()])
        .unwrap()
        .equal_value(&Value::Integer(1)));
    assert!(vector.vector_items().unwrap()[1].equal_value(&Value::Integer(8)));
}

#[test]
fn vector_push_extend_rejects_zero_extension() {
    let vector = make_array(&[
        Value::Integer(1),
        Value::keyword("fill-pointer"),
        Value::Integer(1),
        Value::keyword("adjustable"),
        Value::Boolean(true),
    ])
    .unwrap();
    assert!(vector_push_extend(&[Value::Integer(7), vector, Value::Integer(0),]).is_err());
}

#[test]
fn vector_push_rejects_values_outside_the_element_type() {
    let vector = make_array(&[
        Value::Integer(1),
        Value::keyword("element-type"),
        Value::symbol("bit"),
        Value::keyword("initial-element"),
        Value::Integer(0),
        Value::keyword("fill-pointer"),
        Value::Integer(0),
    ])
    .unwrap();

    assert!(vector_push(&[Value::Integer(2), vector.clone()]).is_err());
    assert!(fill_pointer(&[vector]).unwrap().equal_value(&Value::Integer(0)));
}

#[test]
fn vector_push_extend_rejects_values_before_extending() {
    let vector = make_array(&[
        Value::Integer(1),
        Value::keyword("element-type"),
        Value::symbol("bit"),
        Value::keyword("initial-element"),
        Value::Integer(0),
        Value::keyword("fill-pointer"),
        Value::Integer(1),
        Value::keyword("adjustable"),
        Value::Boolean(true),
    ])
    .unwrap();

    assert!(vector_push_extend(&[Value::Integer(2), vector.clone()]).is_err());
    assert_eq!(vector.vector_items().unwrap().len(), 1);
}

#[test]
fn vector_pop_decrements_fill_pointer() {
    let vector = make_array(&[
        Value::Integer(2),
        Value::keyword("initial-contents"),
        Value::list(vec![Value::Integer(3), Value::Integer(4)]),
        Value::keyword("fill-pointer"),
        Value::Integer(2),
    ])
    .unwrap();
    assert!(vector_pop(&[vector.clone()])
        .unwrap()
        .equal_value(&Value::Integer(4)));
    assert!(fill_pointer(&[vector])
        .unwrap()
        .equal_value(&Value::Integer(1)));
}

#[test]
fn make_array_displaces_vector_storage() {
    let target = Value::vector(vec![
        Value::Integer(1),
        Value::Integer(2),
        Value::Integer(3),
        Value::Integer(4),
    ]);
    let displaced = make_array(&[
        Value::Integer(2),
        Value::keyword("displaced-to"),
        target.clone(),
        Value::keyword("displaced-index-offset"),
        Value::Integer(1),
    ])
    .unwrap();
    let items = displaced.vector_items().unwrap();
    assert!(items[0].equal_value(&Value::Integer(2)) && items[1].equal_value(&Value::Integer(3)));
    displaced.set_vector_item(0, Value::Integer(9));
    assert!(target.vector_items().unwrap()[1].equal_value(&Value::Integer(9)));
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
