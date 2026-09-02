use crate::RuntimeError;
use crate::builtins::builtin_hash_tables::{hash_table_option_name, hash_table_test_name};
use crate::builtins::*;

mod designators;

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
    assert!(hash_table_size(&[Value::Integer(1)]).is_err());
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
fn make_hash_table_size_sets_initial_capacity() -> Result<(), RuntimeError> {
    let table = make_hash_table(&[Value::keyword("size"), Value::Integer(16)])?;
    assert!(hash_table_size(std::slice::from_ref(&table))?.as_integer() >= Some(16));
    Ok(())
}

#[test]
fn hash_table_rehash_options_are_exposed() -> Result<(), RuntimeError> {
    let default = make_hash_table(&[])?;
    assert_eq!(hash_table_rehash_size(&[default.clone()])?.to_string(), "1.5");
    assert_eq!(hash_table_rehash_threshold(&[default])?.to_string(), "0.75");
    let table = make_hash_table(&[
        Value::keyword("rehash-size"), Value::Integer(2),
        Value::keyword("rehash-threshold"), Value::Float(0.5),
    ])?;
    assert_eq!(hash_table_rehash_size(&[table.clone()])?.to_string(), "2.0");
    assert_eq!(hash_table_rehash_threshold(&[table])?.to_string(), "0.5");
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
    assert!(
        hash_table_size(std::slice::from_ref(&table))?
            .as_integer()
            .is_some_and(|size| size >= 1)
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
