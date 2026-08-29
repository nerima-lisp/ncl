#![allow(clippy::wildcard_imports)]
use super::super::*;

pub fn parse_list_membership_options(
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

pub fn parse_association_search_options(
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

#[cfg(test)]
mod tests {
    use super::*;

    const SPAN: Span = Span::new(0, 1);

    #[test]
    fn parse_list_membership_options_accepts_key_and_test() {
        let parsed = parse_list_membership_options(
            &[
                Value::keyword("key"),
                Value::Nil,
                Value::keyword("test"),
                Value::Nil,
            ],
            false,
            SPAN,
        );
        assert!(parsed.is_ok());
    }

    #[test]
    fn parse_association_search_options_accepts_test_not() {
        let parsed = parse_association_search_options(
            &[Value::keyword("test-not"), Value::Nil],
            false,
            SPAN,
        );
        assert!(parsed.is_ok());
    }
}
