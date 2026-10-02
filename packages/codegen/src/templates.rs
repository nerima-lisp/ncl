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
    LoadCapture,
    Call,
    CallIndirect,
    MakeClosure,
    MakeValueCell,
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
        OpKind::LoadCapture { .. } | OpKind::LoadFunctionObject => TemplateKind::LoadCapture,
        OpKind::Call { .. } => TemplateKind::Call,
        OpKind::CallIndirect { .. } => TemplateKind::CallIndirect,
        OpKind::MakeClosure { .. } => TemplateKind::MakeClosure,
        OpKind::MakeValueCell { .. } => TemplateKind::MakeValueCell,
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

#[cfg(test)]
#[allow(clippy::too_many_lines, missing_docs)]
mod tests {
    use super::{Template, TemplateKind, op_template, terminator_template};
    use ncl_ir::{BlockId, ConstantIndex, OpKind, Terminator, ValueId};

    #[test]
    fn maps_every_ir_operation_to_its_fixed_template() {
        let value = ValueId(0);
        let operation_cases = [
            (
                OpKind::Const {
                    result: ConstantIndex(0),
                },
                TemplateKind::Const,
            ),
            (OpKind::Move { value }, TemplateKind::Move),
            (OpKind::Load { address: value }, TemplateKind::Load),
            (
                OpKind::Store {
                    address: value,
                    value,
                },
                TemplateKind::Store,
            ),
            (
                OpKind::LoadField {
                    object: value,
                    field: 0,
                },
                TemplateKind::LoadField,
            ),
            (
                OpKind::StoreField {
                    object: value,
                    field: 0,
                    value,
                },
                TemplateKind::StoreField,
            ),
            (OpKind::Alloc { words: 1 }, TemplateKind::Alloc),
            (OpKind::LoadArg { index: 0 }, TemplateKind::LoadArg),
            (OpKind::LoadCapture { index: 0 }, TemplateKind::LoadCapture),
            (OpKind::LoadFunctionObject, TemplateKind::LoadCapture),
            (
                OpKind::Call {
                    function: value,
                    args: Vec::new(),
                },
                TemplateKind::Call,
            ),
            (
                OpKind::CallIndirect {
                    callee: value,
                    args: Vec::new(),
                },
                TemplateKind::CallIndirect,
            ),
            (
                OpKind::MakeClosure {
                    entry: value,
                    captures: Vec::new(),
                },
                TemplateKind::MakeClosure,
            ),
            (OpKind::MakeValueCell { value }, TemplateKind::MakeValueCell),
            (
                OpKind::CallClosure {
                    closure: value,
                    args: Vec::new(),
                    named_symbol: None,
                },
                TemplateKind::CallClosure,
            ),
            (
                OpKind::Builtin {
                    name: "identity".into(),
                    args: Vec::new(),
                },
                TemplateKind::Builtin,
            ),
            (
                OpKind::Prim {
                    op: ncl_ir::Prim::FixnumAdd,
                    args: vec![value],
                    condition: None,
                },
                TemplateKind::Prim,
            ),
            (
                OpKind::Compare {
                    op: ncl_ir::Compare::Eq,
                    left: value,
                    right: value,
                },
                TemplateKind::Compare,
            ),
            (
                OpKind::Convert {
                    op: ncl_ir::Convert::WordToI64,
                    value,
                },
                TemplateKind::Convert,
            ),
            (
                OpKind::SetMultipleValues {
                    values: vec![value],
                },
                TemplateKind::SetMultipleValues,
            ),
            (OpKind::Safepoint, TemplateKind::Safepoint),
            (
                OpKind::EnterHandler {
                    region: ncl_ir::HandlerRegionId(0),
                },
                TemplateKind::EnterHandler,
            ),
            (
                OpKind::LeaveHandler {
                    region: ncl_ir::HandlerRegionId(0),
                },
                TemplateKind::LeaveHandler,
            ),
        ];

        for (operation, expected) in operation_cases {
            assert_eq!(op_template(&operation), expected, "{operation:?}");
        }
    }

    #[test]
    fn maps_every_ir_terminator_and_keeps_template_payloads_separate() {
        let value = ValueId(0);
        let terminator_cases = [
            (
                Terminator::Jump {
                    target: BlockId(1),
                    args: vec![value],
                },
                TemplateKind::Jump,
            ),
            (
                Terminator::Branch {
                    condition: value,
                    then_target: BlockId(1),
                    then_args: Vec::new(),
                    else_target: BlockId(2),
                    else_args: Vec::new(),
                },
                TemplateKind::Branch,
            ),
            (
                Terminator::Switch {
                    value,
                    cases: vec![(1, BlockId(1), Vec::new())],
                    default: BlockId(2),
                    default_args: Vec::new(),
                },
                TemplateKind::Switch,
            ),
            (
                Terminator::CallReturn {
                    function: value,
                    args: Vec::new(),
                },
                TemplateKind::CallReturn,
            ),
            (
                Terminator::TailCall {
                    function: value,
                    args: Vec::new(),
                },
                TemplateKind::TailCall,
            ),
            (
                Terminator::Return {
                    values: vec![value],
                },
                TemplateKind::Return,
            ),
            (Terminator::Throw { condition: value }, TemplateKind::Throw),
            (Terminator::Unreachable, TemplateKind::Unreachable),
        ];

        for (terminator, expected) in terminator_cases {
            assert_eq!(terminator_template(&terminator), expected, "{terminator:?}");
        }

        let operation = OpKind::Move { value };
        let template = Template::op(TemplateKind::Move, operation.clone());
        assert_eq!(template.kind, TemplateKind::Move);
        assert_eq!(template.op, Some(operation));
        assert_eq!(template.terminator, None);

        let terminator = Terminator::Return { values: Vec::new() };
        let template = Template::terminator(TemplateKind::Return, terminator.clone());
        assert_eq!(template.kind, TemplateKind::Return);
        assert_eq!(template.op, None);
        assert_eq!(template.terminator, Some(terminator));
    }
}
