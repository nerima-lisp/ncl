use super::{FunctionPass, Module, Sccp};
use ncl_ir::{BlockId, FunctionBuilder, FunctionId, Terminator, verify};

#[test]
fn removes_an_unreachable_return_block_without_a_predecessor() {
    let mut builder = FunctionBuilder::new(FunctionId(90), "unreachable-return", vec![], vec![]);
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .unwrap_or_else(|error| panic!("entry terminator failed: {error}"));
    let dead = builder.create_block(Vec::new());
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .unwrap_or_else(|error| panic!("dead terminator failed: {error}"));
    let mut function = builder.finish();

    assert_eq!(dead, BlockId(1));
    assert_eq!(function.blocks.len(), 2);
    let changed = Sccp
        .run(&mut function, &Module::default())
        .unwrap_or_else(|error| panic!("SCCP failed: {error}"));

    assert!(changed);
    assert_eq!(function.blocks.len(), 1);
    assert_eq!(function.blocks[0].id, BlockId(0));
    verify(&function).unwrap_or_else(|errors| panic!("optimized function is invalid: {errors:?}"));
}
