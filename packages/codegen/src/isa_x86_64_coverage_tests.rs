use super::{epilogue, prologue, skeleton};
use crate::TemplateKind;
use ncl_asm_x86_64::Inst;

#[test]
fn x86_64_instruction_skeleton_covers_every_template_kind() {
    let kinds = [
        TemplateKind::Const,
        TemplateKind::Move,
        TemplateKind::Load,
        TemplateKind::Store,
        TemplateKind::LoadField,
        TemplateKind::StoreField,
        TemplateKind::Alloc,
        TemplateKind::LoadArg,
        TemplateKind::LoadCapture,
        TemplateKind::Call,
        TemplateKind::CallIndirect,
        TemplateKind::MakeClosure,
        TemplateKind::MakeValueCell,
        TemplateKind::CallClosure,
        TemplateKind::Builtin,
        TemplateKind::Prim,
        TemplateKind::Compare,
        TemplateKind::Convert,
        TemplateKind::SetMultipleValues,
        TemplateKind::Safepoint,
        TemplateKind::EnterHandler,
        TemplateKind::LeaveHandler,
        TemplateKind::Jump,
        TemplateKind::Branch,
        TemplateKind::Switch,
        TemplateKind::CallReturn,
        TemplateKind::TailCall,
        TemplateKind::Return,
        TemplateKind::Throw,
        TemplateKind::Unreachable,
    ];
    for kind in kinds {
        assert_eq!(skeleton(kind), vec![Inst::Nop(1)]); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    }
}

#[test]
fn x86_64_prologue_and_epilogue_have_the_fixed_frame_shape() {
    assert_eq!(
        // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        prologue(),
        vec![
            Inst::Push(ncl_asm_x86_64::Reg::Rbp),
            Inst::MovRR(ncl_asm_x86_64::Reg::Rbp, ncl_asm_x86_64::Reg::Rsp,)
        ]
    );
    assert_eq!(
        // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        epilogue(),
        vec![
            Inst::MovRR(ncl_asm_x86_64::Reg::Rsp, ncl_asm_x86_64::Reg::Rbp),
            Inst::Pop(ncl_asm_x86_64::Reg::Rbp),
            Inst::Ret,
        ]
    );
}
