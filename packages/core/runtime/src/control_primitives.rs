#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(super) fn special_quote(items: &[Form], span: Span) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(Self::arity("quote", "one", items.len().saturating_sub(1)));
        }
        Self::quoted_value(&items[1]).map_err(|error| match error {
            RuntimeError::InvalidForm { .. } => Self::invalid("invalid quoted form", span),
            error => error,
        })
    }

    pub(super) fn special_the(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(Self::arity("the", "two", items.len().saturating_sub(1)));
        }
        let type_designator = quoted_form_value(&items[1])?;
        let value = self.eval_in(&items[2], environment)?;
        builtins::the_check(&[value, type_designator])
    }

    pub(super) fn special_load_time_value(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(2..=3).contains(&items.len()) {
            return Err(Self::arity(
                "load-time-value",
                "one or two",
                items.len().saturating_sub(1),
            ));
        }
        let value = self.eval_values_in(&items[1], environment)?;
        if let Some(read_only_p) = items.get(2) {
            let _ = self.eval_in(read_only_p, environment)?;
        }
        Ok(value)
    }

    pub(super) fn special_nth_value(
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

    pub(super) fn special_locally(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        self.eval_sequence_values(items.get(1..).unwrap_or(&[]), environment)
    }

    pub(super) fn special_eval_when(
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

    pub(super) fn eval_when_executes(form: &Form) -> Result<bool, RuntimeError> {
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
