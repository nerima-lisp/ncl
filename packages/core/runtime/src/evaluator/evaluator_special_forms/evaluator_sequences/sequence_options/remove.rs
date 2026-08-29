#![allow(clippy::wildcard_imports)]
use super::super::*;

pub(crate) fn parse_sequence_remove_options(
    options: &[Value],
    is_predicate: bool,
    removes_duplicates: bool,
    span: Span,
) -> Result<SequenceRemoveOptions, RuntimeError> {
    if !options.len().is_multiple_of(2) {
        return Err(Runtime::invalid(
            "sequence removal keyword arguments must be supplied in pairs",
            span,
        ));
    }
    let mut parsed = SequenceRemoveOptions {
        from_end: false,
        test: None,
        test_not: None,
        key: None,
        start: 0,
        end: None,
        count: None,
    };
    let index_argument = |option: &str, value: &Value| -> Result<usize, RuntimeError> {
        let Value::Integer(index) = value else {
            return Err(RuntimeError::Type {
                expected: "INTEGER".to_string(),
                actual: value.type_name().to_string(),
                span: Some(span),
            });
        };
        if *index < 0 {
            return Err(Runtime::invalid(
                &format!("sequence removal {option} must be non-negative"),
                span,
            ));
        }
        usize::try_from(*index).map_err(|_| {
            Runtime::invalid(&format!("sequence removal {option} is out of range"), span)
        })
    };

    for pair in options.as_chunks::<2>().0 {
        let keyword_name = match &pair[0] {
            Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
            _ => {
                return Err(Runtime::invalid(
                    "sequence removal keyword argument name must be a keyword",
                    span,
                ));
            }
        };
        match keyword_name.as_str() {
            "FROM-END" => parsed.from_end = pair[1].is_truthy(),
            "TEST" if !is_predicate => {
                if parsed.test_not.is_some() {
                    return Err(Runtime::invalid(
                        "sequence removal cannot use both :test and :test-not",
                        span,
                    ));
                }
                parsed.test = Some(pair[1].clone());
            }
            "TEST-NOT" if !is_predicate => {
                if parsed.test.is_some() {
                    return Err(Runtime::invalid(
                        "sequence removal cannot use both :test and :test-not",
                        span,
                    ));
                }
                parsed.test_not = Some(pair[1].clone());
            }
            "KEY" => parsed.key = Some(pair[1].clone()),
            "START" => parsed.start = index_argument(":start", &pair[1])?,
            "END" => {
                parsed.end = match &pair[1] {
                    Value::Nil => None,
                    value => Some(index_argument(":end", value)?),
                };
            }
            "COUNT" if !removes_duplicates => {
                parsed.count = match &pair[1] {
                    Value::Nil => None,
                    value => Some(index_argument(":count", value)?),
                };
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("unknown sequence removal keyword :{keyword_name}"),
                    span: Some(span),
                });
            }
        }
    }
    Ok(parsed)
}

pub(crate) fn sequence_removal_options(
    options: &SequenceRemoveOptions,
    end: usize,
) -> SequenceRemoveOptions {
    SequenceRemoveOptions {
        end: Some(end),
        ..options.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPAN: Span = Span::new(0, 1);

    #[test]
    fn parse_sequence_remove_options_accepts_bounds_and_count() {
        let parsed = parse_sequence_remove_options(
            &[
                Value::keyword("from-end"),
                Value::boolean(true),
                Value::keyword("end"),
                Value::Nil,
                Value::keyword("count"),
                Value::Nil,
            ],
            false,
            false,
            SPAN,
        );
        assert!(parsed.is_ok());
    }

    #[test]
    fn sequence_removal_options_overrides_end_and_preserves_other_fields() {
        let options = SequenceRemoveOptions {
            from_end: true,
            test: None,
            test_not: None,
            key: None,
            start: 2,
            end: None,
            count: None,
        };
        let narrowed = sequence_removal_options(&options, 5);
        assert!(narrowed.from_end);
        assert_eq!(narrowed.start, 2);
        assert_eq!(narrowed.end, Some(5));
    }
}
