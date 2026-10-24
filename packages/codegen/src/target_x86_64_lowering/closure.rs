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
            OpKind::Move { value }
            | OpKind::Convert { value, .. }
            | OpKind::TypeHint { value, .. } => current = *value,
            // check-added-lines: allow(wildcard) non-closure definitions are not captures.
            _ => return Ok(None),
        }
    }
    Err(CodegenError::Abi(
        "closure value definition has a cycle".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::closure_capture_count;
    use ncl_ir::{Constant, FunctionBuilder, OpKind, ScalarTypeHint, Terminator, Ty};

    #[test]
    fn type_hint_preserves_closure_capture_count() -> Result<(), String> {
        let mut builder = FunctionBuilder::new(
            ncl_ir::FunctionId(0),
            "type-hinted-closure",
            Vec::new(),
            vec![Ty::Word],
        );
        let entry_constant = builder.add_constant(Constant::T);
        let entry = builder.push_op(
            OpKind::Const {
                result: entry_constant,
            },
            &[Ty::Word],
        )?[0];
        let closure = builder.push_op(
            OpKind::MakeClosure {
                entry,
                captures: vec![entry; 3],
            },
            &[Ty::Word],
        )?[0];
        let hinted = builder.push_op(
            OpKind::TypeHint {
                value: closure,
                ty: ScalarTypeHint::NonNumber,
            },
            &[Ty::Word],
        )?[0];
        builder.terminate(Terminator::Return {
            values: vec![hinted],
        })?;

        assert_eq!(
            closure_capture_count(&builder.finish(), hinted).map_err(|error| error.to_string())?,
            Some(3)
        );
        Ok(())
    }
}
