use super::*;

#[test]
fn core_printing_wrappers_cover_success_and_argument_errors() -> Result<(), RuntimeError> {
    let value = Value::string("hello");
    assert_eq!(
        identity(std::slice::from_ref(&value))?.to_string(),
        "\"hello\""
    );
    assert!(identity(&[]).is_err());
    assert!(identity(&[Value::Nil, Value::Nil]).is_err());

    let type_cases = [
        (Value::Integer(1), "INTEGER"),
        (Value::string("text"), "STRING"),
    ];
    for (input, expected) in type_cases {
        assert_eq!(type_of(&[input])?.to_string(), expected);
    }
    assert!(type_of(&[]).is_err());

    assert_eq!(
        princ(std::slice::from_ref(&value))?.to_string(),
        "\"hello\""
    );
    assert_eq!(
        prin1(std::slice::from_ref(&value))?.to_string(),
        "\"hello\""
    );
    assert_eq!(
        print_value(std::slice::from_ref(&value))?.to_string(),
        "\"hello\""
    );
    for primitive in [print_value, princ, prin1] {
        assert!(primitive(&[]).is_err());
        assert!(primitive(&[Value::Nil, Value::Nil, Value::Nil]).is_err());
    }
    Ok(())
}

#[test]
fn write_wrappers_cover_print_options_and_errors() -> Result<(), RuntimeError> {
    let value = Value::string("hello");
    let cases = [
        (vec![value.clone()], "\"\\\"hello\\\"\""),
        (
            vec![value.clone(), Value::keyword("escape"), Value::Nil],
            "\"hello\"",
        ),
    ];
    for (arguments, expected) in cases {
        assert_eq!(write_to_string(&arguments)?.to_string(), expected);
    }
    assert_eq!(
        write_value(std::slice::from_ref(&value))?.to_string(),
        "\"hello\""
    );
    assert!(write_value(&[]).is_err());
    assert!(write_to_string(&[]).is_err());
    assert!(write_to_string(&[value.clone(), Value::Integer(1)]).is_err());
    assert!(write_to_string(&[value.clone(), Value::keyword("stream"), Value::Nil]).is_err());
    assert!(write_to_string(&[value, Value::keyword("unknown"), Value::Nil]).is_err());
    Ok(())
}

#[test]
fn sequence_primitives_reject_bad_inputs_and_handle_zero_counts() {
    assert!(last(&[]).is_err());
    assert!(last(&[Value::Integer(1)]).is_err());
    assert!(matches!(
        last(&[Value::list(vec![Value::Integer(1)]), Value::Integer(0)]),
        Ok(Value::Nil)
    ));
    assert!(butlast(&[]).is_err());
    assert!(butlast(&[Value::Integer(1)]).is_err());
    assert!(copy_list(&[Value::Integer(1)]).is_err());
    assert!(copy_alist(&[Value::Integer(1)]).is_err());
    assert!(copy_alist(&[Value::list(vec![Value::Integer(1)])]).is_err());
}

#[test]
fn sequence_copy_primitives_cover_table_driven_success_cases() -> Result<(), RuntimeError> {
    type Primitive = fn(&[Value]) -> Result<Value, RuntimeError>;

    let list = Value::list(vec![Value::Integer(1), Value::Integer(2)]);
    let alist = Value::list(vec![Value::list(vec![
        Value::keyword("key"),
        Value::Integer(1),
    ])]);
    let cases: [(Primitive, Value, &str); 3] = [
        (last, list.clone(), "(2)"),
        (butlast, list.clone(), "(1)"),
        (copy_list, list, "(1 2)"),
    ];

    for (primitive, input, expected) in cases {
        assert_eq!(primitive(&[input])?.to_string(), expected);
    }
    assert_eq!(copy_alist(&[alist])?.to_string(), "((:KEY 1))");
    Ok(())
}

#[test]
fn hash_table_options_and_operations_cover_invalid_designators() {
    assert!(make_hash_table(&[Value::keyword("test")]).is_err());
    assert!(make_hash_table(&[Value::keyword("rehash-size"), Value::Integer(0)]).is_err());
    assert!(make_hash_table(&[Value::keyword("rehash-threshold"), Value::Integer(2)]).is_err());
    assert!(make_hash_table(&[Value::keyword("synchronized"), Value::Integer(1)]).is_err());
    assert!(make_hash_table(&[Value::keyword("unknown"), Value::Nil]).is_err());
    assert!(gethash(&[Value::Nil]).is_err());
    assert!(gethash(&[Value::Nil, Value::Integer(1)]).is_err());
    assert!(remhash(&[Value::Nil, Value::Integer(1)]).is_err());
    assert!(clrhash(&[Value::Integer(1)]).is_err());
    assert!(hash_table_count(&[Value::Integer(1)]).is_err());
    assert!(hash_table_test_value(&[Value::Integer(1)]).is_err());
    assert!(hash_table_option_name("test", &Value::Integer(1)).is_err());
    assert!(hash_table_test_name("test", &Value::Integer(1)).is_err());
    assert!(hash_table_test_name("test", &Value::symbol("nope")).is_err());
}

