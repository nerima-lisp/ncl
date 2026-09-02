use super::{
    Environment, MethodContinuation, MethodDefinition, RefCell, Runtime,
    RuntimeError, Span, Value,
};
use crate::value::MethodCombination;

impl Runtime {
    fn method_score(method: &MethodDefinition, arguments: &[Value]) -> Option<usize> {
        let required_count = method.specializers.len();
        if arguments.len() < required_count {
            return None;
        }
        if let Value::Function(function) = &method.function
            && let crate::Function::Closure {
                parameters,
                optional,
                rest,
                has_keyword_section,
                ..
            } = function.as_ref()
            && (parameters.len() != required_count
                || (!*has_keyword_section
                    && rest.is_none()
                    && arguments.len() > required_count + optional.len()))
        {
            return None;
        }
        let mut score = 0usize;
        for (specializer, argument) in method
            .specializers
            .iter()
            .zip(arguments.iter().take(required_count))
        {
            if specializer.as_ref() == "T" || specializer.as_ref() == "OBJECT" {
                score = score.saturating_add(1_000_000);
                continue;
            }
            let class = argument.instance_class_definition()?;
            let position = class
                .precedence
                .iter()
                .position(|name| name == specializer)?;
            score = score.saturating_add(position);
        }
        Some(score)
    }

    pub(super) fn apply_generic(
        &self,
        name: &str,
        method_combination: MethodCombination,
        methods: &RefCell<Vec<MethodDefinition>>,
        arguments: &[Value],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let mut applicable = methods
            .borrow()
            .iter()
            .filter_map(|method| {
                Self::method_score(method, arguments).map(|score| (score, method.clone()))
            })
            .collect::<Vec<_>>();
        if applicable.is_empty() {
            return Err(Self::invalid(
                &format!("no applicable method for {name}"),
                span,
            ));
        }
        applicable.sort_by_key(|(score, _)| *score);

        let mut around = Vec::new();
        let mut before = Vec::new();
        let mut primary = Vec::new();
        let mut after = Vec::new();
        for (_, method) in applicable {
            match method.qualifiers.first().map(String::as_str) {
                Some("AROUND") => around.push(method),
                Some("BEFORE") => before.push(method),
                Some("AFTER") => after.push(method),
                _ => primary.push(method),
            }
        }
        after.reverse();
        if method_combination != MethodCombination::Standard {
            if !around.is_empty() || !before.is_empty() || !after.is_empty() {
                return Err(Self::invalid(
                    "auxiliary methods are not supported with this method combination",
                    span,
                ));
            }
            let is_and = method_combination == MethodCombination::And;
            for method in primary {
                let value = self.invoke_method(&method, arguments, None, span, environment)?;
                if (is_and && !value.is_truthy()) || (!is_and && value.is_truthy()) {
                    return Ok(value);
                }
            }
            return Ok(if is_and {
                Value::boolean(true)
            } else {
                Value::Nil
            });
        }
        let core = MethodContinuation::Core {
            before,
            primary,
            after,
        };
        if around.is_empty() {
            self.invoke_continuation(core, arguments, span, environment)
        } else {
            let first = around[0].clone();
            let next = MethodContinuation::Chain {
                methods: around,
                index: 1,
                fallback: Some(Box::new(core)),
            };
            self.invoke_method(&first, arguments, Some(next), span, environment)
        }
    }
}
