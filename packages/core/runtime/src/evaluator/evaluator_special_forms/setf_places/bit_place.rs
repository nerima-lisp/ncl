use super::{Environment, Form, FormKind, Runtime, RuntimeError, Span, Value};

impl Runtime {
    pub(crate) fn set_bit_place_value(
        &self,
        place: &Form,
        indices: &[Value],
        current: &Value,
        value: Value,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        let dimensions = match current {
            Value::Vector(items) => vec![items.len()],
            Value::Array { dimensions, .. } => dimensions.as_ref().clone(),
            other => {
                return Err(RuntimeError::Type {
                    expected: "ARRAY".to_string(),
                    actual: other.type_name().to_string(),
                    span: Some(place.span),
                });
            }
        };
        if indices.len() != dimensions.len() {
            return Err(Self::arity(
                "setf bit",
                &format!("{} subscripts", dimensions.len()),
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
                .try_fold(1_usize, |stride, dimension| stride.checked_mul(*dimension))
                .ok_or_else(|| Self::invalid("SETF index is too large", place.span))?;
            offset = offset
                .checked_add(
                    index
                        .checked_mul(stride)
                        .ok_or_else(|| Self::invalid("SETF index is too large", place.span))?,
                )
                .ok_or_else(|| Self::invalid("SETF index is too large", place.span))?;
        }
        if !matches!(&value, Value::Integer(bit) if *bit == 0 || *bit == 1) {
            return Err(RuntimeError::Type {
                expected: "BIT".to_string(),
                actual: value.type_name().to_string(),
                span: Some(place.span),
            });
        }
        let FormKind::List(items) = &place.kind else {
            return Err(Self::invalid("SETF BIT place is malformed", place.span));
        };
        let updated = match current {
            Value::Vector(_) => {
                let mut elements = current
                    .vector_items()
                    .ok_or_else(|| Self::invalid("SETF target is not a vector", place.span))?;
                let Some(slot) = elements.get_mut(offset) else {
                    return Err(Self::invalid("SETF index is out of bounds", place.span));
                };
                *slot = value;
                Value::vector(elements)
            }
            Value::Array { .. } => {
                let mut elements = current
                    .array_items()
                    .ok_or_else(|| Self::invalid("SETF target is not an array", place.span))?;
                let Some(slot) = elements.get_mut(offset) else {
                    return Err(Self::invalid("SETF index is out of bounds", place.span));
                };
                *slot = value;
                let dimensions = current
                    .array_dimensions()
                    .ok_or_else(|| Self::invalid("SETF target is not an array", place.span))?;
                Value::array(dimensions, elements)
            }
            _ => unreachable!("bit array type checked above"),
        };
        self.set_place(&items[1], updated, environment)
    }

    pub(super) fn set_bit_place(
        &self,
        args: &[Form],
        value: Value,
        environment: &Environment,
        place_span: Span,
    ) -> Result<(), RuntimeError> {
        if args.is_empty() {
            return Err(Self::arity("setf bit", "array and subscripts", 0));
        }
        let current = self.eval_in(&args[0], environment)?;
        let dimensions = match &current {
            Value::Vector(items) => vec![items.snapshot().len()],
            Value::Array { dimensions, .. } => dimensions.as_ref().clone(),
            other => {
                return Err(RuntimeError::Type {
                    expected: "ARRAY".to_string(),
                    actual: other.type_name().to_string(),
                    span: Some(args[0].span),
                });
            }
        };
        if args.len() != dimensions.len() + 1 {
            return Err(Self::arity(
                "setf bit",
                &format!("{} subscripts", dimensions.len()),
                args.len() - 1,
            ));
        }
        let indices = args[1..]
            .iter()
            .map(|argument| self.eval_in(argument, environment))
            .collect::<Result<Vec<_>, _>>()?;
        let mut offset = 0_usize;
        for (axis, (dimension, index_value)) in dimensions.iter().zip(&indices).enumerate() {
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
        if !matches!(&value, Value::Integer(bit) if *bit == 0 || *bit == 1) {
            return Err(RuntimeError::Type {
                expected: "BIT".to_string(),
                actual: value.type_name().to_string(),
                span: Some(place_span),
            });
        }
        match &current {
            Value::Vector(elements) | Value::Array { elements, .. } => {
                if !elements.set(offset, value) {
                    return Err(Self::invalid("SETF index is out of bounds", place_span));
                }
                Ok(())
            }
            _ => unreachable!("bit array type checked above"),
        }
    }
}