#[test]
fn hash_table_options_accept_valid_keyword_values() -> Result<(), RuntimeError> {
    let cases = [
        vec![Value::keyword("size"), Value::Integer(8)],
        vec![Value::keyword("rehash-size"), Value::Integer(2)],
        vec![Value::keyword("rehash-threshold"), Value::Float(0.75)],
        vec![Value::keyword("synchronized"), Value::Nil],
        vec![Value::keyword("synchronized"), Value::Boolean(true)],
    ];

    for arguments in cases {
        assert!(matches!(
            make_hash_table(&arguments)?,
            Value::HashTable { .. }
        ));
    }
    Ok(())
}

#[test]
fn data_helpers_cover_successful_table_cases() -> Result<(), RuntimeError> {
    let tree = Value::dotted_list(
        vec![Value::list(vec![Value::Integer(1), Value::Integer(2)])],
        Value::Integer(3),
    );
    assert_eq!(copy_tree(&[tree])?.to_string(), "((1 2) . 3)");
    let cases = [
        (
            Value::Vector(Rc::new(vec![Value::Integer(1), Value::Integer(2)])),
            vec![2],
        ),
        (
            Value::Array {
                dimensions: Rc::new(vec![1, 2]),
                elements: Rc::new(vec![Value::Integer(1), Value::Integer(2)]),
            },
            vec![1, 2],
        ),
    ];
    for (value, expected) in cases {
        assert_eq!(dimensions_for_array(&value), Some(expected));
        assert!(array_elements(&value).is_some());
    }
    assert!(sequence_items(&Value::list(vec![Value::Integer(1)])).is_some());
    assert!(sequence_items(&Value::Integer(1)).is_none());
    Ok(())
}

#[test]
fn hash_table_operations_cover_successful_table_cases() -> Result<(), RuntimeError> {
    let table = make_hash_table(&[])?;
    assert!(matches!(table, Value::HashTable { .. }));
    assert!(matches!(
        hash_table_p(std::slice::from_ref(&table))?,
        Value::Boolean(true)
    ));
    assert!(matches!(
        hash_table_p(&[Value::Nil])?,
        Value::Nil | Value::Boolean(false)
    ));
    if let Some(entries) = table.hash_table_entries() {
        entries
            .borrow_mut()
            .push((Value::keyword("present"), Value::Integer(7)));
    } else {
        return Err(RuntimeError::InvalidForm {
            message: "expected hash table entries".to_string(),
            span: None,
        });
    }
    assert!(matches!(
        gethash(&[Value::keyword("present"), table.clone()])?.primary_value(),
        Value::Integer(7)
    ));
    assert_eq!(
        gethash(&[Value::keyword("missing"), table.clone()])?
            .primary_value()
            .to_string(),
        "NIL"
    );
    assert_eq!(
        hash_table_count(std::slice::from_ref(&table))?.to_string(),
        "1"
    );
    assert_eq!(
        hash_table_test_value(std::slice::from_ref(&table))?.to_string(),
        "EQL"
    );
    assert_eq!(
        remhash(&[Value::keyword("missing"), table.clone()])?
            .primary_value()
            .to_string(),
        "NIL"
    );
    assert!(matches!(
        remhash(&[Value::keyword("present"), table.clone()])?,
        Value::Boolean(true)
    ));
    assert!(matches!(clrhash(&[table])?, Value::HashTable { .. }));
    Ok(())
}

