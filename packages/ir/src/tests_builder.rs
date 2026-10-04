#![allow(clippy::expect_used)]

use super::*;

#[test]
fn builder_allocates_ids_and_edits_selected_blocks() {
    let mut builder = FunctionBuilder::new(FunctionId(3), "builder", Vec::new(), vec![Ty::Word]);
    let local = builder.add_local("slot", Ty::Address);
    let constant = builder.add_constant(Constant::T);
    assert_eq!(local, LocalId(0));
    assert_eq!(constant, ConstantIndex(0));

    let first = builder.fresh_value();
    let block = builder.create_block(vec![(Ty::I64, ValueId(9))]);
    assert_eq!(first, ValueId(0));
    assert_eq!(block, BlockId(1));
    assert_eq!(builder.fresh_value(), ValueId(10));

    let result = builder
        .push_op(OpKind::Const { result: constant }, &[Ty::Word])
        .expect("the newly created block is selected");
    assert_eq!(result, vec![ValueId(11)]);
    builder
        .position_at(BlockId(0))
        .expect("the entry block exists");
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .expect("the entry block is selected");
    builder.add_handler_region(HandlerRegion {
        id: HandlerRegionId(0),
        kind: HandlerKind::Catch,
        protected: vec![BlockId(0)],
        handler: BlockId(1),
        cleanup: None,
        catch_tag: None,
        binding_targets: Vec::new(),
        depth: 0,
        parent: None,
    });

    let function = builder.finish();
    assert_eq!(function.name, "builder");
    assert_eq!(function.locals[0].name, "slot");
    assert_eq!(function.constants, vec![Constant::T]);
    assert_eq!(function.blocks[1].params[0].value, ValueId(9));
    assert_eq!(
        function.blocks[1].ops[0].results,
        vec![(ValueId(11), Ty::Word)]
    );
    assert_eq!(function.handler_regions.len(), 1);
    assert_eq!(function.handler_regions[0].handler, BlockId(1));
}

#[test]
fn builder_reports_unknown_block() {
    let mut builder = FunctionBuilder::new(FunctionId(0), "builder", Vec::new(), Vec::new());
    let error = builder
        .position_at(BlockId(99))
        .expect_err("an unknown block must be rejected");
    assert_eq!(error, "unknown block BlockId(99)");
}
