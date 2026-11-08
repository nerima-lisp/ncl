#![expect(
    missing_docs,
    reason = "TemplateKind is the closed backend dispatch vocabulary; its variants name the IR operations encoded by each target."
)]

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
