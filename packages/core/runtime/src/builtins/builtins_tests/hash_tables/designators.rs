use std::rc::Rc;

use crate::Function;
use crate::RuntimeError;
use crate::builtins::builtin_hash_tables::{hash_table_option_name, hash_table_test_name};
use crate::builtins::*;

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
    for (test, left, right, expected) in [
        ("EQ", Value::Integer(1), Value::Integer(1), true),
        ("EQUAL", Value::string("x"), Value::string("x"), true),
        ("EQUALP", Value::string("x"), Value::string("X"), true),
        ("EQL", Value::Integer(1), Value::Integer(2), false),
    ] {
        assert_eq!(
            hash_table_key_equal(test, &left, &right),
            expected,
            "{test}"
        );
    }
    Ok(())
}

#[test]
fn hash_table_option_name_accepts_every_symbol_designator_variant() -> Result<(), RuntimeError> {
    for value in [
        Value::symbol("size"),
        Value::uninterned_symbol("size"),
        Value::symbol_exact("size"),
        Value::keyword_exact("size"),
    ] {
        assert_eq!(hash_table_option_name("test", &value)?, "SIZE");
    }
    Ok(())
}

#[test]
fn hash_table_test_name_accepts_every_symbol_designator_variant() -> Result<(), RuntimeError> {
    for value in [
        Value::symbol("eql"),
        Value::uninterned_symbol("eql"),
        Value::symbol_exact("eql"),
        Value::keyword_exact("eql"),
    ] {
        assert_eq!(hash_table_test_name("test", &value)?, "EQL");
    }
    Ok(())
}

#[test]
fn hash_table_test_name_rejects_an_unnamed_function() {
    let unnamed_function = Value::Function(Rc::new(Function::StructurePredicate {
        name: "some-structure".to_string(),
    }));
    let error = hash_table_test_name("test", &unnamed_function).map_or_else(
        |error| error,
        |value| panic!("a structure predicate has no test name, got {value:?}"),
    );
    assert!(matches!(error, RuntimeError::Type { .. }), "{error:?}");
}
