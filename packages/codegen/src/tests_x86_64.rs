#![allow(missing_docs, clippy::unwrap_used)]

use crate::tests_x86_64_fixture::X86_64FixtureAbi;
use crate::{AllocationTarget, allocate, compile_function_x86_64};
use ncl_ir::{Constant, FunctionBuilder, OpKind, Terminator, Ty};

#[test]
fn x86_64_lowering_uses_allocator_register_roots() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(71),
        "allocated-root",
        Vec::new(),
        vec![Ty::Word],
    );
    let constant = builder.add_constant(Constant::Fixnum(9));
    let value = match builder.push_op(OpKind::Const { result: constant }, &[Ty::Word]) {
        Ok(ids) => ids[0],
        Err(error) => unreachable!("constant: {error:?}"),
    };
    assert!(builder.push_op(OpKind::Safepoint, &[]).is_ok(), "safepoint");
    assert!(
        builder
            .terminate(Terminator::Return {
                values: vec![value],
            })
            .is_ok(),
        "return"
    );
    let function = builder.finish();
    let allocation = allocate(&function, AllocationTarget::X86_64);
    let Some(expected) = allocation.safepoint_registers.get(&1) else {
        unreachable!("allocator safepoint roots");
    };
    assert!(!expected.is_empty());
    let compiled = match compile_function_x86_64(&function, &X86_64FixtureAbi) {
        Ok(compiled) => compiled,
        Err(error) => unreachable!("{error:?}"),
    };
    assert_eq!(compiled.safepoint_maps[0].registers, *expected);
    assert!(!compiled.code.is_empty());
}

#[test]
fn x86_64_lowering_reserves_allocator_spills_in_frame() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(72),
        "allocated-spills",
        Vec::new(),
        vec![Ty::Word; 8],
    );
    let mut values = Vec::new();
    for index in 0..8u32 {
        let constant = builder.add_constant(Constant::Fixnum(i64::from(index)));
        values.push(
            match builder.push_op(OpKind::Const { result: constant }, &[Ty::Word]) {
                Ok(ids) => ids[0],
                Err(error) => unreachable!("constant: {error:?}"),
            },
        );
    }
    assert!(builder.push_op(OpKind::Safepoint, &[]).is_ok(), "safepoint");
    assert!(
        builder.terminate(Terminator::Return { values }).is_ok(),
        "return"
    );
    let function = builder.finish();
    let allocation = allocate(&function, AllocationTarget::X86_64);
    assert!(allocation.spill_words > 0);
    let compiled = match compile_function_x86_64(&function, &X86_64FixtureAbi) {
        Ok(compiled) => compiled,
        Err(error) => unreachable!("{error:?}"),
    };
    assert!(compiled.frame_size >= (4 + 8 + allocation.spill_words) * 8);
    assert!(compiled.safepoint_maps[0].bitmap.len() > 1);
}

#[test]
fn lowers_ir_v2_closure_and_handler_ops_x86_64() {
    let mut builder = ncl_ir::FunctionBuilder::new(
        ncl_ir::FunctionId(70),
        "ir-v2-ops",
        Vec::new(),
        vec![Ty::Word],
    );
    let entry = builder.add_constant(Constant::FunctionEntry(ncl_ir::FunctionId(7)));
    let entry_value = ncl_ir::ValueId(0);
    let closure = ncl_ir::ValueId(1);
    let result = ncl_ir::ValueId(2);
    assert!(
        builder
            .push_op(OpKind::Const { result: entry }, &[Ty::Word])
            .is_ok()
    );
    assert!(
        builder
            .push_op(
                OpKind::MakeClosure {
                    entry: entry_value,
                    captures: Vec::new()
                },
                &[Ty::Word]
            )
            .is_ok()
    );
    assert!(
        builder
            .push_op(
                OpKind::CallClosure {
                    closure,
                    args: Vec::new()
                },
                &[Ty::Word]
            )
            .is_ok()
    );
    let region = ncl_ir::HandlerRegionId(3);
    builder.add_handler_region(ncl_ir::HandlerRegion {
        id: region,
        kind: ncl_ir::HandlerKind::UnwindProtect,
        protected: vec![ncl_ir::BlockId(0)],
        handler: ncl_ir::BlockId(0),
        cleanup: Some(ncl_ir::BlockId(0)),
        catch_tag: None,
        binding_targets: Vec::new(),
        depth: 0,
        parent: None,
    });
    assert!(
        builder
            .push_op(OpKind::EnterHandler { region }, &[])
            .is_ok()
    );
    assert!(
        builder
            .push_op(OpKind::LeaveHandler { region }, &[])
            .is_ok()
    );
    assert!(
        builder
            .terminate(Terminator::Return {
                values: vec![result]
            })
            .is_ok()
    );
    let compiled = match compile_function_x86_64(&builder.finish(), &X86_64FixtureAbi) {
        Ok(compiled) => compiled,
        Err(error) => {
            unreachable!("{error:?}");
        }
    };
    assert!(!compiled.code.is_empty());
    assert!(compiled.safepoint_maps.len() >= 4);
}
