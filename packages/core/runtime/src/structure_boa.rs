use super::{OrdinaryLambdaList, Runtime, RuntimeError, Value};

impl Runtime {
    pub(super) fn structure_boa_argument_counts(
        lambda_list: &OrdinaryLambdaList,
        arguments: &[Value],
    ) -> Result<(usize, usize, usize), RuntimeError> {
        let required_count = lambda_list.required.len();
        let optional_count = lambda_list.optional.len();
        if arguments.len() < required_count {
            let expected = if optional_count > 0
                || lambda_list.rest.is_some()
                || lambda_list.has_keyword_section
            {
                format!("at least {required_count}")
            } else {
                required_count.to_string()
            };
            return Err(Self::arity(
                "structure constructor",
                &expected,
                arguments.len(),
            ));
        }
        let optional_supplied_count = if lambda_list.has_keyword_section {
            let available = arguments
                .len()
                .saturating_sub(required_count)
                .min(optional_count);
            (0..available)
                .take_while(|index| {
                    !matches!(
                        arguments[required_count + *index],
                        Value::Keyword(_) | Value::KeywordExact(_)
                    )
                })
                .count()
        } else {
            arguments
                .len()
                .saturating_sub(required_count)
                .min(optional_count)
        };
        let key_start = required_count + optional_supplied_count;
        if !lambda_list.has_keyword_section
            && lambda_list.rest.is_none()
            && arguments.len() > required_count + optional_count
        {
            let maximum = required_count + optional_count;
            let expected = if optional_count > 0 {
                format!("at most {maximum}")
            } else {
                maximum.to_string()
            };
            return Err(Self::arity(
                "structure constructor",
                &expected,
                arguments.len(),
            ));
        }
        Ok((required_count, optional_supplied_count, key_start))
    }
}
