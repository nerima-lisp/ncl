use std::rc::Rc;

use ncl_runtime::Value;

#[test]
fn scalar_values_have_stable_human_readable_forms() {
    let cases = [
        (Value::Nil, "NIL"),
        (Value::Unbound, "#<UNBOUND>"),
        (Value::Boolean(true), "T"),
        (Value::Boolean(false), "NIL"),
        (Value::Integer(42), "42"),
        (Value::Float(2.0), "2.0"),
        (Value::Float(2.5), "2.5"),
        (Value::String(Rc::from("hello")), "\"hello\""),
        (Value::Character(' '), "#\\SPACE"),
        (Value::Character('\n'), "#\\NEWLINE"),
        (Value::Character('\t'), "#\\TAB"),
        (Value::Character('\r'), "#\\RETURN"),
        (Value::Character('x'), "#\\x"),
        (Value::Symbol(Rc::from("name")), "name"),
        (Value::SymbolExact(Rc::from("a|b\\c")), "|a\\|b\\\\c|"),
        (Value::UninternedSymbol(Rc::from("gensym")), "#:gensym"),
        (Value::Keyword(Rc::from("key")), ":key"),
        (Value::KeywordExact(Rc::from("a|b")), ":|a\\|b|"),
    ];

    for (value, expected) in cases {
        assert_eq!(value.to_string(), expected, "displaying {value:?}");
    }
}

#[test]
fn compound_values_have_stable_readable_forms() {
    let cases = [
        (
            Value::List(Rc::new(vec![Value::Integer(1), Value::Boolean(true)])),
            "(1 T)",
        ),
        (
            Value::DottedList {
                items: Rc::new(vec![Value::Integer(1)]),
                tail: Rc::new(Value::Symbol(Rc::from("tail"))),
            },
            "(1 . tail)",
        ),
        (
            Value::Vector(Rc::new(vec![Value::Integer(1), Value::Integer(2)])),
            "#(1 2)",
        ),
        (Value::Values(Rc::new(Vec::new())), "#<VALUES>"),
        (
            Value::Values(Rc::new(vec![Value::Integer(1), Value::Integer(2)])),
            "#<VALUES 1 2>",
        ),
    ];

    for (value, expected) in cases {
        assert_eq!(value.to_string(), expected, "displaying {value:?}");
    }
}
