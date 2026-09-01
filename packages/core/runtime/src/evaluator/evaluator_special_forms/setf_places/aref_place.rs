use super::{Environment, Form, Runtime, RuntimeError, Span, Value};

impl Runtime {
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
            Value::Vector(_) => {
                if indices.len() != 1 {
                    return Err(Self::arity("setf aref", "two", args.len()));
                }
                let index = Self::setf_index(indices[0].clone(), args[1].span)?;
                let Some(()) = current.set_vector_item(index, value.clone()) else {
                    return Err(Self::invalid("SETF index is out of bounds", args[1].span));
                };
                self.set_place(&args[0], current, environment)
            }
            Value::Array { dimensions, .. } => {
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
                let Some(()) = current.set_array_item(offset, value) else {
                    return Err(Self::invalid("SETF index is out of bounds", place_span));
                };
                self.set_place(
                    &args[0],
                    current,
                    environment,
                )
            }
            other => Err(RuntimeError::Type {
                expected: "ARRAY or VECTOR".to_string(),
                actual: other.type_name().to_string(),
                span: Some(args[0].span),
            }),
        }
    }
}
