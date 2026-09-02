#[allow(clippy::wildcard_imports)]
use super::*;

pub fn execute_array_element_instruction(
    stack: &mut Vec<Value>, operation: &str, argument_count: usize, span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("array-element has too few stack values", span));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    let value = match operation {
        "AREF" => crate::builtins::aref(&arguments)?,
        "SVREF" => crate::builtins::svref(&arguments)?,
        "BIT" => crate::builtins::bit(&arguments)?,
        "ROW-MAJOR-AREF" => crate::builtins::row_major_aref(&arguments)?,
        "ARRAY-ROW-MAJOR-INDEX" => crate::builtins::array_row_major_index(&arguments)?,
        "ARRAY-IN-BOUNDS-P" => crate::builtins::array_in_bounds_p(&arguments)?,
        _ => return Err(invalid("unknown array-element operation", span)),
    };
    stack.push(value);
    Ok(())
}

pub fn execute_array_metadata_instruction(
    stack: &mut Vec<Value>, operation: &str, argument_count: usize, span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("array-metadata has too few stack values", span));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    let value = match operation {
        "ARRAY-ELEMENT-TYPE" => crate::builtins::array_element_type(&arguments)?,
        "ADJUSTABLE-ARRAY-P" => crate::builtins::adjustable_array_p(&arguments)?,
        "ARRAY-DISPLACEMENT" => crate::builtins::array_displacement(&arguments)?,
        "ARRAY-HAS-FILL-POINTER-P" => crate::builtins::array_has_fill_pointer_p(&arguments)?,
        "ARRAY-RANK" => crate::builtins::array_rank(&arguments)?,
        "ARRAY-DIMENSIONS" => crate::builtins::array_dimensions(&arguments)?,
        "ARRAY-DIMENSION" => crate::builtins::array_dimension(&arguments)?,
        "ARRAY-TOTAL-SIZE" => crate::builtins::array_total_size(&arguments)?,
        _ => return Err(invalid("unknown array-metadata operation", span)),
    };
    stack.push(value);
    Ok(())
}
