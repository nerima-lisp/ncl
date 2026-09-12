#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn special_quote(&self, items: &[Form], span: Span) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(Self::arity("quote", "one", items.len().saturating_sub(1)));
        }
        self.runtime_quoted_value(&items[1])
            .map_err(|error| match error {
                RuntimeError::InvalidForm { .. } => Self::invalid("invalid quoted form", span),
                error => error,
            })
    }

    pub(crate) fn special_the(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(Self::arity("the", "two", items.len().saturating_sub(1)));
        }
        let type_designator = self.runtime_quoted_value(&items[1])?;
        let value = self.eval_in(&items[2], environment)?;
        builtins::the_check(&[value, type_designator])
    }

    pub(crate) fn special_load_time_value(&self, items: &[Form]) -> Result<Value, RuntimeError> {
        if !(2..=3).contains(&items.len()) {
            return Err(Self::arity(
                "load-time-value",
                "one or two",
                items.len().saturating_sub(1),
            ));
        }
        if let Some(read_only_p) = items.get(2)
            && !matches!(
                Self::quoted_value(read_only_p)?,
                Value::Nil | Value::Boolean(_)
            )
            && !atom_name(read_only_p).is_some_and(|atom| {
                parse_symbol_token(atom).is_ok_and(|token| {
                    token.kind == SymbolTokenKind::Symbol
                        && matches!(token.name.as_str(), "T" | "NIL")
                        && token.package.as_deref().is_none_or(|name| {
                            package::normalize_package_name(name) == package::COMMON_LISP_PACKAGE
                        })
                })
            })
        {
            return Err(Self::invalid(
                "load-time-value read-only-p must be a literal boolean",
                read_only_p.span,
            ));
        }
        self.eval_in(&items[1], &self.global)
    }

    pub(crate) fn special_nth_value(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(Self::arity(
                "nth-value",
                "two",
                items.len().saturating_sub(1),
            ));
        }
        let index_value = self.eval_in(&items[1], environment)?;
        let index = match index_value {
            Value::Integer(index) if index >= 0 => {
                usize::try_from(index).map_err(|_| RuntimeError::NumericOverflow)?
            }
            Value::Integer(_) => {
                return Err(Self::invalid(
                    "nth-value index must be non-negative",
                    items[1].span,
                ));
            }
            Value::BigInteger(_) => return Err(RuntimeError::NumericOverflow),
            value => {
                return Err(RuntimeError::Type {
                    expected: "INTEGER".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(items[1].span),
                });
            }
        };
        let values = self
            .eval_values_in(&items[2], environment)?
            .multiple_values();
        Ok(values.get(index).cloned().unwrap_or(Value::Nil))
    }

    pub(crate) fn special_locally(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let forms = items.get(1..).unwrap_or(&[]);
        let special_names = Self::special_declaration_names(forms)?;
        let local = environment.child();
        Self::declare_special_names(&local, &special_names);
        let _dynamic_guard = self.dynamic_guard();
        self.eval_sequence_values(forms, &local)
    }

    pub(crate) fn special_eval_when(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(Self::arity(
                "eval-when",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        if Self::eval_when_executes(&items[1])? {
            self.eval_sequence_values(items.get(2..).unwrap_or(&[]), environment)
        } else {
            Ok(Value::Nil)
        }
    }

    pub(crate) fn special_with_compilation_unit(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(Self::arity(
                "with-compilation-unit",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        self.eval_sequence_values(items.get(2..).unwrap_or(&[]), environment)
    }

    pub(crate) fn eval_when_executes(form: &Form) -> Result<bool, RuntimeError> {
        let FormKind::List(situations) = &form.kind else {
            return Err(Self::invalid(
                "eval-when situations must be a list",
                form.span,
            ));
        };
        let mut executes = false;
        for situation in situations {
            let Some(name) = atom_name(situation) else {
                return Err(Self::invalid(
                    "eval-when situations must contain symbols",
                    situation.span,
                ));
            };
            let token = parse_symbol_token(name).map_err(|_| {
                Self::invalid("eval-when situations must contain symbols", situation.span)
            })?;
            if token.kind == SymbolTokenKind::Uninterned
                || (token.kind == SymbolTokenKind::Symbol && literal_atom(name).is_some())
            {
                return Err(Self::invalid(
                    "eval-when situations must contain symbols",
                    situation.span,
                ));
            }
            if token.package.is_none() && token.name.eq_ignore_ascii_case("execute") {
                executes = true;
            }
        }
        Ok(executes)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Runtime, RuntimeError};

    #[test]
    fn the_and_nth_value_report_type_and_evaluation_errors() {
        assert!(Runtime::new().eval_source("(the integer (car 5))").is_err());

        let arity = Runtime::new().eval_source("(nth-value 1)").map_or_else(
            |error| error,
            |value| panic!("expected an error, got {value:?}"),
        );
        assert!(matches!(
            arity,
            RuntimeError::Arity { function, expected, actual: 1 }
                if function == "nth-value" && expected == "two"
        ));

        let type_error = Runtime::new()
            .eval_source("(nth-value \"x\" 1)")
            .map_or_else(
                |error| error,
                |value| panic!("expected an error, got {value:?}"),
            );
        assert!(matches!(
            type_error,
            RuntimeError::Type { expected, actual, .. }
                if expected == "INTEGER" && actual == "STRING"
        ));

        assert!(Runtime::new().eval_source("(nth-value 0 (car 5))").is_err());
    }

    #[test]
    fn load_time_value_propagates_body_errors() {
        assert!(matches!(
            Runtime::new().eval_source("(load-time-value (car 5))"),
            Err(RuntimeError::Type { .. })
        ));
    }

    #[test]
    fn eval_when_rejects_a_situation_with_invalid_symbol_syntax() {
        assert!(Runtime::new().eval_source("(eval-when (a:b:c) 1)").is_err());
    }
}
