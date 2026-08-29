#![allow(clippy::wildcard_imports)]
use super::super::*;

use super::search::parse_sequence_index;

pub fn parse_sequence_pair_search_options(
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

#[cfg(test)]
mod tests {
    use super::*;

    const SPAN: Span = Span::new(0, 1);

    #[test]
    fn parse_sequence_pair_search_options_accepts_bounds_on_both_sequences() {
        let parsed = parse_sequence_pair_search_options(
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
        );
        assert!(parsed.is_ok());
    }
}
