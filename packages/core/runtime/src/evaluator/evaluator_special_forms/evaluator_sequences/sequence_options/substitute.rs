#![allow(clippy::wildcard_imports)]
use super::super::*;

pub fn parse_sequence_substitute_options(
    options: &[Value],
    is_predicate: bool,
    span: Span,
) -> Result<SequenceSubstituteOptions, RuntimeError> {
    if !options.len().is_multiple_of(2) {
        return Err(Runtime::invalid(
            "sequence substitution keyword arguments must be supplied in pairs",
            span,
        ));
    }
    let mut parsed = SequenceSubstituteOptions {
        from_end: false,
        test: None,
        test_not: None,
        key: None,
        start: 0,
        end: None,
        count: None,
    };
    for pair in options.as_chunks::<2>().0 {
        let keyword_name = match &pair[0] {
            Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
            _ => {
                return Err(Runtime::invalid(
                    "sequence substitution keyword argument name must be a keyword",
                    span,
                ));
            }
        };
        match keyword_name.as_str() {
            "FROM-END" => parsed.from_end = pair[1].is_truthy(),
            "TEST" if !is_predicate => {
                if parsed.test_not.is_some() {
                    return Err(Runtime::invalid(
                        "sequence substitution cannot use both :test and :test-not",
                        span,
                    ));
                }
                parsed.test = Some(pair[1].clone());
            }
            "TEST-NOT" if !is_predicate => {
                if parsed.test.is_some() {
                    return Err(Runtime::invalid(
                        "sequence substitution cannot use both :test and :test-not",
                        span,
                    ));
                }
                parsed.test_not = Some(pair[1].clone());
            }
            "KEY" => parsed.key = Some(pair[1].clone()),
            "START" => parsed.start = sequence_substitute_index(":start", &pair[1], span)?,
            "END" => {
                parsed.end = sequence_substitute_optional_index(":end", &pair[1], span)?;
            }
            "COUNT" => {
                parsed.count = sequence_substitute_optional_index(":count", &pair[1], span)?;
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("unknown sequence substitution keyword :{keyword_name}"),
                    span: Some(span),
                });
            }
        }
    }
    Ok(parsed)
}

fn sequence_substitute_optional_index(
    option: &str,
    value: &Value,
    span: Span,
) -> Result<Option<usize>, RuntimeError> {
    match value {
        Value::Nil => Ok(None),
        value => sequence_substitute_index(option, value, span).map(Some),
    }
}

fn sequence_substitute_index(
    option: &str,
    value: &Value,
    span: Span,
) -> Result<usize, RuntimeError> {
    let Value::Integer(index) = value else {
        return Err(RuntimeError::Type {
            expected: "INTEGER".to_string(),
            actual: value.type_name().to_string(),
            span: Some(span),
        });
    };
    if *index < 0 {
        return Err(Runtime::invalid(
            &format!("sequence substitution {option} must be non-negative"),
            span,
        ));
    }
    usize::try_from(*index).map_err(|_| {
        Runtime::invalid(
            &format!("sequence substitution {option} is out of range"),
            span,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPAN: Span = Span::new(0, 1);

    #[test]
    fn parse_sequence_substitute_options_accepts_bounds_and_count() {
        let parsed = parse_sequence_substitute_options(
            &[
                Value::keyword("from-end"),
                Value::boolean(true),
                Value::keyword("start"),
                Value::Integer(1),
                Value::keyword("end"),
                Value::Nil,
                Value::keyword("count"),
                Value::Integer(2),
            ],
            false,
            SPAN,
        );
        assert!(parsed.is_ok());
    }

    #[test]
    fn sequence_substitute_optional_index_treats_nil_as_absent() {
        assert_eq!(
            sequence_substitute_optional_index(":end", &Value::Nil, SPAN),
            Ok(None)
        );
    }

    #[test]
    fn parse_sequence_substitute_options_rejects_odd_options() {
        let parsed = parse_sequence_substitute_options(&[Value::keyword("start")], false, SPAN);
        assert!(matches!(parsed, Err(RuntimeError::InvalidForm { .. })));
    }

    #[test]
    fn parse_sequence_substitute_options_rejects_conflicting_test_options() {
        let parsed = parse_sequence_substitute_options(
            &[
                Value::keyword("test"),
                Value::symbol("EQL"),
                Value::keyword("test-not"),
                Value::symbol("EQL"),
            ],
            false,
            SPAN,
        );
        assert!(matches!(parsed, Err(RuntimeError::InvalidForm { .. })));

        let parsed_reverse = parse_sequence_substitute_options(
            &[
                Value::keyword("test-not"),
                Value::symbol("EQL"),
                Value::keyword("test"),
                Value::symbol("EQL"),
            ],
            false,
            SPAN,
        );
        assert!(matches!(
            parsed_reverse,
            Err(RuntimeError::InvalidForm { .. })
        ));
    }

    #[test]
    fn parse_sequence_substitute_options_rejects_unknown_keyword() {
        let parsed =
            parse_sequence_substitute_options(&[Value::keyword("bogus"), Value::Nil], false, SPAN);
        assert!(matches!(parsed, Err(RuntimeError::InvalidForm { .. })));
    }

    #[test]
    fn parse_sequence_substitute_options_rejects_non_keyword_name() {
        let parsed =
            parse_sequence_substitute_options(&[Value::Integer(1), Value::Nil], false, SPAN);
        assert!(matches!(parsed, Err(RuntimeError::InvalidForm { .. })));
    }

    #[test]
    fn sequence_substitute_index_rejects_negative_and_non_integer() {
        assert!(matches!(
            sequence_substitute_index(":start", &Value::Integer(-1), SPAN),
            Err(RuntimeError::InvalidForm { .. })
        ));
        assert!(matches!(
            sequence_substitute_index(":start", &Value::symbol("X"), SPAN),
            Err(RuntimeError::Type { expected, .. }) if expected == "INTEGER"
        ));
    }
}
