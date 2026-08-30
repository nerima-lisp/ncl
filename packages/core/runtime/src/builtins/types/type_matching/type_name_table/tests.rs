use super::type_matches;
use crate::value::RandomState;
use crate::{Environment, RuntimeError, Value};

#[test]
fn type_table_exercises_primitive_designators_and_fallbacks() {
    let big = Value::big_integer(ibig::IBig::from(i128::MAX));
    let rational = match Value::rational(1, 2) {
        Ok(value) => value,
        Err(error) => panic!("valid rational: {error}"),
    };
    let values = [
        (
            Value::Nil,
            vec!["T", "NIL", "BOOLEAN", "SYMBOL", "LIST", "ATOM"],
        ),
        (
            Value::Integer(1),
            vec!["NUMBER", "REAL", "RATIONAL", "INTEGER", "FIXNUM", "BIT"],
        ),
        (big, vec!["NUMBER", "RATIONAL", "INTEGER", "BIGNUM"]),
        (rational, vec!["NUMBER", "REAL", "RATIONAL", "RATIO"]),
        (Value::Float(1.0), vec!["NUMBER", "REAL", "FLOAT"]),
        (
            Value::Character('x'),
            vec!["CHARACTER", "BASE-CHAR", "STANDARD-CHAR", "EXTENDED-CHAR"],
        ),
        (
            Value::String("x".into()),
            vec![
                "STRING",
                "BASE-STRING",
                "SIMPLE-STRING",
                "SIMPLE-BASE-STRING",
                "SEQUENCE",
            ],
        ),
        (
            Value::vector(Vec::new()),
            vec!["VECTOR", "SIMPLE-VECTOR", "SEQUENCE"],
        ),
        (
            Value::array(vec![0], Vec::new()),
            vec!["ARRAY", "SIMPLE-ARRAY"],
        ),
        (Value::hash_table("equal"), vec!["HASH-TABLE"]),
        (Value::package("P"), vec!["PACKAGE"]),
        (Value::environment(Environment::new()), vec!["ENVIRONMENT"]),
        (
            Value::random_state(RandomState::seeded()),
            vec!["RANDOM-STATE"],
        ),
        (Value::string_input_stream("x", 0, 1), vec!["STREAM"]),
        (
            Value::closure(Vec::new(), Vec::new(), Environment::new()),
            vec!["FUNCTION", "COMPILED-FUNCTION"],
        ),
        (Value::Unbound, vec!["UNBOUND"]),
        (Value::values(vec![Value::Nil]), vec!["VALUES"]),
        (Value::restart("r"), vec!["RESTART"]),
    ];
    for (value, names) in values {
        for name in names {
            assert_eq!(type_matches(&value, name), Ok(true), "{name} for {value:?}");
        }
    }
    assert_eq!(type_matches(&Value::Nil, "CONDITION"), Ok(false));
    assert_eq!(type_matches(&Value::Nil, "ERROR"), Ok(false));
    assert_eq!(type_matches(&Value::Nil, "STRUCTURE"), Ok(false));
    assert_eq!(type_matches(&Value::Nil, "STANDARD-OBJECT"), Ok(false));
    assert_eq!(
        type_matches(&Value::Nil, "NOT-A-TYPE"),
        Err(RuntimeError::InvalidForm {
            message: "unknown type designator NOT-A-TYPE".into(),
            span: None,
        })
    );
}
