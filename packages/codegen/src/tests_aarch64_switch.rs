#![allow(missing_docs, clippy::unwrap_used)]

use super::{Aarch64FixtureAbi, AllocationTarget, allocate, decoded_text};
use crate::compile_function_aarch64;
use ncl_ir::{FunctionBuilder, Terminator, Ty};

#[test]
fn aarch64_switch_compares_cases_and_moves_block_arguments() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(31),
        "switch-arguments",
        vec![
            ncl_ir::Param {
                name: "selector".into(),
                ty: Ty::I64,
            },
            ncl_ir::Param {
                name: "payload".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let case_target = builder.create_block(vec![(Ty::Word, ncl_ir::ValueId(2))]);
    let default_target = builder.create_block(vec![(Ty::Word, ncl_ir::ValueId(3))]);
    assert!(builder.position_at(ncl_ir::BlockId(0)).is_ok());
    assert!(
        builder
            .terminate(Terminator::Switch {
                value: ncl_ir::ValueId(0),
                cases: vec![(0x1234_5678_9abc_def0, case_target, vec![ncl_ir::ValueId(1)])],
                default: default_target,
                default_args: vec![ncl_ir::ValueId(1)],
            })
            .is_ok()
    );
    assert!(builder.position_at(case_target).is_ok());
    assert!(
        builder
            .terminate(Terminator::Return {
                values: vec![ncl_ir::ValueId(2)],
            })
            .is_ok()
    );
    assert!(builder.position_at(default_target).is_ok());
    assert!(
        builder
            .terminate(Terminator::Return {
                values: vec![ncl_ir::ValueId(3)],
            })
            .is_ok()
    );

    let function = builder.finish();
    let allocation = allocate(&function, AllocationTarget::AArch64);
    let source = allocation.location(ncl_ir::ValueId(1));
    assert!(source.is_some(), "payload location");
    let Some(source) = source else {
        return;
    };
    let destination = allocation.location(ncl_ir::ValueId(2));
    assert!(destination.is_some(), "case block parameter location");
    let Some(destination) = destination else {
        return;
    };
    assert_ne!(source, destination, "case payload must be moved");
    let expected_move = match destination {
        crate::Location::Register(register) => {
            format!("orr x{register}, x31, x17, lsl #0")
        }
        crate::Location::Spill(_) => "str x17, [x16, #0]".to_owned(),
    };
    let compiled = compile_function_aarch64(&function, &Aarch64FixtureAbi::default());
    assert!(compiled.is_ok(), "switch failed to compile: {compiled:?}");
    let Some(compiled) = compiled.ok() else {
        return;
    };
    let (encoded_instructions, remainder) = compiled.code.as_chunks::<4>();
    assert!(remainder.is_empty(), "AArch64 code must be word-aligned");
    let instructions = encoded_instructions
        .iter()
        .map(|word| decoded_text(*word, "switch instruction"))
        .collect::<Vec<_>>();
    assert!(
        instructions
            .iter()
            .any(|text| text.starts_with("subs sp, x16, x17"))
    );
    assert!(instructions.iter().any(|text| text.starts_with("b.eq")));
    assert!(
        instructions.iter().any(|text| text == &expected_move),
        "switch block argument must be stored in its allocated location: {instructions:?}"
    );
}
