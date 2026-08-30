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
mod tests;
