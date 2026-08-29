#![allow(clippy::wildcard_imports)]
use super::super::*;

pub(crate) fn parse_sequence_search_options(
    options: &[Value],
    span: Span,
) -> Result<SequenceSearchOptions, RuntimeError> {
    if !options.len().is_multiple_of(2) {
        return Err(Runtime::invalid(
            "sequence search keyword arguments must be supplied in pairs",
            span,
        ));
    }
    let mut parsed = SequenceSearchOptions {
        from_end: false,
        test: None,
        test_not: None,
        key: None,
        start: 0,
        end: None,
    };
    for pair in options.as_chunks::<2>().0 {
        let keyword_name = match &pair[0] {
            Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
            _ => {
                return Err(Runtime::invalid(
                    "sequence search keyword argument name must be a keyword",
                    span,
                ));
            }
        };
        match keyword_name.as_str() {
            "FROM-END" => parsed.from_end = pair[1].is_truthy(),
            "TEST" => {
                if parsed.test_not.is_some() {
                    return Err(Runtime::invalid(
                        "sequence search cannot use both :test and :test-not",
                        span,
                    ));
                }
                parsed.test = Some(pair[1].clone());
            }
            "TEST-NOT" => {
                if parsed.test.is_some() {
                    return Err(Runtime::invalid(
                        "sequence search cannot use both :test and :test-not",
                        span,
                    ));
                }
                parsed.test_not = Some(pair[1].clone());
            }
            "KEY" => parsed.key = Some(pair[1].clone()),
            "START" => parsed.start = parse_sequence_index(":start", &pair[1], span)?,
            "END" => {
                parsed.end = match &pair[1] {
                    Value::Nil => None,
                    value => Some(parse_sequence_index(":end", value, span)?),
                };
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("unknown sequence search keyword :{keyword_name}"),
                    span: Some(span),
                });
            }
        }
    }
    Ok(parsed)
}

pub(crate) fn parse_sequence_index(
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
            &format!("reduce {option} must be non-negative"),
            span,
        ));
    }
    usize::try_from(*index)
        .map_err(|_| Runtime::invalid(&format!("reduce {option} is out of range"), span))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPAN: Span = Span::new(0, 1);

    #[test]
    fn parse_sequence_search_options_accepts_from_end() {
        let parsed = parse_sequence_search_options(
            &[Value::keyword("from-end"), Value::boolean(true)],
            SPAN,
        );
        assert!(parsed.is_ok());
    }

    #[test]
    fn parse_sequence_search_options_rejects_malformed_and_conflicting_options() {
        let cases: &[&[Value]] = &[
            &[Value::keyword("start")],
            &[Value::Integer(0), Value::Nil],
            &[
                Value::keyword("test"),
                Value::Nil,
                Value::keyword("test-not"),
                Value::Nil,
            ],
            &[Value::keyword("start"), Value::Integer(-1)],
            &[Value::keyword("unknown"), Value::Nil],
        ];
        for options in cases {
            assert!(parse_sequence_search_options(options, SPAN).is_err());
        }
    }

    #[test]
    fn parse_sequence_index_covers_valid_negative_and_non_integer_inputs() {
        assert_eq!(
            parse_sequence_index(":start", &Value::Integer(3), SPAN),
            Ok(3)
        );
        assert!(parse_sequence_index(":start", &Value::Integer(-1), SPAN).is_err());
        assert!(parse_sequence_index(":start", &Value::Nil, SPAN).is_err());
    }
}
