impl Runtime {
    fn rewrite_vector_contents(
        &self,
        template: &Value,
        items: Vec<Value>,
        fill_pointer: Option<Option<usize>>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match template {
            Value::Vector {
                elements,
                length,
                fill_pointer: current_fill_pointer,
                element_type,
                adjustable,
                displaced_to,
                displaced_index_offset,
            } => {
                let end = displaced_index_offset
                    .checked_add(*length)
                    .ok_or_else(|| self.invalid("vector bounds are invalid", span))?;
                let mut storage = elements.borrow_mut();
                if end > storage.len() {
                    return Err(self.invalid("vector bounds are invalid", span));
                }
                storage.splice(*displaced_index_offset..end, items.clone());
                let length = items.len();
                let fill_pointer = fill_pointer.unwrap_or(*current_fill_pointer);
                Ok(
                    Value::vector_with_storage_fill_pointer_element_type_adjustable_and_displacement(
                        elements.clone(),
                        length,
                        fill_pointer,
                        element_type.as_ref().clone(),
                        *adjustable,
                        displaced_to.as_ref().map(|value| value.as_ref().clone()),
                        *displaced_index_offset,
                    ),
                )
            }
            _ => unreachable!("validated vector template"),
        }
    }
}
