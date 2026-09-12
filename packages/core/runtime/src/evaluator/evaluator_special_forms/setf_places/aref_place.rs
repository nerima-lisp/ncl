use super::{Environment, Form, FormKind, Runtime, RuntimeError, Span, Value};

impl Runtime {
    pub(crate) fn set_aref_vector_place_value(
        &self,
        place: &Form,
        index: &Value,
        current: &Value,
        value: Value,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        let index = Self::setf_index(index.clone(), place.span)?;
        let mut elements = current.vector_items().ok_or_else(|| RuntimeError::Type {
            expected: "VECTOR".to_string(),
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

    pub(crate) fn set_aref_array_place_value(
        &self,
        place: &Form,
        indices: &[Value],
        current: &Value,
        value: Value,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        let Value::Array { dimensions, .. } = current else {
            return Err(RuntimeError::Type {
                expected: "ARRAY".to_string(),
                actual: current.type_name().to_string(),
                span: Some(place.span),
            });
        };
        if indices.len() != dimensions.len() {
            return Err(Self::arity(
                "setf aref",
                &format!("{} indices", dimensions.len()),
                indices.len(),
            ));
        }
        let mut offset = 0_usize;
        for (axis, (dimension, index_value)) in dimensions.iter().zip(indices).enumerate() {
            let index = Self::setf_index(index_value.clone(), place.span)?;
            if index >= *dimension {
                return Err(Self::invalid("SETF index is out of bounds", place.span));
            }
            let stride = dimensions[axis + 1..]
                .iter()
                .try_fold(1_usize, |s, d| s.checked_mul(*d))
                .ok_or_else(|| Self::invalid("SETF index is too large", place.span))?;
            offset = offset
                .checked_add(
                    index
                        .checked_mul(stride)
                        .ok_or_else(|| Self::invalid("SETF index is too large", place.span))?,
                )
                .ok_or_else(|| Self::invalid("SETF index is too large", place.span))?;
        }
        let mut elements = current
            .array_items()
            .ok_or_else(|| Self::invalid("SETF target is not an array", place.span))?;
        let element_type = current
            .array_storage()
            .map(|storage| storage.element_type())
            .ok_or_else(|| Self::invalid("SETF target is not an array", place.span))?;
        if !crate::builtins::typep_value_in(&value, &element_type, environment)? {
            return Err(RuntimeError::Type {
                expected: element_type.to_string(),
                actual: value.type_name().to_string(),
                span: Some(place.span),
            });
        }
        let Some(slot) = elements.get_mut(offset) else {
            return Err(Self::invalid("SETF index is out of bounds", place.span));
        };
        *slot = value;
        let FormKind::List(items) = &place.kind else {
            unreachable!()
        };
        self.set_place(
            &items[1],
            Value::array(dimensions.as_ref().clone(), elements),
            environment,
        )
    }

    pub(super) fn set_aref_place(
        &self,
        args: &[Form],
        value: Value,
        environment: &Environment,
        place_span: Span,
    ) -> Result<(), RuntimeError> {
        if args.is_empty() {
            return Err(Self::arity("setf aref", "at least one", args.len()));
        }
        let current = self.eval_in(&args[0], environment)?;
        let indices = args[1..]
            .iter()
            .map(|argument| self.eval_in(argument, environment))
            .collect::<Result<Vec<_>, _>>()?;
        match &current {
            Value::Vector(elements) => {
                if indices.len() != 1 {
                    return Err(Self::arity("setf aref", "two", args.len()));
                }
                let index = Self::setf_index(indices[0].clone(), args[1].span)?;
                let element_type = elements.element_type();
                if !crate::builtins::typep_value_in(&value, &element_type, environment)? {
                    return Err(RuntimeError::Type {
                        expected: element_type.to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(place_span),
                    });
                }
                if !elements.set(index, value) {
                    return Err(Self::invalid("SETF index is out of bounds", args[1].span));
                }
                Ok(())
            }
            Value::Array {
                dimensions,
                elements,
            } => {
                if args.len() != dimensions.len() + 1 {
                    return Err(Self::arity(
                        "setf aref",
                        &format!("{} indices", dimensions.len()),
                        indices.len(),
                    ));
                }
                let mut offset = 0_usize;
                for (axis, (dimension, index_value)) in dimensions.iter().zip(&indices).enumerate()
                {
                    let index = Self::setf_index(index_value.clone(), args[axis + 1].span)?;
                    if index >= *dimension {
                        return Err(Self::invalid(
                            "SETF index is out of bounds",
                            args[axis + 1].span,
                        ));
                    }
                    let stride = dimensions[axis + 1..]
                        .iter()
                        .try_fold(1_usize, |stride, dimension| stride.checked_mul(*dimension))
                        .ok_or_else(|| Self::invalid("SETF index is too large", place_span))?;
                    let contribution = index
                        .checked_mul(stride)
                        .ok_or_else(|| Self::invalid("SETF index is too large", place_span))?;
                    offset = offset
                        .checked_add(contribution)
                        .ok_or_else(|| Self::invalid("SETF index is too large", place_span))?;
                }
                let element_type = elements.element_type();
                if !crate::builtins::typep_value_in(&value, &element_type, environment)? {
                    return Err(RuntimeError::Type {
                        expected: element_type.to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(place_span),
                    });
                }
                if !elements.set(offset, value) {
                    return Err(Self::invalid("SETF index is out of bounds", place_span));
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
}
