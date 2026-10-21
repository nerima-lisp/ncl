use ncl_ir::{OpKind, Prim};

pub const fn is_call(kind: &OpKind) -> bool {
    matches!(
        kind,
        OpKind::Call { .. }
            | OpKind::CallIndirect { .. }
            | OpKind::CallClosure { .. }
            | OpKind::Builtin { .. }
            | OpKind::MakeClosure { .. }
            | OpKind::MakeValueCell { .. }
            // `EnterHandler`/`LeaveHandler` (catch/unwind-protect/progv)
            // call a native runtime function just like `Builtin`, so a value
            // crossing one is not guaranteed to survive in a register and
            // must be forced to a frame slot too.
            | OpKind::EnterHandler { .. }
            | OpKind::LeaveHandler { .. }
            | OpKind::StoreField { .. }
            | OpKind::Prim {
                op: Prim::Rplaca | Prim::Rplacd | Prim::Aset,
                ..
            }
    )
}

pub const fn is_safepoint(kind: &OpKind) -> bool {
    matches!(kind, OpKind::Safepoint | OpKind::Alloc { .. })
        || matches!(
            kind,
            OpKind::Call { .. }
                | OpKind::CallIndirect { .. }
                | OpKind::CallClosure { .. }
                | OpKind::Builtin { .. }
                | OpKind::MakeClosure { .. }
                | OpKind::MakeValueCell { .. }
                | OpKind::EnterHandler { .. }
                | OpKind::LeaveHandler { .. }
        )
}
