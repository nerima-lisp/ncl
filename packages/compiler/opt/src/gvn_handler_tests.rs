#![allow(clippy::unwrap_used)]

use super::{Fixture, GlobalValueNumbering, Module};
use crate::FunctionPass;
use ncl_ir::{
    BlockId, FunctionBuilder, FunctionId, HandlerKind, HandlerRegion, HandlerRegionId, OpKind,
    Param, Prim, Terminator, Ty, ValueId,
};

fn handler_join_with_duplicate_expression() -> ncl_ir::Function {
    let mut builder = FunctionBuilder::new(
        FunctionId(4),
        "handler-join",
        vec![
            Param {
                name: "x".into(),
                ty: Ty::I64,
            },
            Param {
                name: "y".into(),
                ty: Ty::I64,
            },
            Param {
                name: "tag".into(),
                ty: Ty::Word,
            },
            Param {
                name: "condition".into(),
                ty: Ty::Bool,
            },
        ],
        vec![Ty::I64],
    );
    builder
        .push_op(
            OpKind::EnterHandler {
                region: HandlerRegionId(0),
            },
            &[],
        )
        .fixture();
    builder
        .terminate(Terminator::Branch {
            condition: ValueId(3),
            then_target: BlockId(1),
            then_args: Vec::new(),
            else_target: BlockId(2),
            else_args: Vec::new(),
        })
        .fixture();

    let then_block = builder.create_block(Vec::new());
    builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![ValueId(0), ValueId(1)],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture();
    builder
        .terminate(Terminator::Jump {
            target: BlockId(3),
            args: vec![ValueId(2)],
        })
        .fixture();

    let throw_block = builder.create_block(Vec::new());
    builder
        .terminate(Terminator::Throw {
            condition: ValueId(2),
        })
        .fixture();

    let handler = builder.create_block(vec![(Ty::Word, ValueId(5))]);
    builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![ValueId(0), ValueId(1)],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture();
    builder
        .push_op(
            OpKind::LeaveHandler {
                region: HandlerRegionId(0),
            },
            &[],
        )
        .fixture();
    builder
        .terminate(Terminator::Return {
            values: vec![ValueId(6)],
        })
        .fixture();
    builder.add_handler_region(HandlerRegion {
        id: HandlerRegionId(0),
        kind: HandlerKind::Catch,
        protected: vec![BlockId(0), then_block, throw_block],
        handler,
        cleanup: None,
        catch_tag: Some(ValueId(2)),
        binding_targets: Vec::new(),
        depth: 0,
        parent: None,
    });
    builder.finish()
}

#[test]
fn gvn_includes_handler_edges_when_reusing_values() {
    let mut function = handler_join_with_duplicate_expression();
    assert!(ncl_ir::verify(&function).is_ok());

    let mut pass = GlobalValueNumbering;
    assert!(
        pass.run(&mut function, &Module::default()).is_ok(),
        "GVN must preserve valid handler-region IR"
    );
    assert!(ncl_ir::verify(&function).is_ok());
    assert!(matches!(
        function.blocks[3].ops.first().map(|op| &op.kind),
        Some(OpKind::Prim {
            op: Prim::FixnumAdd,
            ..
        })
    ));
}
