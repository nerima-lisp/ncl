#![allow(clippy::wildcard_imports)]
use super::super::*;

pub(crate) fn parse_sequence_sort_key(
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

pub(crate) fn sequence_sort_result(
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

#[cfg(test)]
mod tests {
    use super::*;

    const SPAN: Span = Span::new(0, 1);

    #[test]
    fn parse_sequence_sort_key_accepts_nil_key() {
        assert!(parse_sequence_sort_key(&[Value::keyword("key"), Value::Nil], SPAN).is_ok());
    }

    #[test]
    fn parse_sequence_sort_key_rejects_unknown_keyword() {
        assert!(parse_sequence_sort_key(&[Value::keyword("unknown"), Value::Nil], SPAN).is_err());
    }
}
