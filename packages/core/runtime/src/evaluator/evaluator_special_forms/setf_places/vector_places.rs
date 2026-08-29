use super::{Environment, Form, RuntimeError, Runtime, Span, Value};

impl Runtime {
    pub(super) fn set_vector_index_place(
        &self,
        operator: &str,
        args: &[Form],
        value: Value,
        environment: &Environment,
        place_span: Span,
    ) -> Result<(), RuntimeError> {
        match operator {
            "SVREF" => {
                if args.len() != 2 {
                    return Err(Self::arity("setf svref", "two", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let index = Self::setf_index(self.eval_in(&args[1], environment)?, args[1].span)?;
                let Value::Vector(_) = &current else {
                    return Err(RuntimeError::Type {
                        expected: "SIMPLE-VECTOR".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                };
                let mut elements = current
                    .vector_items()
                    .ok_or_else(|| Self::invalid("SETF target is not a vector", place_span))?;
                let Some(slot) = elements.get_mut(index) else {
                    return Err(Self::invalid("SETF index is out of bounds", args[1].span));
                };
                *slot = value;
                self.set_place(&args[0], Value::vector(elements), environment)
            }
            "ROW-MAJOR-AREF" => {
                if args.len() != 2 {
                    return Err(Self::arity("setf row-major-aref", "two", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let index = Self::setf_index(self.eval_in(&args[1], environment)?, args[1].span)?;
                match &current {
                    Value::Vector(_) => {
                        let mut elements = current.vector_items().ok_or_else(|| {
                            Self::invalid("SETF target is not a vector", args[0].span)
                        })?;
                        let Some(slot) = elements.get_mut(index) else {
                            return Err(Self::invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = value;
                        self.set_place(&args[0], Value::vector(elements), environment)
                    }
                    Value::Array { .. } => {
                        let mut elements = current.array_items().ok_or_else(|| {
                            Self::invalid("SETF target is not an array", args[0].span)
                        })?;
                        let Some(slot) = elements.get_mut(index) else {
                            return Err(Self::invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = value;
                        let dimensions = current.array_dimensions().ok_or_else(|| {
                            Self::invalid("SETF target is not an array", args[0].span)
                        })?;
                        self.set_place(&args[0], Value::array(dimensions, elements), environment)
                    }
                    other => Err(RuntimeError::Type {
                        expected: "ARRAY or VECTOR".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(args[0].span),
                    }),
                }
            }
            _ => unreachable!("set_vector_index_place called with unsupported operator"),
        }
    }
}
