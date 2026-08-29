use std::collections::HashMap;

use ncl_syntax::{Form, Span};

use crate::evaluator::helpers::macro_keyword_name;
use crate::value::MacroLambdaList;
use crate::{Runtime, RuntimeError, Value};

impl Runtime {
    pub(super) fn parse_macro_keywords(
        keyword_arguments: &[Form],
        lambda_list: &MacroLambdaList,
        span: Span,
    ) -> Result<HashMap<String, Form>, RuntimeError> {
        if !keyword_arguments.len().is_multiple_of(2) {
            return Err(Self::invalid(
                "keyword arguments must be supplied in pairs",
                span,
            ));
        }
        let mut supplied = HashMap::new();
        let mut accepts_unknown_keywords = lambda_list.allow_other_keys;
        for pair in keyword_arguments.as_chunks::<2>().0 {
            let Some(keyword_name) = macro_keyword_name(&pair[0]) else {
                return Err(Self::invalid(
                    "keyword argument name must be a keyword",
                    pair[0].span,
                ));
            };
            if keyword_name == "ALLOW-OTHER-KEYS" && Self::quoted_value(&pair[1])?.is_truthy() {
                accepts_unknown_keywords = true;
            }
            supplied.insert(keyword_name, pair[1].clone());
        }
        if !accepts_unknown_keywords {
            for keyword_name in supplied.keys() {
                if keyword_name != "ALLOW-OTHER-KEYS"
                    && !lambda_list
                        .keywords
                        .iter()
                        .any(|specification| specification.keyword_name == *keyword_name)
                {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown keyword :{keyword_name}"),
                        span: Some(span),
                    });
                }
            }
        }
        Ok(supplied)
    }

    pub(super) fn parse_destructuring_keywords(
        keyword_arguments: &[Value],
        lambda_list: &MacroLambdaList,
        span: Span,
    ) -> Result<HashMap<String, Value>, RuntimeError> {
        if !keyword_arguments.len().is_multiple_of(2) {
            return Err(Self::invalid(
                "keyword arguments must be supplied in pairs",
                span,
            ));
        }
        let mut supplied = HashMap::new();
        let mut accepts_unknown = lambda_list.allow_other_keys;
        for pair in keyword_arguments.as_chunks::<2>().0 {
            let (Value::Keyword(keyword) | Value::KeywordExact(keyword)) = &pair[0] else {
                return Err(Self::invalid(
                    "keyword argument name must be a keyword",
                    span,
                ));
            };
            let name = keyword.to_string();
            accepts_unknown |= name == "ALLOW-OTHER-KEYS" && pair[1].is_truthy();
            supplied.insert(name, pair[1].clone());
        }
        if !accepts_unknown {
            for name in supplied.keys() {
                if name != "ALLOW-OTHER-KEYS"
                    && !lambda_list
                        .keywords
                        .iter()
                        .any(|specification| specification.keyword_name == *name)
                {
                    return Err(Self::invalid(&format!("unknown keyword :{name}"), span));
                }
            }
        }
        Ok(supplied)
    }
}
