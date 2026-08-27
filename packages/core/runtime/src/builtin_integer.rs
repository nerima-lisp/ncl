use super::{arity, array_option_name, index_argument, integer_argument, type_error};
use crate::{RuntimeError, Value};

pub(super) fn parse_integer(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

#[derive(Debug, Copy, Clone)]
struct ParseIntegerOptions {
    start: usize,
    end: usize,
    radix: u32,
    junk_allowed: bool,
}

impl ParseIntegerOptions {
    fn from_arguments(arguments: &[Value], character_count: usize) -> Result<Self, RuntimeError> {
        let mut options = Self {
            start: 0,
            end: character_count,
            radix: 10,
            junk_allowed: false,
        };
        for pair in arguments.as_chunks::<2>().0 {
            match array_option_name("parse-integer", &pair[0])?.as_str() {
                "START" => options.start = index_argument("parse-integer", &pair[1])?,
                "END" => options.end = index_argument("parse-integer", &pair[1])?,
                "RADIX" => {
                    let radix = integer_argument("parse-integer", &pair[1])?;
                    options.radix = u32::try_from(radix).map_err(|_| invalid_radix(radix))?;
                }
                "JUNK-ALLOWED" => options.junk_allowed = pair[1].is_truthy(),
                option => {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("parse-integer does not accept :{option}"),
                        span: None,
                    });
                }
            }
        }
        if options.start > options.end || options.end > character_count {
            return Err(RuntimeError::InvalidForm {
                message: "parse-integer bounds are invalid".to_string(),
                span: None,
            });
        }
        if !(2..=36).contains(&options.radix) {
            return Err(invalid_radix(i64::from(options.radix)));
        }
        Ok(options)
    }
}

fn invalid_radix(radix: i64) -> RuntimeError {
    RuntimeError::InvalidForm {
        message: format!("parse-integer radix must be between 2 and 36, got {radix}"),
        span: None,
    }
}

fn parse_integer_value(
    chars: &[char],
    options: ParseIntegerOptions,
) -> Result<Value, RuntimeError> {
    let ParseIntegerOptions {
        start,
        end,
        radix,
        junk_allowed,
    } = options;
    let mut cursor = start;
    while cursor < end && chars[cursor].is_whitespace() {
        cursor += 1;
    }
    let negative = match chars.get(cursor) {
        Some('+') => {
            cursor += 1;
            false
        }
        Some('-') => {
            cursor += 1;
            true
        }
        _ => false,
    };
    let digits_start = cursor;
    let mut magnitude = 0_i128;
    while cursor < end {
        let Some(digit) = parse_integer_digit(chars[cursor]) else {
            break;
        };
        if digit >= radix {
            break;
        }
        magnitude = magnitude
            .checked_mul(i128::from(radix))
            .and_then(|value| value.checked_add(i128::from(digit)))
            .ok_or(RuntimeError::NumericOverflow)?;
        cursor += 1;
    }
    if cursor == digits_start {
        if junk_allowed {
            return parse_integer_result(None, cursor);
        }
        return Err(RuntimeError::InvalidForm {
            message: "parse-integer found no integer".to_string(),
            span: None,
        });
    }
    let signed = if negative {
        magnitude
            .checked_neg()
            .ok_or(RuntimeError::NumericOverflow)?
    } else {
        magnitude
    };
    let integer = i64::try_from(signed).map_err(|_| RuntimeError::NumericOverflow)?;
    if junk_allowed {
        return parse_integer_result(Some(integer), cursor);
    }
    let mut trailing = cursor;
    while trailing < end && chars[trailing].is_whitespace() {
        trailing += 1;
    }
    if trailing != end {
        return Err(RuntimeError::InvalidForm {
            message: "parse-integer found junk after the integer".to_string(),
            span: None,
        });
    }
    parse_integer_result(Some(integer), end)
}

fn parse_integer_result(integer: Option<i64>, position: usize) -> Result<Value, RuntimeError> {
    let position = i64::try_from(position).map_err(|_| RuntimeError::NumericOverflow)?;
    Ok(Value::values(vec![
        integer.map_or(Value::Nil, Value::Integer),
        Value::Integer(position),
    ]))
}

fn parse_integer_digit(character: char) -> Option<u32> {
    match character {
        '0'..='9' => Some(u32::from(character as u8 - b'0')),
        'A'..='Z' => Some(u32::from(character as u8 - b'A') + 10),
        'a'..='z' => Some(u32::from(character as u8 - b'a') + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_integer, parse_integer_digit};
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
