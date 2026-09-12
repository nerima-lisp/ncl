use super::{Environment, Form, FormKind, Runtime, RuntimeError, Span, Value};

impl Runtime {
    pub(crate) fn set_svref_place_value(
        &self,
        place: &Form,
        index: &Value,
        current: &Value,
        value: Value,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        let index = Self::setf_index(index.clone(), place.span)?;
        let mut elements = current.vector_items().ok_or_else(|| RuntimeError::Type {
            expected: "SIMPLE-VECTOR".to_string(),
            actual: current.type_name().to_string(),
            span: Some(place.span),
        })?;
        let Some(slot) = elements.get_mut(index) else {
            return Err(Self::invalid("SETF index is out of bounds", place.span));
        };
        *slot = value;
        let FormKind::List(items) = &place.kind else {
            unreachable!()
        };
        self.set_place(&items[1], Value::vector(elements), environment)
    }

    pub(crate) fn set_row_major_aref_place_value(
        &self,
        place: &Form,
        index: &Value,
        current: &Value,
        value: Value,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        let index = Self::setf_index(index.clone(), place.span)?;
        let FormKind::List(items) = &place.kind else {
            unreachable!()
        };
        match current {
            Value::Vector(_) => {
                let mut elements = current
                    .vector_items()
                    .ok_or_else(|| Self::invalid("SETF target is not a vector", place.span))?;
                let Some(slot) = elements.get_mut(index) else {
                    return Err(Self::invalid("SETF index is out of bounds", place.span));
                };
                *slot = value;
                self.set_place(&items[1], Value::vector(elements), environment)
            }
            Value::Array { .. } => {
                let mut elements = current
                    .array_items()
                    .ok_or_else(|| Self::invalid("SETF target is not an array", place.span))?;
                let Some(slot) = elements.get_mut(index) else {
                    return Err(Self::invalid("SETF index is out of bounds", place.span));
                };
                *slot = value;
                let dimensions = current
                    .array_dimensions()
                    .ok_or_else(|| Self::invalid("SETF target is not an array", place.span))?;
                self.set_place(&items[1], Value::array(dimensions, elements), environment)
            }
            other => Err(RuntimeError::Type {
                expected: "ARRAY or VECTOR".to_string(),
                actual: other.type_name().to_string(),
                span: Some(items[1].span),
            }),
        }
    }

    pub(super) fn set_fill_pointer_place(
        &self,
        args: &[Form],
        value: Value,
        environment: &Environment,
        place_span: Span,
    ) -> Result<(), RuntimeError> {
        if args.len() != 1 {
            return Err(Self::arity("setf fill-pointer", "one", args.len()));
        }
        let current = self.eval_in(&args[0], environment)?;
        let index = Self::setf_index(value, place_span)?;
        let Value::Vector(elements) = &current else {
            return Err(RuntimeError::Type {
                expected: "VECTOR".to_string(),
                actual: current.type_name().to_string(),
                span: Some(args[0].span),
            });
        };
        if !elements.set_fill_pointer(index) {
            return Err(Self::invalid(
                "SETF fill-pointer requires a vector with a valid fill pointer",
                place_span,
            ));
        }
        Ok(())
    }

    pub(super) fn set_vector_index_place(
        &self,
        operator: &str,
        args: &[Form],
        value: Value,
        environment: &Environment,
        _place_span: Span,
    ) -> Result<(), RuntimeError> {
        match operator {
            "SVREF" => {
                if args.len() != 2 {
                    return Err(Self::arity("setf svref", "two", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let index = Self::setf_index(self.eval_in(&args[1], environment)?, args[1].span)?;
                let Value::Vector(elements) = &current else {
                    return Err(RuntimeError::Type {
                        expected: "SIMPLE-VECTOR".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                };
                if !elements.set(index, value) {
                    return Err(Self::invalid("SETF index is out of bounds", args[1].span));
                }
                Ok(())
            }
            "ROW-MAJOR-AREF" => {
                if args.len() != 2 {
                    return Err(Self::arity("setf row-major-aref", "two", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let index = Self::setf_index(self.eval_in(&args[1], environment)?, args[1].span)?;
                match &current {
                    Value::Vector(elements) | Value::Array { elements, .. } => {
                        if !elements.set(index, value) {
                            return Err(Self::invalid("SETF index is out of bounds", args[1].span));
                        }
                        Ok(())
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
