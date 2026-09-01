use super::{Environment, Form, Runtime, RuntimeError, Span, Value};

impl Runtime {
    pub(super) fn set_list_place(
        &self,
        operator: &str,
        args: &[Form],
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<Option<()>, RuntimeError> {
        match operator {
            "CAR" | "FIRST" => {
                if args.len() != 1 {
                    return Err(Self::arity("setf car", "one", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let Some(mut elements) = current.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                };
                if elements.is_empty() {
                    return Err(Self::invalid("cannot SETF CAR of NIL", args[0].span));
                }
                elements[0] = value;
                self.set_place(&args[0], Value::list(elements), environment)?;
            }
            "CDR" | "REST" => {
                if args.len() != 1 {
                    return Err(Self::arity("setf cdr", "one", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let Some(elements) = current.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                };
                if elements.is_empty() {
                    return Err(Self::invalid("cannot SETF CDR of NIL", args[0].span));
                }
                let Some(mut replacement) = value.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(span),
                    });
                };
                let mut rebuilt = Vec::with_capacity(elements.len() + replacement.len());
                rebuilt.push(elements[0].clone());
                rebuilt.append(&mut replacement);
                self.set_place(&args[0], Value::list(rebuilt), environment)?;
            }
            "NTH" | "SECOND" | "THIRD" => {
                let expected = if operator == "NTH" { 2 } else { 1 };
                if args.len() != expected {
                    return Err(Self::arity(
                        "setf list accessor",
                        if expected == 2 { "two" } else { "one" },
                        args.len(),
                    ));
                }
                let index = match operator {
                    "SECOND" => 1,
                    "THIRD" => 2,
                    _ => Self::setf_index(self.eval_in(&args[0], environment)?, args[0].span)?,
                };
                let target = if operator == "NTH" {
                    &args[1]
                } else {
                    &args[0]
                };
                let current = self.eval_in(target, environment)?;
                let Some(mut elements) = current.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(target.span),
                    });
                };
                let Some(slot) = elements.get_mut(index) else {
                    return Err(Self::invalid("SETF index is out of bounds", args[0].span));
                };
                *slot = value;
                self.set_place(target, Value::list(elements), environment)?;
            }
            _ => return Ok(None),
        }
        Ok(Some(()))
    }
}
