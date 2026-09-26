use super::*;

#[test]
fn golden_aarch64_add_one_two_decodes() {
    assert_aarch64_fixture(&constant_return(20, "aarch64-add", 3));
}

#[test]
fn golden_aarch64_cons_decodes() {
    let mut builder =
        FunctionBuilder::new(ncl_ir::FunctionId(21), "aarch64-cons", Vec::new(), vec![]);
    assert!(
        builder
            .push_op(OpKind::Alloc { words: 2 }, &[Ty::Word])
            .is_ok()
    );
    assert!(
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .is_ok()
    );
    assert_aarch64_fixture(&builder.finish());
}

#[test]
fn golden_aarch64_branch_decodes() {
    let mut builder =
        FunctionBuilder::new(ncl_ir::FunctionId(22), "aarch64-branch", Vec::new(), vec![]);
    let condition = builder.add_constant(Constant::T);
    let values = builder.push_op(OpKind::Const { result: condition }, &[Ty::Word]);
    assert!(values.is_ok());
    let condition = values.map_or(ncl_ir::ValueId(0), |ids| ids[0]);
    let then_target = builder.create_block(Vec::new());
    let else_target = builder.create_block(Vec::new());
    assert!(builder.position_at(ncl_ir::BlockId(0)).is_ok());
    assert!(
        builder
            .terminate(Terminator::Branch {
                condition,
                then_target,
                else_target,
                then_args: Vec::new(),
                else_args: Vec::new()
            })
            .is_ok()
    );
    assert!(builder.position_at(then_target).is_ok());
    assert!(
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .is_ok()
    );
    assert!(builder.position_at(else_target).is_ok());
    assert!(
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .is_ok()
    );
    assert_aarch64_fixture(&builder.finish());
}

#[test]
fn golden_aarch64_builtin_decodes() {
    struct Abi;
    impl RuntimeAbi for Abi {
        fn builtin_address(&self, name: &str) -> Option<u64> {
            (name == "identity").then_some(0x1000)
        }

        fn context_offset(&self, _field: &str) -> Option<i32> {
            None
        }
    }
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(23),
        "aarch64-builtin",
        Vec::new(),
        vec![],
    );
    assert!(
        builder
            .push_op(
                OpKind::Builtin {
                    name: "identity".into(),
                    args: Vec::new()
                },
                &[]
            )
            .is_ok()
    );
    assert!(
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .is_ok()
    );
    assert_aarch64_fixture_with_abi(&builder.finish(), &Abi);
}

#[test]
fn golden_aarch64_safepoint_decodes() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(24),
        "aarch64-safepoint",
        Vec::new(),
        vec![],
    );
    assert!(builder.push_op(OpKind::Safepoint, &[]).is_ok());
    assert!(
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .is_ok()
    );
    assert_aarch64_fixture(&builder.finish());
}

#[test]
fn golden_aarch64_fib_loop_decodes() {
    let mut builder =
        FunctionBuilder::new(ncl_ir::FunctionId(25), "aarch64-fib-25", Vec::new(), vec![]);
    assert!(
        builder
            .terminate(Terminator::Jump {
                target: ncl_ir::BlockId(0),
                args: Vec::new()
            })
            .is_ok()
    );
    assert_aarch64_fixture(&builder.finish());
}
