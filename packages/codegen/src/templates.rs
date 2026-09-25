#![allow(dead_code, missing_docs)]

use ncl_ir::{OpKind, Terminator};

/// The fixed-template vocabulary consumed by a target backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemplateKind {
    Const,
    Move,
    Load,
    Store,
    LoadField,
    StoreField,
    Alloc,
    LoadArg,
    Call,
    CallIndirect,
    MakeClosure,
    CallClosure,
    Builtin,
    Prim,
    Compare,
    Convert,
    SetMultipleValues,
    Safepoint,
    EnterHandler,
    LeaveHandler,
    Jump,
    Branch,
    Switch,
    CallReturn,
    TailCall,
    Return,
    Throw,
    Unreachable,
}

/// One lowered operation. Operands remain in IR form until allocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Template {
    pub kind: TemplateKind,
    pub op: Option<OpKind>,
    pub terminator: Option<Terminator>,
}

impl Template {
    pub const fn op(kind: TemplateKind, op: OpKind) -> Self {
        Self {
            kind,
            op: Some(op),
            terminator: None,
        }
    }

    pub const fn terminator(kind: TemplateKind, terminator: Terminator) -> Self {
        Self {
            kind,
            op: None,
            terminator: Some(terminator),
        }
    }
}

pub const fn op_template(op: &OpKind) -> TemplateKind {
    match op {
        OpKind::Const { .. } => TemplateKind::Const,
        OpKind::Move { .. } => TemplateKind::Move,
        OpKind::Load { .. } => TemplateKind::Load,
        OpKind::Store { .. } => TemplateKind::Store,
        OpKind::LoadField { .. } => TemplateKind::LoadField,
        OpKind::StoreField { .. } => TemplateKind::StoreField,
        OpKind::Alloc { .. } => TemplateKind::Alloc,
        OpKind::LoadArg { .. } => TemplateKind::LoadArg,
        OpKind::Call { .. } => TemplateKind::Call,
        OpKind::CallIndirect { .. } => TemplateKind::CallIndirect,
        OpKind::MakeClosure { .. } => TemplateKind::MakeClosure,
        OpKind::CallClosure { .. } => TemplateKind::CallClosure,
        OpKind::Builtin { .. } => TemplateKind::Builtin,
        OpKind::Prim { .. } => TemplateKind::Prim,
        OpKind::Compare { .. } => TemplateKind::Compare,
        OpKind::Convert { .. } => TemplateKind::Convert,
        OpKind::SetMultipleValues { .. } => TemplateKind::SetMultipleValues,
        OpKind::Safepoint => TemplateKind::Safepoint,
        OpKind::EnterHandler { .. } => TemplateKind::EnterHandler,
        OpKind::LeaveHandler { .. } => TemplateKind::LeaveHandler,
    }
}

pub const fn terminator_template(terminator: &Terminator) -> TemplateKind {
    match terminator {
        Terminator::Jump { .. } => TemplateKind::Jump,
        Terminator::Branch { .. } => TemplateKind::Branch,
        Terminator::Switch { .. } => TemplateKind::Switch,
        Terminator::CallReturn { .. } => TemplateKind::CallReturn,
        Terminator::TailCall { .. } => TemplateKind::TailCall,
        Terminator::Return { .. } => TemplateKind::Return,
        Terminator::Throw { .. } => TemplateKind::Throw,
        Terminator::Unreachable => TemplateKind::Unreachable,
    }
}
