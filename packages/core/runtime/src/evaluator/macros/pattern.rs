use ncl_syntax::Span;

use crate::evaluator::helpers::macro_dotted_parts;
use crate::value::MacroPattern;
use crate::{Environment, Runtime, RuntimeError, Value};

impl Runtime {
    pub(in crate::evaluator) fn bind_macro_pattern(
        pattern: &MacroPattern,
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        match pattern {
            MacroPattern::Name(name) => {
                environment.define(name, value);
                Ok(())
            }
            MacroPattern::List(patterns) => {
                let Some(values) = value.list_items() else {
                    return Err(Self::invalid(
                        "macro destructuring pattern requires a proper list",
                        span,
                    ));
                };
                if values.len() != patterns.len() {
                    return Err(Self::invalid(
                        "macro destructuring pattern has the wrong number of elements",
                        span,
                    ));
                }
                for (pattern, value) in patterns.iter().zip(values) {
                    Self::bind_macro_pattern(pattern, value, environment, span)?;
                }
                Ok(())
            }
            MacroPattern::Dotted { items, tail } => {
                let Some((values, dotted_tail)) = macro_dotted_parts(&value) else {
                    return Err(Self::invalid(
                        "macro destructuring pattern requires a list",
                        span,
                    ));
                };
                if values.len() < items.len() {
                    return Err(Self::invalid(
                        "macro destructuring pattern has too few elements",
                        span,
                    ));
                }
                for (pattern, value) in items.iter().zip(values.iter().cloned()) {
                    Self::bind_macro_pattern(pattern, value, environment, span)?;
                }
                let remaining = values[items.len()..].to_vec();
                let tail_value = if remaining.is_empty() {
                    dotted_tail
                } else if dotted_tail.is_truthy() {
                    Value::dotted_list(remaining, dotted_tail)
                } else {
                    Value::list(remaining)
                };
                Self::bind_macro_pattern(tail, tail_value, environment, span)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ncl_syntax::Span;

    use crate::value::MacroPattern;
    use crate::{Environment, Runtime, Value};

    const SPAN: Span = Span::new(0, 1);

    fn bound_value(environment: &Environment, name: &str) -> Value {
        environment
            .lookup(name)
            .unwrap_or_else(|| panic!("expected {name} to be bound"))
    }

    #[test]
    fn list_pattern_rejects_a_mismatched_element_count() {
        let environment = Environment::new();
        let pattern = MacroPattern::List(vec![
            MacroPattern::Name("A".into()),
            MacroPattern::Name("B".into()),
        ]);
        let value = Value::list(vec![Value::Integer(1)]);

        assert!(Runtime::bind_macro_pattern(&pattern, value, &environment, SPAN).is_err());
    }

    #[test]
    fn list_pattern_propagates_errors_from_nested_patterns() {
        let environment = Environment::new();
        let pattern = MacroPattern::List(vec![MacroPattern::List(vec![MacroPattern::Name(
            "A".into(),
        )])]);
        let value = Value::list(vec![Value::Integer(1)]);

        assert!(Runtime::bind_macro_pattern(&pattern, value, &environment, SPAN).is_err());
    }

    #[test]
    fn dotted_pattern_rejects_non_list_values() {
        let environment = Environment::new();
        let pattern = MacroPattern::Dotted {
            items: vec![MacroPattern::Name("A".into())],
            tail: Box::new(MacroPattern::Name("REST".into())),
        };

        assert!(
            Runtime::bind_macro_pattern(&pattern, Value::Integer(1), &environment, SPAN).is_err()
        );
    }

    #[test]
    fn dotted_pattern_rejects_too_few_elements() {
        let environment = Environment::new();
        let pattern = MacroPattern::Dotted {
            items: vec![
                MacroPattern::Name("A".into()),
                MacroPattern::Name("B".into()),
            ],
            tail: Box::new(MacroPattern::Name("REST".into())),
        };
        let value = Value::list(vec![Value::Integer(1)]);

        assert!(Runtime::bind_macro_pattern(&pattern, value, &environment, SPAN).is_err());
    }

    #[test]
    fn dotted_pattern_binds_the_final_cdr_when_nothing_remains() {
        let environment = Environment::new();
        let pattern = MacroPattern::Dotted {
            items: vec![
                MacroPattern::Name("A".into()),
                MacroPattern::Name("B".into()),
            ],
            tail: Box::new(MacroPattern::Name("REST".into())),
        };
        let value = Value::dotted_list(
            vec![Value::Integer(1), Value::Integer(2)],
            Value::Integer(9),
        );

        if let Err(error) = Runtime::bind_macro_pattern(&pattern, value, &environment, SPAN) {
            panic!("expected binding to succeed: {error}");
        }
        assert_eq!(bound_value(&environment, "REST").to_string(), "9");
    }

    #[test]
    fn dotted_pattern_carries_a_truthy_final_cdr_into_the_remaining_tail() {
        let environment = Environment::new();
        let pattern = MacroPattern::Dotted {
            items: vec![MacroPattern::Name("A".into())],
            tail: Box::new(MacroPattern::Name("REST".into())),
        };
        let value = Value::dotted_list(
            vec![Value::Integer(1), Value::Integer(2)],
            Value::Integer(9),
        );

        if let Err(error) = Runtime::bind_macro_pattern(&pattern, value, &environment, SPAN) {
            panic!("expected binding to succeed: {error}");
        }
        assert_eq!(bound_value(&environment, "REST").to_string(), "(2 . 9)");
    }
}
