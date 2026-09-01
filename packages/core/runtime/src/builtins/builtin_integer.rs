use super::{arity, type_error};
use crate::{RuntimeError, Value};

mod options;
mod parser;

use options::ParseIntegerOptions;
use parser::parse_integer_value;

pub(crate) fn parse_integer(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() || !(arguments.len() - 1).is_multiple_of(2) {
        return Err(arity(
            "parse-integer",
            "a string and keyword/value pairs",
            arguments.len(),
        ));
    }
    let chars = match &arguments[0] {
        Value::String(value) => value.as_ref().chars().collect::<Vec<_>>(),
        value => return Err(type_error("parse-integer", "a string", value)),
    };
    let options = ParseIntegerOptions::from_arguments(&arguments[1..], chars.len())?;
    parse_integer_value(&chars, options)
}

#[cfg(test)]
mod tests {
    use super::{parse_integer, parser::parse_integer_digit};
    use crate::Value;

    fn keyword(name: &str) -> Value {
        Value::keyword(name)
    }

    fn parse(arguments: &[Value]) -> String {
        parse_integer(arguments)
            .unwrap_or_else(|error| panic!("integer should parse: {error}"))
            .to_string()
    }

    #[test]
    fn parses_integer_table_cases() {
        let cases = [
            (vec![Value::string("+42")], "#<VALUES 42 3>"),
            (vec![Value::string("  -42  ")], "#<VALUES -42 7>"),
            (
                vec![
                    Value::string("ff!"),
                    keyword("radix"),
                    Value::Integer(16),
                    keyword("junk-allowed"),
                    Value::Boolean(true),
                ],
                "#<VALUES 255 2>",
            ),
            (
                vec![
                    Value::string("xx42"),
                    keyword("start"),
                    Value::Integer(2),
                    keyword("end"),
                    Value::Integer(4),
                    keyword("junk-allowed"),
                    Value::Boolean(true),
                ],
                "#<VALUES 42 4>",
            ),
            (
                vec![Value::string("101"), keyword("radix"), Value::Integer(2)],
                "#<VALUES 5 3>",
            ),
            (
                vec![Value::string("ZZ"), keyword("radix"), Value::Integer(36)],
                "#<VALUES 1295 2>",
            ),
        ];

        for (arguments, expected) in cases {
            assert_eq!(parse(&arguments), expected);
        }
    }

    #[test]
    fn rejects_invalid_integer_table_cases() {
        let cases = [
            vec![],
            vec![Value::Integer(1)],
            vec![Value::string("1"), keyword("radix"), Value::Integer(1)],
            vec![Value::string("1"), keyword("radix"), Value::Integer(37)],
            vec![Value::string("1"), keyword("radix"), Value::Integer(-1)],
            vec![Value::string("1"), keyword("start"), Value::Integer(2)],
            vec![Value::string("1"), keyword("unknown"), Value::Boolean(true)],
            vec![Value::string("x")],
            vec![Value::string("12x")],
        ];

        for arguments in cases {
            assert!(
                parse_integer(&arguments).is_err(),
                "arguments: {arguments:?}"
            );
        }
    }

    #[test]
    fn parses_integer_digits_case_insensitively_and_rejects_punctuation() {
        let cases = [
            ('0', Some(0)),
            ('9', Some(9)),
            ('A', Some(10)),
            ('Z', Some(35)),
            ('a', Some(10)),
            ('z', Some(35)),
            ('!', None),
            (' ', None),
        ];

        for (character, expected) in cases {
            assert_eq!(parse_integer_digit(character), expected, "{character:?}");
        }
    }
}