#[test]
fn hash_table_designators_and_key_tests_cover_supported_variants() -> Result<(), RuntimeError> {
    for test in ["EQ", "EQL", "EQUAL", "EQUALP"] {
        let table = make_hash_table(&[Value::keyword("test"), Value::keyword(test)])?;
        assert_eq!(
            hash_table_test_value(std::slice::from_ref(&table))?.to_string(),
            test
        );
        assert!(matches!(
            gethash(&[Value::string("key"), table])?.primary_value(),
            Value::Nil
        ));
    }

    let builtin = Value::builtin("eql", make_hash_table);
    assert_eq!(hash_table_test_name("test", &builtin)?, "EQL");
    let primitive = Value::primitive("equalp");
    assert_eq!(hash_table_test_name("test", &primitive)?, "EQUALP");

    let equal_cases = [
        ("EQ", Value::Integer(1), Value::Integer(1), true),
        ("EQUAL", Value::string("x"), Value::string("x"), true),
        ("EQUALP", Value::string("x"), Value::string("X"), true),
        ("EQL", Value::Integer(1), Value::Integer(2), false),
    ];
    for (test, left, right, expected) in equal_cases {
        assert_eq!(
            hash_table_key_equal(test, &left, &right),
            expected,
            "{test}"
        );
    }
    Ok(())
}

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
fn reader_and_stream_builtins_cover_bounds_and_eof_modes() -> Result<(), RuntimeError> {
    assert!(read_from_string(&[]).is_err());
    assert!(read_from_string(&[Value::Integer(1)]).is_err());
    let parsed = read_from_string(&[Value::string("1 2")])?;
    let parsed_values = parsed.multiple_values();
    assert!(matches!(
        parsed_values.as_slice(),
        [Value::Integer(1), Value::Integer(2)]
    ));
    let parsed = read_from_string(&[Value::string("  1"), Value::Nil, Value::Nil])?;
    let parsed_values = parsed.multiple_values();
    assert_eq!(
        parsed_values.get(1).and_then(|value| match value {
            Value::Integer(position) => Some(*position),
            _ => None,
        }),
        Some(3)
    );
    assert!(read_from_string(&[Value::string(""), Value::Nil, Value::symbol("eof")]).is_ok());
    assert!(read_from_string(&[Value::string(""), Value::boolean(true)]).is_err());
    assert!(
        read_from_string(&[
            Value::string("1"),
            Value::Nil,
            Value::Nil,
            Value::keyword("start")
        ])
        .is_err()
    );
    assert!(
        read_from_string(&[
            Value::string("1"),
            Value::Nil,
            Value::Nil,
            Value::Integer(0),
            Value::Nil
        ])
        .is_err()
    );
    assert!(
        read_from_string(&[
            Value::string("1"),
            Value::Nil,
            Value::Nil,
            Value::keyword("start"),
            Value::Integer(1),
            Value::keyword("end"),
            Value::Integer(1)
        ])
        .is_ok()
    );
    assert!(
        read_from_string(&[
            Value::string("1 2"),
            Value::Nil,
            Value::symbol("eof"),
            Value::keyword("start"),
            Value::Integer(2),
            Value::keyword("end"),
            Value::Integer(3),
            Value::keyword("preserve-whitespace"),
            Value::boolean(true)
        ])
        .is_ok()
    );
    assert!(
        read_from_string(&[
            Value::string("1"),
            Value::Nil,
            Value::Nil,
            Value::keyword("unknown"),
            Value::Nil
        ])
        .is_err()
    );
    assert!(
        read_from_string(&[
            Value::string("1"),
            Value::Nil,
            Value::Nil,
            Value::keyword("start"),
            Value::Integer(2),
            Value::keyword("end"),
            Value::Integer(1)
        ])
        .is_err()
    );
    assert!(make_string_input_stream(&[]).is_err());
    assert!(make_string_input_stream(&[Value::Integer(1)]).is_err());
    assert!(make_string_input_stream(&[Value::string("abc"), Value::Integer(-1)]).is_err());
    assert!(
        make_string_input_stream(&[Value::string("abc"), Value::Integer(2), Value::Integer(1)])
            .is_err()
    );
    assert!(stream_bound("test", &Value::Integer(4), 3).is_err());
    Ok(())
}

