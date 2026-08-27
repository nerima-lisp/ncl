#![allow(clippy::wildcard_imports)]

use super::*;

pub(super) fn parse_list_membership_options(
    options: &[Value],
    is_predicate: bool,
    span: Span,
) -> Result<ListMembershipOptions, RuntimeError> {
    if !options.len().is_multiple_of(2) {
        return Err(Runtime::invalid(
            "list membership keyword arguments must be supplied in pairs",
            span,
        ));
    }
    let mut parsed = ListMembershipOptions {
        test: None,
        test_not: None,
        key: None,
    };
    for pair in options.as_chunks::<2>().0 {
        let keyword_name = match &pair[0] {
            Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
            _ => {
                return Err(Runtime::invalid(
                    "list membership keyword argument name must be a keyword",
                    span,
                ));
            }
        };
        match keyword_name.as_str() {
            "KEY" => parsed.key = Some(pair[1].clone()),
            "TEST" if !is_predicate => {
                if parsed.test_not.is_some() {
                    return Err(Runtime::invalid(
                        "list membership cannot use both :test and :test-not",
                        span,
                    ));
                }
                parsed.test = Some(pair[1].clone());
            }
            "TEST-NOT" if !is_predicate => {
                if parsed.test.is_some() {
                    return Err(Runtime::invalid(
                        "list membership cannot use both :test and :test-not",
                        span,
                    ));
                }
                parsed.test_not = Some(pair[1].clone());
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("unknown list membership keyword :{keyword_name}"),
                    span: Some(span),
                });
            }
        }
    }
    Ok(parsed)
}

pub(super) fn parse_association_search_options(
    options: &[Value],
    is_predicate: bool,
    span: Span,
) -> Result<AssociationSearchOptions, RuntimeError> {
    if !options.len().is_multiple_of(2) {
        return Err(Runtime::invalid(
            "association search keyword arguments must be supplied in pairs",
            span,
        ));
    }
    let mut parsed = AssociationSearchOptions {
        test: None,
        test_not: None,
        key: None,
    };
    for pair in options.as_chunks::<2>().0 {
        let keyword_name = match &pair[0] {
            Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
            _ => {
                return Err(Runtime::invalid(
                    "association search keyword argument name must be a keyword",
                    span,
                ));
            }
        };
        match keyword_name.as_str() {
            "KEY" => parsed.key = Some(pair[1].clone()),
            "TEST" if !is_predicate => {
                if parsed.test_not.is_some() {
                    return Err(Runtime::invalid(
                        "association search cannot use both :test and :test-not",
                        span,
                    ));
                }
                parsed.test = Some(pair[1].clone());
            }
            "TEST-NOT" if !is_predicate => {
                if parsed.test.is_some() {
                    return Err(Runtime::invalid(
                        "association search cannot use both :test and :test-not",
                        span,
                    ));
                }
                parsed.test_not = Some(pair[1].clone());
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("unknown association search keyword :{keyword_name}"),
                    span: Some(span),
                });
            }
        }
    }
    Ok(parsed)
}

pub(super) fn parse_sequence_substitute_options(
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

pub(super) fn sequence_substitute_optional_index(
    option: &str,
    value: &Value,
    span: Span,
) -> Result<Option<usize>, RuntimeError> {
    match value {
        Value::Nil => Ok(None),
        value => sequence_substitute_index(option, value, span).map(Some),
    }
}

pub(super) fn sequence_substitute_index(
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

pub(super) fn reduce_initial_value(
    initial_value: Option<Value>,
    first_value: Option<&Value>,
    apply_key: &dyn Fn(&Value) -> Result<Value, RuntimeError>,
    span: Span,
) -> Result<Value, RuntimeError> {
    initial_value.map_or_else(
        || {
            first_value.map_or_else(
                || Err(Runtime::invalid("reduce of an empty sequence", span)),
                apply_key,
            )
        },
        Ok,
    )
}

pub(super) fn parse_sequence_pair_search_options(
    options: &[Value],
    span: Span,
) -> Result<SequencePairSearchOptions, RuntimeError> {
    if !options.len().is_multiple_of(2) {
        return Err(Runtime::invalid(
            "sequence pair search keyword arguments must be supplied in pairs",
            span,
        ));
    }
    let mut parsed = SequencePairSearchOptions {
        from_end: false,
        test: None,
        test_not: None,
        key: None,
        start1: 0,
        end1: None,
        start2: 0,
        end2: None,
    };
    for pair in options.as_chunks::<2>().0 {
        let keyword_name = match &pair[0] {
            Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
            _ => {
                return Err(Runtime::invalid(
                    "sequence pair search keyword argument name must be a keyword",
                    span,
                ));
            }
        };
        match keyword_name.as_str() {
            "FROM-END" => parsed.from_end = pair[1].is_truthy(),
            "TEST" => {
                if parsed.test_not.is_some() {
                    return Err(Runtime::invalid(
                        "sequence pair search cannot use both :test and :test-not",
                        span,
                    ));
                }
                parsed.test = Some(pair[1].clone());
            }
            "TEST-NOT" => {
                if parsed.test.is_some() {
                    return Err(Runtime::invalid(
                        "sequence pair search cannot use both :test and :test-not",
                        span,
                    ));
                }
                parsed.test_not = Some(pair[1].clone());
            }
            "KEY" => parsed.key = Some(pair[1].clone()),
            "START1" => parsed.start1 = parse_sequence_index(":start1", &pair[1], span)?,
            "END1" => {
                parsed.end1 = match &pair[1] {
                    Value::Nil => None,
                    value => Some(parse_sequence_index(":end1", value, span)?),
                };
            }
            "START2" => parsed.start2 = parse_sequence_index(":start2", &pair[1], span)?,
            "END2" => {
                parsed.end2 = match &pair[1] {
                    Value::Nil => None,
                    value => Some(parse_sequence_index(":end2", value, span)?),
                };
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("unknown sequence pair search keyword :{keyword_name}"),
                    span: Some(span),
                });
            }
        }
    }
    Ok(parsed)
}

