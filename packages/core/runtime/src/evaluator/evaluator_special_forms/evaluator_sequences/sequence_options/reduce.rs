#![allow(clippy::wildcard_imports)]
use super::super::*;

use super::search::parse_sequence_index;

pub(crate) fn reduce_initial_value(
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

pub(crate) fn parse_sequence_reduce_options(
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

#[cfg(test)]
mod tests {
    use super::*;

    const SPAN: Span = Span::new(0, 1);

    #[test]
    fn parse_sequence_reduce_options_accepts_start_and_end() {
        let parsed = parse_sequence_reduce_options(
            &[
                Value::keyword("start"),
                Value::Integer(1),
                Value::keyword("end"),
                Value::Integer(2),
            ],
            SPAN,
        );
        assert!(parsed.is_ok());
    }

    #[test]
    fn parse_sequence_reduce_options_rejects_odd_argument_count() {
        assert!(parse_sequence_reduce_options(&[Value::keyword("start")], SPAN).is_err());
    }

    #[test]
    fn reduce_initial_value_covers_empty_provided_and_failing_key() {
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
        let first_value = Value::Integer(3);
        assert_eq!(
            reduce_initial_value(
                None,
                Some(&first_value),
                &|_| { Ok(Value::Integer(4)) },
                SPAN
            )
            .map(|value| value.to_string()),
            Ok("4".to_string())
        );
        assert!(
            reduce_initial_value(
                None,
                Some(&first_value),
                &|_| Err(Runtime::invalid("reduce key failed", SPAN)),
                SPAN
            )
            .is_err()
        );
    }
}
