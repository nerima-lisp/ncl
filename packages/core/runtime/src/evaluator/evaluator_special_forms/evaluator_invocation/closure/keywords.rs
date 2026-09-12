use super::{ClosureKeywordApplicationContext, Environment, Runtime, RuntimeError, Value};
use std::collections::HashMap;

impl Runtime {
    pub(super) fn apply_closure_keywords(
        &self,
        context: &ClosureKeywordApplicationContext<'_>,
    ) -> Result<Environment, RuntimeError> {
        let ClosureKeywordApplicationContext {
            keywords,
            arguments,
            key_start,
            allow_other_keys,
            local,
            span,
            special_names,
        } = *context;
        let keyword_arguments = &arguments[key_start..];
        if !keyword_arguments.len().is_multiple_of(2) {
            return Err(Self::invalid(
                "keyword arguments must be supplied in pairs",
                span,
            ));
        }
        let mut supplied_keywords = HashMap::new();
        for pair in keyword_arguments.as_chunks::<2>().0 {
            let keyword_name = match &pair[0] {
                Value::Keyword(keyword) | Value::KeywordExact(keyword) => keyword.to_string(),
                Value::InternedSymbol(symbol) if symbol.keyword() => symbol.name().to_string(),
                _ => {
                    return Err(Self::invalid(
                        "keyword argument name must be a keyword",
                        span,
                    ));
                }
            };
            supplied_keywords
                .entry(keyword_name)
                .or_insert_with(|| pair[1].clone());
        }
        let accepts_unknown_keywords = allow_other_keys
            || supplied_keywords
                .get("ALLOW-OTHER-KEYS")
                .is_some_and(Value::is_truthy);
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
        let mut local = local.clone();
        for specification in keywords {
            let supplied = supplied_keywords.get(&specification.keyword_name);
            let value = match supplied {
                Some(argument) => argument.clone(),
                None => self.eval_in(&specification.init_form, &local)?,
            };
            local = local.child();
            if Self::declares_special(
                special_names,
                &specification.name,
                specification.name_escaped,
            ) {
                Self::declare_special_names(
                    &local,
                    &[(specification.name.clone(), specification.name_escaped)],
                );
            }
            if specification.name_escaped {
                self.define_exact_in(&specification.name, value, &local);
            } else {
                self.define_in(&specification.name, value, &local);
            }
            if let Some(supplied_p) = &specification.supplied_p {
                let supplied_value = Value::boolean(supplied.is_some());
                if Self::declares_special(
                    special_names,
                    supplied_p,
                    specification.supplied_p_escaped.unwrap_or(false),
                ) {
                    Self::declare_special_names(
                        &local,
                        &[(
                            supplied_p.clone(),
                            specification.supplied_p_escaped.unwrap_or(false),
                        )],
                    );
                }
                if specification.supplied_p_escaped.unwrap_or(false) {
                    self.define_exact_in(supplied_p, supplied_value, &local);
                } else {
                    self.define_in(supplied_p, supplied_value, &local);
                }
            }
        }
        Ok(local)
    }
}