pub(super) fn parse_sequence_search_options(
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

pub(super) fn parse_sequence_reduce_options(
    options: &[Value],
    span: Span,
) -> Result<SequenceReduceOptions, RuntimeError> {
    if !options.len().is_multiple_of(2) {
        return Err(Runtime::invalid(
            "reduce keyword arguments must be supplied in pairs",
            span,
        ));
    }
    let mut parsed = SequenceReduceOptions {
        from_end: false,
        start: 0,
        end: None,
        initial_value: None,
        key: None,
    };
    for pair in options.as_chunks::<2>().0 {
        let keyword_name = match &pair[0] {
            Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
            _ => {
                return Err(Runtime::invalid(
                    "reduce keyword argument name must be a keyword",
                    span,
                ));
            }
        };
        match keyword_name.as_str() {
            "FROM-END" => parsed.from_end = pair[1].is_truthy(),
            "START" => parsed.start = parse_sequence_index(":start", &pair[1], span)?,
            "END" => parsed.end = Some(parse_sequence_index(":end", &pair[1], span)?),
            "INITIAL-VALUE" => parsed.initial_value = Some(pair[1].clone()),
            "KEY" => parsed.key = Some(pair[1].clone()),
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("unknown reduce keyword :{keyword_name}"),
                    span: Some(span),
                });
            }
        }
    }
    Ok(parsed)
}

pub(super) fn parse_sequence_index(
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

pub(super) fn parse_sequence_sort_key(
    options: &[Value],
    span: Span,
) -> Result<Option<Value>, RuntimeError> {
    if !options.len().is_multiple_of(2) {
        return Err(Runtime::invalid(
            "sequence sort keyword arguments must be supplied in pairs",
            span,
        ));
    }
    let mut key = None;
    for pair in options.as_chunks::<2>().0 {
        let keyword_name = match &pair[0] {
            Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
            _ => {
                return Err(Runtime::invalid(
                    "sequence sort keyword argument name must be a keyword",
                    span,
                ));
            }
        };
        match keyword_name.as_str() {
            "KEY" => key = Some(pair[1].clone()),
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("unknown sequence sort keyword :{keyword_name}"),
                    span: Some(span),
                });
            }
        }
    }
    Ok(key)
}

pub(super) fn sequence_sort_result(
    kind: SequenceKind,
    sorted: Vec<Value>,
    span: Span,
) -> Result<Value, RuntimeError> {
    match kind {
        SequenceKind::List => Ok(Value::list(sorted)),
        SequenceKind::Vector => Ok(Value::vector(sorted)),
        SequenceKind::String => {
            let mut value = String::new();
            for item in sorted {
                let Value::Character(character) = item else {
                    return Err(RuntimeError::Type {
                        expected: "CHARACTER".to_string(),
                        actual: item.type_name().to_string(),
                        span: Some(span),
                    });
                };
                value.push(character);
            }
            Ok(Value::string(value))
        }
    }
}

