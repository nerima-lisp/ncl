#![allow(missing_docs, clippy::unwrap_used)]

use super::{Aarch64FixtureAbi, decoded_text};
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

    let compiled = compile_function_aarch64(&builder.finish(), &Aarch64FixtureAbi);
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
    assert!(instructions.iter().any(|text| text == "mov x6, x17"));
}
