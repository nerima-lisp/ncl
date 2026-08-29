#[cfg(test)]
mod tests {
    use crate::error::RuntimeError;

    use super::super::{Environment, Value};

    #[test]
    fn values_have_stable_display_and_debug_forms() {
        #[allow(clippy::unnecessary_wraps)]
        fn no_op(_: &[Value]) -> Result<Value, RuntimeError> {
            Ok(Value::Nil)
        }

        let rational = match Value::rational(3, 2) {
            Ok(value) => value,
            Err(error) => panic!("expected valid rational, got {error:?}"),
        };
        let cases = [
            (Value::Nil, "NIL"),
            (Value::Unbound, "#<UNBOUND>"),
            (Value::Boolean(true), "T"),
            (Value::Boolean(false), "NIL"),
            (Value::Integer(7), "7"),
            (rational, "3/2"),
            (Value::Float(2.0), "2.0"),
            (Value::Float(2.5), "2.5"),
            (Value::string("line\n"), "\"line\\n\""),
            (Value::Character(' '), "#\\SPACE"),
            (Value::Character('\n'), "#\\NEWLINE"),
            (Value::Character('\t'), "#\\TAB"),
            (Value::Character('\r'), "#\\RETURN"),
            (Value::Character('x'), "#\\x"),
            (Value::package("USER"), "#<PACKAGE \"USER\">"),
            (Value::symbol("name"), "NAME"),
            (Value::symbol_exact("a|b\\c"), "|a\\|b\\\\c|"),
            (Value::uninterned_symbol("x"), "#:x"),
            (Value::keyword("key"), ":KEY"),
            (Value::keyword_exact("a|b"), ":|a\\|b|"),
            (
                Value::list(vec![Value::Integer(1), Value::Integer(2)]),
                "(1 2)",
            ),
            (Value::list(Vec::new()), "NIL"),
            (
                Value::dotted_list(vec![Value::Integer(1)], Value::Integer(2)),
                "(1 . 2)",
            ),
            (Value::dotted_list(Vec::new(), Value::Integer(2)), "(. 2)"),
            (Value::vector(vec![Value::Integer(1)]), "#(1)"),
            (
                Value::array(vec![2], vec![Value::Nil, Value::Nil]),
                "#<ARRAY [2]>",
            ),
            (Value::hash_table("EQ"), "#<HASH-TABLE EQ>"),
            (Value::values(vec![Value::Integer(1)]), "#<VALUES 1>"),
            (Value::values(Vec::new()), "#<VALUES>"),
            (Value::restart("retry"), "#<RESTART retry>"),
            (Value::builtin("NO-OP", no_op), "#<BUILTIN NO-OP>"),
            (Value::primitive("PRIMITIVE"), "#<PRIMITIVE PRIMITIVE>"),
            (Value::generic("combine"), "#<GENERIC-FUNCTION combine>"),
            (
                Value::slot_reader("person", "name"),
                "#<SLOT-READER person-name>",
            ),
            (
                Value::slot_writer("person", "name"),
                "#<SLOT-WRITER person-name>",
            ),
            (
                Value::closure(Vec::new(), Vec::new(), Environment::new()),
                "#<FUNCTION>",
            ),
            (
                Value::structure_with_types("point", Vec::new(), Vec::new()),
                "#S(point)",
            ),
        ];

        for (value, expected) in cases {
            assert_eq!(value.to_string(), expected);
            assert_eq!(format!("{value:?}"), format!("Value({expected})"));
        }
    }

    #[test]
    fn display_covers_exact_numeric_and_runtime_boundaries() {
        let cases = [
            (
                Value::rational(3, 2)
                    .unwrap_or_else(|error| panic!("rational should be valid: {error}")),
                "3/2",
            ),
            (Value::Character('\t'), "#\\TAB"),
            (Value::Character('\r'), "#\\RETURN"),
            (Value::keyword_exact("a|b"), ":|a\\|b|"),
            (Value::list(vec![Value::Integer(1)]), "(1)"),
            (
                Value::vector(vec![Value::Integer(1), Value::Integer(2)]),
                "#(1 2)",
            ),
            (Value::Environment(Environment::new()), "#<ENVIRONMENT>"),
            (
                Value::condition_from_parts(
                    "ERROR".to_owned(),
                    "failure".to_owned(),
                    None,
                    Vec::new(),
                ),
                "#<CONDITION failure>",
            ),
        ];

        for (value, expected) in cases {
            assert_eq!(value.to_string(), expected);
        }
    }
}