pub(super) fn parse_sequence_merge_key(
    options: &[Value],
    span: Span,
) -> Result<Option<Value>, RuntimeError> {
    if !options.len().is_multiple_of(2) {
        return Err(Runtime::invalid(
            "merge keyword arguments must be supplied in pairs",
            span,
        ));
    }
    let mut key = None;
    for pair in options.as_chunks::<2>().0 {
        let keyword_name = match &pair[0] {
            Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
            _ => {
                return Err(Runtime::invalid(
                    "merge keyword argument name must be a keyword",
                    span,
                ));
            }
        };
        if keyword_name != "KEY" {
            return Err(RuntimeError::InvalidForm {
                message: format!("unknown merge keyword :{keyword_name}"),
                span: Some(span),
            });
        }
        key = Some(pair[1].clone());
    }
    Ok(key)
}

pub(super) fn sequence_items(value: &Value, span: Span) -> Result<Vec<Value>, RuntimeError> {
    match value {
        Value::Nil => Ok(Vec::new()),
        Value::List(items) | Value::Vector(items) => Ok(items.as_ref().clone()),
        Value::String(value) => Ok(value.chars().map(Value::Character).collect()),
        value => Err(RuntimeError::Type {
            expected: "SEQUENCE".to_string(),
            actual: value.type_name().to_string(),
            span: Some(span),
        }),
    }
}

pub(super) fn merge_result_kind(
    result_type: &Value,
    span: Span,
) -> Result<&'static str, RuntimeError> {
    match result_type.symbol_name().map(normalize_name).as_deref() {
        Some("NIL") => Ok("NIL"),
        Some("LIST") => Ok("LIST"),
        Some("VECTOR" | "SIMPLE-VECTOR") => Ok("VECTOR"),
        Some("STRING" | "SIMPLE-STRING") => Ok("STRING"),
        _ => Err(Runtime::invalid(
            "merge result type must be LIST, VECTOR, STRING, or NIL",
            span,
        )),
    }
}

pub(super) fn sequence_merge_result(
    result_kind: &str,
    merged: Vec<Value>,
    span: Span,
) -> Result<Value, RuntimeError> {
    match result_kind {
        "NIL" => Ok(Value::Nil),
        "LIST" => Ok(Value::list(merged)),
        "VECTOR" => Ok(Value::vector(merged)),
        "STRING" => {
            let mut value = String::new();
            for item in merged {
                let Value::Character(character) = item else {
                    return Err(RuntimeError::Type {
                        expected: "CHARACTER".to_string(),
                        actual: item.type_name().to_string(),
                        span: Some(span),
                    });
                };
                value.push(character);
            }
            Ok(Value::string(value))
        }
        _ => unreachable!("validated MERGE result type"),
    }
}

pub(super) fn sequence_removal_options(
    options: &SequenceRemoveOptions,
    end: usize,
) -> SequenceRemoveOptions {
    SequenceRemoveOptions {
        end: Some(end),
        ..options.clone()
    }
}

pub(super) fn parse_list_set_options(
    options: &[Value],
    span: Span,
) -> Result<ListSetOptions, RuntimeError> {
    if !options.len().is_multiple_of(2) {
        return Err(Runtime::invalid(
            "list set operation keyword arguments must be supplied in pairs",
            span,
        ));
    }
    let mut parsed = ListSetOptions {
        test: None,
        test_not: None,
        key: None,
    };
    for pair in options.as_chunks::<2>().0 {
        let keyword_name = match &pair[0] {
            Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
            _ => {
                return Err(Runtime::invalid(
                    "list set operation keyword argument name must be a keyword",
                    span,
                ));
            }
        };
        match keyword_name.as_str() {
            "TEST" => {
                if parsed.test_not.is_some() {
                    return Err(Runtime::invalid(
                        "list set operation cannot use both :test and :test-not",
                        span,
                    ));
                }
                parsed.test = Some(pair[1].clone());
            }
            "TEST-NOT" => {
                if parsed.test.is_some() {
                    return Err(Runtime::invalid(
                        "list set operation cannot use both :test and :test-not",
                        span,
                    ));
                }
                parsed.test_not = Some(pair[1].clone());
            }
            "KEY" => parsed.key = Some(pair[1].clone()),
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("unknown list set operation keyword :{keyword_name}"),
                    span: Some(span),
                });
            }
        }
    }
    Ok(parsed)
}