#[test]
fn character_stream_builtins_cover_peek_unread_and_output_boundaries() -> Result<(), RuntimeError> {
    let input = make_string_input_stream(&[Value::string("  ab")])?;
    assert!(matches!(
        peek_char(std::slice::from_ref(&input))?,
        Value::Character(' ')
    ));
    assert!(matches!(
        peek_char(&[Value::boolean(true), input.clone()])?,
        Value::Character('a')
    ));
    assert!(matches!(
        peek_char(&[Value::Character('b'), input.clone()])?,
        Value::Character('b')
    ));
    assert!(matches!(
        read_char(std::slice::from_ref(&input))?,
        Value::Character('b')
    ));
    assert!(unread_char(&[Value::Character('b'), input.clone()]).is_ok());
    assert!(matches!(
        read_char(std::slice::from_ref(&input))?,
        Value::Character('b')
    ));
    assert!(peek_char(&[Value::Integer(1), input]).is_err());

    let output = make_string_output_stream(&[])?;
    assert!(matches!(
        write_char(&[Value::Character('x'), output.clone()])?,
        Value::Character('x')
    ));
    assert!(matches!(
        write_string(&[Value::string("y"), output.clone()])?,
        Value::String(_)
    ));
    assert!(matches!(terpri(std::slice::from_ref(&output))?, Value::Nil));
    assert!(fresh_line(std::slice::from_ref(&output)).is_ok());
    assert!(matches!(
        get_output_stream_string(&[output])?,
        Value::String(text) if text.as_ref() == "xy\n"
    ));
    Ok(())
}

#[test]
fn character_stream_builtins_cover_eof_states_and_stream_types() -> Result<(), RuntimeError> {
    type Reader = fn(&[Value]) -> Result<Value, RuntimeError>;
    let eof_cases: [(&str, Reader); 2] = [("read-char", read_char), ("read-line", read_line)];
    for (name, operation) in eof_cases {
        let stream = make_string_input_stream(&[Value::string("")])?;
        assert!(
            operation(std::slice::from_ref(&stream)).is_err(),
            "{name} should signal EOF"
        );
    }

    let stream = make_string_input_stream(&[Value::string("")])?;
    assert!(matches!(
        read_char(&[stream.clone(), Value::Nil, Value::Integer(7)])?,
        Value::Integer(7)
    ));
    assert!(matches!(
        peek_char(&[stream, Value::Nil, Value::Integer(8)])?,
        Value::Integer(8)
    ));

    let output = make_string_output_stream(&[])?;
    assert!(read_char(std::slice::from_ref(&output)).is_err());
    assert!(write_char(&[Value::Character('x'), Value::Nil]).is_ok());
    assert!(write_string(&[Value::string("x"), Value::Integer(1)]).is_err());
    assert!(fresh_line(&[Value::Integer(1)]).is_err());
    assert!(write_line(&[Value::string("x"), Value::Integer(1)]).is_err());
    Ok(())
}

#[test]
fn print_and_file_builtins_reject_invalid_options() {
    assert!(print_value(&[]).is_err());
    assert!(princ(&[]).is_err());
    assert!(prin1(&[]).is_err());
    assert!(write_value(&[]).is_err());
    assert!(write_to_string(&[]).is_err());
    assert!(
        write_to_string(&[Value::string("text"), Value::keyword("stream"), Value::Nil]).is_err()
    );
    assert!(parse_print_options("test", &[Value::keyword("escape")], false).is_err());
    assert!(parse_print_options("test", &[Value::keyword("unknown"), Value::Nil], false).is_err());
    assert!(open_file(&[]).is_err());
    assert!(open_file(&[Value::Integer(1)]).is_err());
    assert!(
        open_file(&[
            Value::string("missing"),
            Value::keyword("unknown"),
            Value::Nil
        ])
        .is_err()
    );
    assert!(probe_file(&[Value::Integer(1)]).is_err());
    assert!(delete_file(&[Value::Integer(1)]).is_err());
    assert!(rename_file(&[]).is_err());
    assert!(rename_file(&[Value::Integer(1), Value::Integer(2)]).is_err());
    assert!(file_write_date(&[Value::Integer(1)]).is_err());
    assert!(truename(&[Value::Integer(1)]).is_err());
    assert!(make_string_output_stream(&[Value::Integer(1)]).is_err());
    assert!(get_output_stream_string(&[Value::Integer(1)]).is_err());
}

#[test]
fn print_helpers_cover_table_driven_values_and_options() -> Result<(), RuntimeError> {
    let values = [
        (Value::string("text"), "\"text\"", "text"),
        (
            Value::list(vec![Value::Integer(1), Value::Integer(2)]),
            "(1 2)",
            "(1 2)",
        ),
        (
            Value::dotted_list(vec![Value::Integer(1)], Value::Integer(2)),
            "(1 . 2)",
            "(1 . 2)",
        ),
        (
            Value::dotted_list(Vec::new(), Value::Integer(2)),
            "(. 2)",
            "(. 2)",
        ),
        (
            Value::Vector(Rc::new(vec![Value::string("text"), Value::Integer(2)])),
            "#(\"text\" 2)",
            "#(text 2)",
        ),
        (
            Value::list(vec![Value::string("nested")]),
            "(\"nested\")",
            "(nested)",
        ),
    ];
    for (value, escaped, unescaped) in values {
        assert_eq!(printed_value(&value, true), escaped);
        assert_eq!(printed_value(&value, false), unescaped);
        let written = write_to_string(std::slice::from_ref(&value))?;
        assert_eq!(printed_value(&written, false), escaped);
    }

    let (escape, stream) = parse_print_options(
        "write",
        &[
            Value::keyword("escape"),
            Value::Nil,
            Value::keyword("stream"),
            Value::Nil,
        ],
        true,
    )?;
    assert!(!escape);
    assert!(matches!(stream, Some(Value::Nil)));
    Ok(())
}

