use crate::CodegenError;
use ncl_ir::{Function, OpKind, ValueId};

pub fn closure_capture_count(
    function: &Function,
    closure: ValueId,
) -> Result<Option<usize>, CodegenError> {
    let mut current = closure;
    let limit = function
        .blocks
        .iter()
        .map(|block| block.ops.len())
        .sum::<usize>();
    for _ in 0..limit {
        let definition = function
            .blocks
            .iter()
            .flat_map(|block| &block.ops)
            .find(|op| op.results.iter().any(|(value, _)| *value == current))
            .ok_or_else(|| CodegenError::Abi("closure value definition is unavailable".into()))?;
        match &definition.kind {
            OpKind::MakeClosure { captures, .. } => return Ok(Some(captures.len())),
            OpKind::Move { value } | OpKind::Convert { value, .. } => current = *value,
            // check-added-lines: allow(wildcard) non-closure definitions are not captures.
            _ => return Ok(None),
        }
    }
    Err(CodegenError::Abi(
        "closure value definition has a cycle".into(),
    ))
}