pub(super) fn parse_sequence_remove_options(
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

#[cfg(test)]
mod tests {
    use super::*;

    const SPAN: Span = Span::new(0, 1);

    #[test]
    fn sequence_option_parsers_accept_supported_keyword_shapes() {
        type Parser = fn(&[Value], Span) -> Result<(), RuntimeError>;
        let cases: &[(Parser, &[Value])] = &[
            (
                |options, span| parse_sequence_search_options(options, span).map(|_| ()),
                &[Value::keyword("from-end"), Value::boolean(true)],
            ),
            (
                |options, span| parse_sequence_reduce_options(options, span).map(|_| ()),
                &[
                    Value::keyword("start"),
                    Value::Integer(1),
                    Value::keyword("end"),
                    Value::Integer(2),
                ],
            ),
            (
                |options, span| parse_sequence_sort_key(options, span).map(|_| ()),
                &[Value::keyword("key"), Value::Nil],
            ),
            (
                |options, span| parse_sequence_merge_key(options, span).map(|_| ()),
                &[Value::keyword("key"), Value::Nil],
            ),
        ];

        for (parser, options) in cases {
            assert!(parser(options, SPAN).is_ok());
        }
    }

    #[test]
    fn sequence_option_parsers_reject_malformed_and_conflicting_options() {
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
        assert!(parse_sequence_reduce_options(cases[0], SPAN).is_err());
        assert!(parse_sequence_sort_key(cases[4], SPAN).is_err());
    }

    #[test]
    fn sequence_index_and_reduce_helpers_cover_empty_and_invalid_inputs() {
        assert_eq!(
            parse_sequence_index(":start", &Value::Integer(3), SPAN),
            Ok(3)
        );
        assert!(parse_sequence_index(":start", &Value::Integer(-1), SPAN).is_err());
        assert!(parse_sequence_index(":start", &Value::Nil, SPAN).is_err());
        assert_eq!(
            sequence_substitute_optional_index(":end", &Value::Nil, SPAN),
            Ok(None)
        );
        assert!(reduce_initial_value(None, None, &|value| Ok(value.clone()), SPAN).is_err());
        assert_eq!(
            reduce_initial_value(
                Some(Value::Integer(4)),
                None,
                &|value| Ok(value.clone()),
                SPAN
            )
            .map(|value| value.to_string()),
            Ok("4".to_string())
        );
    }

    #[test]
    fn sequence_option_parsers_cover_operation_specific_keywords() {
        let cases: &[(&str, Result<(), RuntimeError>)] = &[
            (
                "membership",
                parse_list_membership_options(
                    &[
                        Value::keyword("key"),
                        Value::Nil,
                        Value::keyword("test"),
                        Value::Nil,
                    ],
                    false,
                    SPAN,
                )
                .map(|_| ()),
            ),
            (
                "association",
                parse_association_search_options(
                    &[Value::keyword("test-not"), Value::Nil],
                    false,
                    SPAN,
                )
                .map(|_| ()),
            ),
            (
                "substitute",
                parse_sequence_substitute_options(
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
                )
                .map(|_| ()),
            ),
            (
                "pair search",
                parse_sequence_pair_search_options(
                    &[
                        Value::keyword("start1"),
                        Value::Integer(0),
                        Value::keyword("end1"),
                        Value::Nil,
                        Value::keyword("start2"),
                        Value::Integer(1),
                        Value::keyword("end2"),
                        Value::Integer(2),
                    ],
                    SPAN,
                )
                .map(|_| ()),
            ),
            (
                "list set",
                parse_list_set_options(
                    &[Value::keyword("test-not"), Value::Nil, Value::keyword("key"), Value::Nil],
                    SPAN,
                )
                .map(|_| ()),
            ),
            (
                "removal",
                parse_sequence_remove_options(
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
                )
                .map(|_| ()),
            ),
        ];

        for (name, result) in cases {
            assert!(result.is_ok(), "{name}: {result:?}");
        }
    }

    #[test]
    fn sequence_result_helpers_cover_supported_kinds_and_type_errors() {
        let characters = vec![Value::Character('a'), Value::Character('b')];
        let cases = [
            ("NIL", sequence_merge_result("NIL", characters.clone(), SPAN)),
            ("LIST", sequence_merge_result("LIST", characters.clone(), SPAN)),
            (
                "VECTOR",
                sequence_merge_result("VECTOR", characters.clone(), SPAN),
            ),
            ("STRING", sequence_merge_result("STRING", characters, SPAN)),
        ];
        for (kind, result) in cases {
            assert!(result.is_ok(), "{kind}: {result:?}");
        }
        assert!(sequence_merge_result("STRING", vec![Value::Nil], SPAN).is_err());
        assert_eq!(merge_result_kind(&Value::symbol("simple-vector"), SPAN), Ok("VECTOR"));
        assert!(merge_result_kind(&Value::Integer(1), SPAN).is_err());
        assert!(sequence_items(&Value::Nil, SPAN).is_ok_and(|items| items.is_empty()));
        assert!(sequence_items(&Value::Integer(1), SPAN).is_err());
    }
}
