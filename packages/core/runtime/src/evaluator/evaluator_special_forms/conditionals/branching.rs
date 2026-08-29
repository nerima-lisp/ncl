use ncl_syntax::{Form, FormKind};

use crate::{Environment, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn special_when(
        &self,
        items: &[Form],
        environment: &Environment,
        positive: bool,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(Self::arity(
                if positive { "when" } else { "unless" },
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let condition = self.eval_in(&items[1], environment)?.is_truthy();
        if condition == positive {
            self.eval_sequence_values(&items[2..], environment)
        } else {
            Ok(Value::Nil)
        }
    }

    pub(crate) fn special_cond(
        &self,
        clauses: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        for clause in clauses {
            let FormKind::List(items) = &clause.kind else {
                return Err(Self::invalid("cond clauses must be lists", clause.span));
            };
            if items.is_empty() {
                return Err(Self::invalid("cond clause cannot be empty", clause.span));
            }
            let condition = self.eval_in(&items[0], environment)?;
            if condition.is_truthy() {
                return if items.len() == 1 {
                    Ok(condition)
                } else {
                    self.eval_sequence_values(&items[1..], environment)
                };
            }
        }
        Ok(Value::Nil)
    }
}
