#![allow(dead_code)]

use ncl_asm_x86_64::{Inst, Reg};

use crate::templates::TemplateKind;

pub const THREAD_CONTEXT: Reg = Reg::R15;
pub const SCRATCH: [Reg; 2] = [Reg::R10, Reg::R11];

/// Fixed x86-64 instruction skeleton for one template.
/// Real operands and fixups are filled after register allocation.
pub fn skeleton(kind: TemplateKind) -> Vec<Inst> {
    match kind {
        TemplateKind::Const
        | TemplateKind::Move
        | TemplateKind::Load
        | TemplateKind::Store
        | TemplateKind::LoadField
        | TemplateKind::StoreField
        | TemplateKind::Alloc
        | TemplateKind::LoadArg
        | TemplateKind::Call
        | TemplateKind::CallIndirect
        | TemplateKind::MakeClosure
        | TemplateKind::CallClosure
        | TemplateKind::Builtin
        | TemplateKind::Prim
        | TemplateKind::Compare
        | TemplateKind::Convert
        | TemplateKind::SetMultipleValues
        | TemplateKind::Safepoint
        | TemplateKind::EnterHandler
        | TemplateKind::LeaveHandler
        | TemplateKind::Jump
        | TemplateKind::Branch
        | TemplateKind::Switch
        | TemplateKind::CallReturn
        | TemplateKind::TailCall
        | TemplateKind::Return
        | TemplateKind::Throw
        | TemplateKind::Unreachable => vec![Inst::Nop(1)],
    }
}

pub fn prologue() -> Vec<Inst> {
    vec![Inst::Push(Reg::Rbp), Inst::MovRR(Reg::Rbp, Reg::Rsp)]
}

pub fn epilogue() -> Vec<Inst> {
    vec![
        Inst::MovRR(Reg::Rsp, Reg::Rbp),
        Inst::Pop(Reg::Rbp),
        Inst::Ret,
    ]
}
