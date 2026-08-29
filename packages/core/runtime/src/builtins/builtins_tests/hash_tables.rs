use crate::RuntimeError;
use crate::builtins::*;

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
