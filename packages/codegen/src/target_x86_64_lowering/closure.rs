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

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]
mod tests {
    use super::closure_capture_count;
    use crate::CodegenError;
    use ncl_ir::{
        BasicBlock, Constant, Function, FunctionBuilder, Op, OpKind, Terminator, Ty, ValueId,
    };

    #[test]
    fn follows_moves_and_distinguishes_non_closures_from_real_closures() -> Result<(), String> {
        let mut builder = FunctionBuilder::new(
            ncl_ir::FunctionId(220),
            "closure-definitions",
            Vec::new(),
            Vec::new(),
        );
        let entry = builder.add_constant(Constant::FunctionEntry(ncl_ir::FunctionId(1)));
        let entry = builder
            .push_op(OpKind::Const { result: entry }, &[Ty::Word])?
            .into_iter()
            .next()
            .ok_or_else(|| "entry result missing".to_owned())?;
        let closure = builder
            .push_op(
                OpKind::MakeClosure {
                    entry,
                    captures: vec![entry, entry],
                },
                &[Ty::Word],
            )?
            .into_iter()
            .next()
            .ok_or_else(|| "closure result missing".to_owned())?;
        let moved = builder
            .push_op(OpKind::Move { value: closure }, &[Ty::Word])?
            .into_iter()
            .next()
            .ok_or_else(|| "moved closure result missing".to_owned())?;
        let function = builder.finish();
        assert_eq!(closure_capture_count(&function, moved), Ok(Some(2)));

        let mut non_closure_builder = FunctionBuilder::new(
            ncl_ir::FunctionId(221),
            "non-closure-definition",
            Vec::new(),
            Vec::new(),
        );
        let constant = non_closure_builder.add_constant(Constant::Fixnum(1));
        let value = non_closure_builder
            .push_op(OpKind::Const { result: constant }, &[Ty::Word])?
            .into_iter()
            .next()
            .ok_or_else(|| "constant result missing".to_owned())?;
        assert_eq!(
            closure_capture_count(&non_closure_builder.finish(), value),
            Ok(None)
        );
        Ok(())
    }

    #[test]
    fn reports_missing_and_cyclic_closure_definitions_as_abi_errors() {
        let mut missing_builder = FunctionBuilder::new(
            ncl_ir::FunctionId(222),
            "missing-closure-definition",
            Vec::new(),
            Vec::new(),
        );
        let unrelated = missing_builder.add_constant(Constant::Fixnum(0));
        missing_builder
            .push_op(OpKind::Const { result: unrelated }, &[Ty::Word])
            .expect("unrelated definition");
        let missing = missing_builder.finish();
        assert!(matches!(
            closure_capture_count(&missing, ValueId(99)),
            Err(CodegenError::Abi(message))
                if message.contains("definition is unavailable")
        ));

        let cyclic = Function {
            id: ncl_ir::FunctionId(223),
            name: "cyclic-closure-definition".into(),
            params: Vec::new(),
            return_types: Vec::new(),
            blocks: vec![BasicBlock {
                id: ncl_ir::BlockId(0),
                params: Vec::new(),
                ops: vec![
                    Op {
                        results: vec![(ValueId(1), Ty::Word)],
                        kind: OpKind::Move { value: ValueId(0) },
                        loc: None,
                    },
                    Op {
                        results: vec![(ValueId(0), Ty::Word)],
                        kind: OpKind::Move { value: ValueId(1) },
                        loc: None,
                    },
                ],
                terminator: Terminator::Unreachable,
            }],
            locals: Vec::new(),
            constants: Vec::new(),
            handler_regions: Vec::new(),
            debug: Vec::new(),
        };
        assert!(matches!(
            closure_capture_count(&cyclic, ValueId(0)),
            Err(CodegenError::Abi(message))
                if message.contains("definition has a cycle")
        ));
    }
}
