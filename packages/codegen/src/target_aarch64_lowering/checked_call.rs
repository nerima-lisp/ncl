use std::collections::HashMap;

use ncl_asm_aarch64::{Assembler, Label};
use ncl_ir::{BlockId, Function};

use super::{Allocation, CodegenError, RuntimeAbi, dispatch};

pub struct CallCheckContext<'a> {
    pub(crate) function: &'a Function,
    pub(crate) block: BlockId,
    pub(crate) body_bytes: u32,
    pub(crate) labels: &'a HashMap<BlockId, Label>,
}

#[derive(Clone, Copy)]
pub enum CheckedCallMode {
    RuntimeAndNonLocal,
    RuntimeOnly,
}

pub struct CheckedCall {
    return_pc: u32,
}

impl CheckedCall {
    pub const fn return_pc(self) -> u32 {
        self.return_pc
    }
}

pub fn lower_checked_call(
    assembler: &mut Assembler,
    context: &CallCheckContext<'_>,
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
    mode: CheckedCallMode,
    emit_call: impl FnOnce(&mut Assembler) -> Result<(), CodegenError>,
) -> Result<CheckedCall, CodegenError> {
    emit_call(assembler)?;
    let return_pc = u32::try_from(assembler.offset()).map_err(|_| CodegenError::FrameOverflow)?;
    dispatch::lower_checked_call_check(assembler, context, allocation, abi, mode)?;
    Ok(CheckedCall { return_pc })
}
