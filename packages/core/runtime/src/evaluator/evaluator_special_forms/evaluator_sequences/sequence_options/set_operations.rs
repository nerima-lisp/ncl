#![allow(clippy::wildcard_imports)]
use super::super::*;

pub(crate) fn parse_list_set_options(
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

#[cfg(test)]
mod tests {
    use super::*;

    const SPAN: Span = Span::new(0, 1);

    #[test]
    fn parse_list_set_options_accepts_test_not_and_key() {
        let parsed = parse_list_set_options(
            &[
                Value::keyword("test-not"),
                Value::Nil,
                Value::keyword("key"),
                Value::Nil,
            ],
            SPAN,
        );
        assert!(parsed.is_ok());
    }
}
