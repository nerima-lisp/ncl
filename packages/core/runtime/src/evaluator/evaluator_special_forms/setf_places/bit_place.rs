use super::{Environment, Form, Runtime, RuntimeError, Span, Value};

impl Runtime {
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
            Value::Vector(items) => vec![items.len()],
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
            Value::Vector(_) => {
                let mut elements = current
                    .vector_items()
                    .ok_or_else(|| Self::invalid("SETF target is not a vector", place_span))?;
                let Some(slot) = elements.get_mut(offset) else {
                    return Err(Self::invalid("SETF index is out of bounds", place_span));
                };
                *slot = value;
                self.set_place(&args[0], Value::vector(elements), environment)
            }
            Value::Array { .. } => {
                let mut elements = current
                    .array_items()
                    .ok_or_else(|| Self::invalid("SETF target is not an array", place_span))?;
                let Some(slot) = elements.get_mut(offset) else {
                    return Err(Self::invalid("SETF index is out of bounds", place_span));
                };
                *slot = value;
                let dimensions = current
                    .array_dimensions()
                    .ok_or_else(|| Self::invalid("SETF target is not an array", place_span))?;
                self.set_place(&args[0], Value::array(dimensions, elements), environment)
            }
            _ => unreachable!("bit array type checked above"),
        }
    }
}
