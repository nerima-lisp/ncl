use super::*;
use crate::{Aarch64Abi, CodegenError};
use ncl_ir::{BasicBlock, FunctionId, Param, Ty};

fn function(params: Vec<Param>, terminator: Terminator) -> Function {
    Function {
        id: FunctionId(1),
        name: "target-boundary-test".into(),
        params,
        return_types: Vec::new(),
        blocks: vec![BasicBlock {
            id: ncl_ir::BlockId(0),
            params: Vec::new(),
            ops: Vec::new(),
            terminator,
        }],
        locals: Vec::new(),
        constants: Vec::new(),
        handler_regions: Vec::new(),
        debug: Vec::new(),
    }
}

#[test]
fn aarch64_compile_rejects_empty_and_unknown_control_flow_targets() {
    let empty = Function {
        id: FunctionId(0),
        name: "empty".into(),
        params: Vec::new(),
        return_types: Vec::new(),
        blocks: Vec::new(),
        locals: Vec::new(),
        constants: Vec::new(),
        handler_regions: Vec::new(),
        debug: Vec::new(),
    };
    assert_eq!(
        compile_function_aarch64(&empty, &Aarch64Abi),
        Err(CodegenError::EmptyFunction)
    );

    let jump = function(
        Vec::new(),
        Terminator::Jump {
            target: ncl_ir::BlockId(9),
            args: Vec::new(),
        },
    );
    assert_eq!(
        compile_function_aarch64(&jump, &Aarch64Abi),
        Err(CodegenError::UnknownBlock(ncl_ir::BlockId(9)))
    );

    let branch = function(
        vec![Param {
            name: "condition".into(),
            ty: Ty::Word,
        }],
        Terminator::Branch {
            condition: ncl_ir::ValueId(0),
            then_target: ncl_ir::BlockId(9),
            then_args: Vec::new(),
            else_target: ncl_ir::BlockId(0),
            else_args: Vec::new(),
        },
    );
    assert_eq!(
        compile_function_aarch64(&branch, &Aarch64Abi),
        Err(CodegenError::UnknownBlock(ncl_ir::BlockId(9)))
    );
}
