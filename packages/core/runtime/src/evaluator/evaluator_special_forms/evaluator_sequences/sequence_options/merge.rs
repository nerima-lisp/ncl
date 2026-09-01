#![allow(clippy::wildcard_imports)]
use super::super::*;

pub fn parse_sequence_merge_key(
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

pub fn sequence_items(value: &Value, span: Span) -> Result<Vec<Value>, RuntimeError> {
    match value {
        Value::Nil => Ok(Vec::new()),
        Value::List(items) => Ok(items.as_ref().clone()),
        Value::Vector(items) => Ok(items.borrow().clone()),
        Value::String(value) => Ok(value.chars().map(Value::Character).collect()),
        value => Err(RuntimeError::Type {
            expected: "SEQUENCE".to_string(),
            actual: value.type_name().to_string(),
            span: Some(span),
        }),
    }
}

pub fn merge_result_kind(result_type: &Value, span: Span) -> Result<&'static str, RuntimeError> {
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

pub fn sequence_merge_result(
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

#[cfg(test)]
mod tests {
    use super::*;

    const SPAN: Span = Span::new(0, 1);

    #[test]
    fn parse_sequence_merge_key_accepts_nil_key() {
        assert!(parse_sequence_merge_key(&[Value::keyword("key"), Value::Nil], SPAN).is_ok());
    }

    #[test]
    fn sequence_result_helpers_cover_supported_kinds_and_type_errors() {
        let characters = vec![Value::Character('a'), Value::Character('b')];
        let cases = [
            (
                "NIL",
                sequence_merge_result("NIL", characters.clone(), SPAN),
            ),
            (
                "LIST",
                sequence_merge_result("LIST", characters.clone(), SPAN),
            ),
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
        assert_eq!(
            merge_result_kind(&Value::symbol("simple-vector"), SPAN),
            Ok("VECTOR")
        );
        assert!(merge_result_kind(&Value::Integer(1), SPAN).is_err());
        assert!(sequence_items(&Value::Nil, SPAN).is_ok_and(|items| items.is_empty()));
        assert!(sequence_items(&Value::Integer(1), SPAN).is_err());
    }
}
