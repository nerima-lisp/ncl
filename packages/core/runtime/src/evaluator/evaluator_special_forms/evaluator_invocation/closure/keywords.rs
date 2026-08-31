use super::{ClosureKeywordApplicationContext, Runtime, RuntimeError, Value};
use std::collections::HashMap;

impl Runtime {
    pub(super) fn apply_closure_keywords(
        &self,
        context: &ClosureKeywordApplicationContext<'_>,
    ) -> Result<(), RuntimeError> {
        let ClosureKeywordApplicationContext {
            keywords,
            arguments,
            key_start,
            allow_other_keys,
            local,
            special_parameters,
            span,
        } = *context;
        let keyword_arguments = &arguments[key_start..];
        if !keyword_arguments.len().is_multiple_of(2) {
            return Err(Self::invalid(
                "keyword arguments must be supplied in pairs",
                span,
            ));
        }
        let mut supplied_keywords = HashMap::new();
        let mut accepts_unknown_keywords = allow_other_keys;
        for pair in keyword_arguments.as_chunks::<2>().0 {
            let (Value::Keyword(keyword) | Value::KeywordExact(keyword)) = &pair[0] else {
                return Err(Self::invalid(
                    "keyword argument name must be a keyword",
                    span,
                ));
            };
            let keyword_name = keyword.to_string();
            if keyword_name == "ALLOW-OTHER-KEYS" && pair[1].is_truthy() {
                accepts_unknown_keywords = true;
            }
            supplied_keywords.insert(keyword_name, pair[1].clone());
        }
        if !accepts_unknown_keywords {
            for keyword_name in supplied_keywords.keys() {
                if keyword_name != "ALLOW-OTHER-KEYS"
                    && !keywords
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
        for specification in keywords {
            let supplied = supplied_keywords.get(&specification.keyword_name);
            let value = match supplied {
                Some(argument) => argument.clone(),
                None => self.eval_in(&specification.init_form, local)?,
            };
            self.define_closure_binding(
                &specification.name,
                specification.name_escaped,
                value,
                local,
                special_parameters,
            );
            if let Some(supplied_p) = &specification.supplied_p {
                let supplied_value = Value::boolean(supplied.is_some());
                self.define_closure_binding(
                    supplied_p,
                    specification.supplied_p_escaped.unwrap_or(false),
                    supplied_value,
                    local,
                    special_parameters,
                );
            }
        }
        Ok(())
    }
}
