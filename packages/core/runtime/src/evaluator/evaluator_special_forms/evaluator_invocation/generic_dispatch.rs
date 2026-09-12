use super::{
    Environment, MethodContinuation, MethodDefinition, RefCell, Runtime, RuntimeError, Span, Value,
};
use crate::value::{MethodCombination, MethodSpecializer};

impl Runtime {
    fn method_score(
        method: &MethodDefinition,
        arguments: &[Value],
        environment: &Environment,
    ) -> Option<Vec<usize>> {
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
        let mut score = Vec::with_capacity(required_count);
        for (specializer, argument) in method
            .specializers
            .iter()
            .zip(arguments.iter().take(required_count))
        {
            if let MethodSpecializer::Eql(expected) = specializer {
                if !crate::builtins::eql_value(expected, argument) {
                    return None;
                }
                score.push(0);
                continue;
            }
            let MethodSpecializer::Class(specializer) = specializer else {
                unreachable!()
            };
            if specializer.as_ref() == "T" || specializer.as_ref() == "OBJECT" {
                score.push(1_000_000);
                continue;
            }
            if let Some(class) = argument.instance_class_definition() {
                let position = class
                    .precedence
                    .iter()
                    .position(|name| name == specializer)?;
                score.push(position.saturating_add(1));
            } else {
                let type_designator = Value::symbol(specializer.clone());
                if !crate::builtins::typep_value_in(argument, &type_designator, environment).ok()? {
                    return None;
                }
                score.push(match specializer.as_ref() {
                    "NIL" => 1,
                    "BIT" | "FIXNUM" | "BIGNUM" | "INTEGER" => 100,
                    "RATIO" | "RATIONAL" => 200,
                    "FLOAT" | "SHORT-FLOAT" | "SINGLE-FLOAT" | "DOUBLE-FLOAT" | "LONG-FLOAT"
                    | "REAL" => 300,
                    "NUMBER" => 400,
                    _ => 500_000,
                });
            }
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
                Self::method_score(method, arguments, environment)
                    .map(|score| (score, method.clone()))
            })
            .collect::<Vec<_>>();
        if applicable.is_empty() {
            return Err(Self::invalid(
                &format!("no applicable method for {name}"),
                span,
            ));
        }
        applicable.sort_by(|(left, _), (right, _)| left.cmp(right));

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
            if method_combination == MethodCombination::Progn {
                let mut result = Value::Nil;
                for method in primary {
                    result = self.invoke_method(&method, arguments, None, span, environment)?;
                }
                return Ok(result);
            }
            if matches!(
                method_combination,
                MethodCombination::List
                    | MethodCombination::Append
                    | MethodCombination::Nconc
                    | MethodCombination::Plus
                    | MethodCombination::Max
                    | MethodCombination::Min
            ) {
                let values = primary
                    .iter()
                    .map(|method| self.invoke_method(method, arguments, None, span, environment))
                    .collect::<Result<Vec<_>, _>>()?;
                return match method_combination {
                    MethodCombination::List => Ok(Value::list(values)),
                    MethodCombination::Append => crate::builtins::append_lists("append", &values),
                    MethodCombination::Nconc => crate::builtins::nconc(&values),
                    MethodCombination::Plus => crate::builtins::add(&values),
                    MethodCombination::Max => crate::builtins::maximum(&values),
                    MethodCombination::Min => crate::builtins::minimum(&values),
                    _ => unreachable!("method combination was checked above"),
                };
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
