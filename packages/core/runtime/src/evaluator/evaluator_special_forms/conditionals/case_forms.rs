use ncl_syntax::{Form, FormKind};

use crate::builtins;
use crate::evaluator::helpers::is_case_default_form;
use crate::evaluator::quoted_form_value;
use crate::{Environment, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn special_case(
        &self,
        items: &[Form],
        environment: &Environment,
        error_on_miss: bool,
    ) -> Result<Value, RuntimeError> {
        self.special_case_like(items, environment, error_on_miss, false)
    }

    pub(crate) fn special_typecase(
        &self,
        items: &[Form],
        environment: &Environment,
        error_on_miss: bool,
    ) -> Result<Value, RuntimeError> {
        self.special_case_like(items, environment, error_on_miss, true)
    }

    fn special_case_like(
        &self,
        items: &[Form],
        environment: &Environment,
        error_on_miss: bool,
        type_case: bool,
    ) -> Result<Value, RuntimeError> {
        let operator = match (type_case, error_on_miss) {
            (true, true) => "etypecase",
            (true, false) => "typecase",
            (false, true) => "ecase",
            (false, false) => "case",
        };
        if items.len() < 2 {
            return Err(Self::arity(
                operator,
                "at least one",
                items.len().saturating_sub(1),
            ));
        }

        let key = self.eval_in(&items[1], environment)?;
        let mut default_body: Option<&[Form]> = None;
        for clause in &items[2..] {
            let FormKind::List(parts) = &clause.kind else {
                return Err(Self::invalid(
                    if type_case {
                        "typecase clauses must be lists"
                    } else {
                        "case clauses must be lists"
                    },
                    clause.span,
                ));
            };
            if parts.is_empty() {
                return Err(Self::invalid(
                    if type_case {
                        "typecase clause cannot be empty"
                    } else {
                        "case clause cannot be empty"
                    },
                    clause.span,
                ));
            }
            if is_case_default_form(&parts[0]) {
                default_body = Some(&parts[1..]);
                continue;
            }

            let matches = if type_case {
                builtins::typep_value(&key, &quoted_form_value(&parts[0])?)?
            } else {
                let keys = match &parts[0].kind {
                    FormKind::List(keys) => keys.as_slice(),
                    _ => std::slice::from_ref(&parts[0]),
                };
                keys.iter()
                    .try_fold(false, |matched, key_form| -> Result<bool, RuntimeError> {
                        Ok(matched || builtins::eql_value(&key, &quoted_form_value(key_form)?))
                    })?
            };
            if matches {
                return self.eval_sequence_values(&parts[1..], environment);
            }
        }

        default_body.map_or_else(
            || {
                if error_on_miss {
                    Err(Self::invalid(
                        if type_case {
                            "etypecase fell through"
                        } else {
                            "ecase fell through"
                        },
                        items[0].span,
                    ))
                } else {
                    Ok(Value::Nil)
                }
            },
            |body| self.eval_sequence_values(body, environment),
        )
    }
}
