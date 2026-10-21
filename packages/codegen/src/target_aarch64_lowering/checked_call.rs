use std::collections::HashMap;

use ncl_asm_aarch64::{Assembler, Inst, Label, Reg};
use ncl_ir::{BlockId, Function};

use super::{Allocation, CodegenError, RuntimeAbi, dispatch};
use crate::machine_verify::SafepointSite;

pub fn emit_safepoint_call(assembler: &mut Assembler) -> Result<SafepointSite, CodegenError> {
    let start_offset = crate::checked_u32(assembler.offset())?;
    super::emit(assembler, Inst::Blr { rn: Reg(17) })?;
    let end_offset = crate::checked_u32(assembler.offset())?;
    Ok(SafepointSite::call(start_offset, end_offset))
}

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
    site: SafepointSite,
}

impl CheckedCall {
    pub const fn site(self) -> SafepointSite {
        self.site
    }
}

pub fn lower_checked_call(
    assembler: &mut Assembler,
    context: &CallCheckContext<'_>,
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
    mode: CheckedCallMode,
    emit_call: impl FnOnce(&mut Assembler) -> Result<SafepointSite, CodegenError>,
) -> Result<CheckedCall, CodegenError> {
    let site = emit_call(assembler)?;
    dispatch::lower_checked_call_check(assembler, context, allocation, abi, mode)?;
    Ok(CheckedCall { site })
}