#[test]
fn file_operations_cover_lifecycle_and_open_directions() -> Result<(), RuntimeError> {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| RuntimeError::InvalidForm {
            message: error.to_string(),
            span: None,
        })?
        .as_nanos();
    let root = std::env::temp_dir().join(format!("ncl-runtime-{suffix}"));
    let path = Value::string(root.to_string_lossy().to_string());
    let output = open_file(&[
        path.clone(),
        Value::keyword("direction"),
        Value::keyword("output"),
    ])?;
    write_line(&[Value::string("line"), output.clone()])?;
    close_stream(&[output])?;
    assert!(matches!(
        probe_file(std::slice::from_ref(&path))?,
        Value::String(_)
    ));
    assert!(matches!(
        open_file(&[
            path.clone(),
            Value::keyword("direction"),
            Value::keyword("probe"),
        ])?,
        Value::Stream(_)
    ));
    let input = open_file(std::slice::from_ref(&path))?;
    assert!(matches!(
        read_line(std::slice::from_ref(&input))?.primary_value(),
        Value::String(text) if text.as_ref() == "line"
    ));
    close_stream(&[input])?;
    let renamed = Value::string(format!("{}-renamed", root.display()));
    assert!(matches!(
        rename_file(&[path.clone(), renamed.clone()])?,
        Value::Values(_)
    ));
    assert!(
        file_write_date(std::slice::from_ref(&renamed))?
            .to_string()
            .parse::<i64>()
            .is_ok()
    );
    assert!(
        truename(std::slice::from_ref(&renamed))?
            .to_string()
            .contains("ncl-runtime-")
    );
    assert!(matches!(delete_file(&[renamed])?, Value::Boolean(true)));
    assert!(matches!(probe_file(&[path])?, Value::Nil));
    Ok(())
}

#[test]
fn open_keyword_options_cover_defaults_and_validation() -> Result<(), RuntimeError> {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| RuntimeError::InvalidForm {
            message: error.to_string(),
            span: None,
        })?
        .as_nanos();
    let missing_path = std::env::temp_dir().join(format!("ncl-open-options-{suffix}-missing"));
    let existing_path = std::env::temp_dir().join(format!("ncl-open-options-{suffix}-existing"));
    std::fs::write(&existing_path, "content")
        .map_err(|error| RuntimeError::Io(error.to_string()))?;
    let missing = Value::string(missing_path.to_string_lossy().to_string());
    let existing = Value::string(existing_path.to_string_lossy().to_string());

    assert!(open_file(std::slice::from_ref(&missing)).is_err());
    assert!(matches!(
        open_file(&[
            missing.clone(),
            Value::keyword("if-does-not-exist"),
            Value::keyword("nil"),
        ])?,
        Value::Nil
    ));
    let output = open_file(&[
        missing,
        Value::keyword("direction"),
        Value::keyword("output"),
        Value::keyword("element-type"),
        Value::keyword("character"),
        Value::keyword("external-format"),
        Value::keyword("utf-8"),
    ])?;
    close_stream(&[output])?;
    let io = open_file(&[
        existing.clone(),
        Value::keyword("direction"),
        Value::keyword("io"),
        Value::keyword("if-exists"),
        Value::keyword("overwrite"),
    ])?;
    close_stream(&[io])?;

    assert!(open_file(&[existing.clone(), Value::keyword("unknown"), Value::Nil]).is_err());
    assert!(open_file(&[existing.clone(), Value::keyword("direction")]).is_err());
    assert!(
        open_file(&[
            existing,
            Value::keyword("direction"),
            Value::keyword("unknown"),
        ])
        .is_err()
    );

    let _ = std::fs::remove_file(missing_path);
    let _ = std::fs::remove_file(existing_path);
    Ok(())
}
