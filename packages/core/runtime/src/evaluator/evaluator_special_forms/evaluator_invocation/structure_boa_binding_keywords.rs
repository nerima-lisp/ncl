use super::{Environment, HashMap, OrdinaryLambdaList, Runtime, RuntimeError, Span, Value};

pub(super) struct StructureBoaKeywordContext<'a, F, D>
where
    F: Fn(&str) -> Option<usize>,
    D: Fn(&str) -> Result<Value, RuntimeError>,
{
    pub(super) lambda_list: &'a OrdinaryLambdaList,
    pub(super) arguments: &'a [Value],
    pub(super) key_start: usize,
    pub(super) span: Span,
    pub(super) local: &'a Environment,
    pub(super) slot_index: &'a F,
    pub(super) evaluate_slot_default: &'a D,
    pub(super) slot_values: &'a mut [Option<Value>],
}

impl Runtime {
    pub(super) fn bind_structure_boa_keywords<F, D>(
        &self,
        context: StructureBoaKeywordContext<'_, F, D>,
    ) -> Result<(), RuntimeError>
    where
        F: Fn(&str) -> Option<usize>,
        D: Fn(&str) -> Result<Value, RuntimeError>,
    {
        let StructureBoaKeywordContext {
            lambda_list,
            arguments,
            key_start,
            span,
            local,
            slot_index,
            evaluate_slot_default,
            slot_values,
        } = context;
        let keyword_arguments = &arguments[key_start..];
        if !keyword_arguments.len().is_multiple_of(2) {
            return Err(Self::invalid(
                "keyword arguments must be supplied in pairs",
                span,
            ));
        }
        let mut supplied_keywords = HashMap::new();
        let mut accepts_unknown_keywords = lambda_list.allow_other_keys;
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
        for specification in &lambda_list.keywords {
            let supplied = supplied_keywords.get(&specification.keyword_name);
            let value = match supplied {
                Some(argument) => argument.clone(),
                None if specification.init_form_supplied => {
                    self.eval_in(&specification.init_form, local)?
                }
                None => evaluate_slot_default(&specification.name)?,
            };
            if specification.name_escaped {
                self.define_exact_in(&specification.name, value.clone(), local);
            } else {
                self.define_in(&specification.name, value.clone(), local);
            }
            if let Some(index) = slot_index(&specification.name) {
                slot_values[index] = Some(value);
            }
            if let Some(supplied_p) = &specification.supplied_p {
                let supplied_value = Value::boolean(supplied.is_some());
                if specification.supplied_p_escaped.unwrap_or(false) {
                    self.define_exact_in(supplied_p, supplied_value, local);
                } else {
                    self.define_in(supplied_p, supplied_value, local);
                }
            }
        }
        Ok(())
    }
}
