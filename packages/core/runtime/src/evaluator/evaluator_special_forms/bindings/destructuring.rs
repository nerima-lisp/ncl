use super::{Environment, Form, FormKind, HashSet, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn special_destructuring_bind(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(Self::arity(
                "destructuring-bind",
                "two or more",
                items.len().saturating_sub(1),
            ));
        }
        let lambda_list = match &items[1].kind {
            FormKind::List(_) => {
                let lambda_list = Self::macro_parameters(&items[1])?;
                if lambda_list.environment.is_some() {
                    return Err(Self::invalid(
                        "&environment is only valid in macro lambda lists",
                        items[1].span,
                    ));
                }
                Some(lambda_list)
            }
            _ => None,
        };
        let mut seen = HashSet::new();
        let pattern = lambda_list
            .is_none()
            .then(|| Self::macro_pattern(&items[1], &mut seen));
        let pattern = pattern.transpose()?;
        let local = environment.child();
        let _dynamic_guard = self.dynamic_guard();
        let value = self.eval_in(&items[2], environment)?.primary_value();
        if let Some(lambda_list) = &lambda_list
            && let Some(whole) = &lambda_list.whole
        {
            local.define(whole, value.clone());
        }
        if let Some(lambda_list) = lambda_list {
            self.bind_destructuring_lambda_list(&lambda_list, &value, &local, items[1].span)?;
        } else if let Some(pattern) = pattern {
            Self::bind_macro_pattern(&pattern, value, &local, items[1].span)?;
        }
        self.eval_sequence_values(&items[3..], &local)
    }
}
