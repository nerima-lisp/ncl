#![allow(missing_docs, clippy::unwrap_used)]

use crate::tests_x86_64_fixture::X86_64FixtureAbi;
use crate::{FLAG_CALL, compile_function_x86_64};
use ncl_ir::{Compare, Constant, FunctionBuilder, OpKind, Terminator, Ty};

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn count_occurrences(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

#[test]
fn golden_x86_64_safepoint_pc_follows_emitted_instruction() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(26),
        "x86-64-pc-tracking",
        Vec::new(),
        vec![],
    );
    let constant = builder.add_constant(Constant::Fixnum(0x1234_5678));
    assert!(
        builder
            .push_op(OpKind::Const { result: constant }, &[Ty::Word])
            .is_ok()
    );
    assert!(builder.push_op(OpKind::Safepoint, &[]).is_ok());
    assert!(
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .is_ok()
    );
    let compiled = compile_function_x86_64(&builder.finish(), &X86_64FixtureAbi);
    assert!(compiled.is_ok(), "x86-64 fixture failed: {compiled:?}");
    let Some(compiled) = compiled.ok() else {
        return;
    };
    let Some(map) = compiled.safepoint_maps.first() else {
        return;
    };
    let end = usize::try_from(map.pc_offset).unwrap_or(0);
    assert!(end >= 10);
    // The continuation PC is materialised by a RIP-relative `lea` whose
    // displacement spans the three-byte indirect call that follows it, so the
    // map PC lands immediately after that call.
    assert_eq!(
        compiled.code[end - 10..end - 3],
        [0x48, 0x8D, 0x15, 0x03, 0x00, 0x00, 0x00]
    );
}

#[test]
fn golden_x86_64_prologue_spills_register_arguments() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(28),
        "spill-arguments",
        vec![
            ncl_ir::Param {
                name: "left".into(),
                ty: Ty::Word,
            },
            ncl_ir::Param {
                name: "right".into(),
                ty: Ty::Word,
            },
        ],
        vec![],
    );
    assert!(builder.push_op(OpKind::Safepoint, &[]).is_ok());
    assert!(
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .is_ok()
    );
    let compiled_result = compile_function_x86_64(&builder.finish(), &X86_64FixtureAbi);
    assert!(compiled_result.is_ok());
    let Some(compiled) = compiled_result.ok() else {
        return;
    };
    // push rbp; mov rbp, rsp; mov [rbp+16], r10; mov r11, 0; mov [rbp+24], r11;
    // sub rsp, 32; mov [rbp-8], rsi; mov [rbp-16], rdx
    assert_eq!(compiled.code[0], 0x55);
    assert_eq!(compiled.code[1..4], [0x48, 0x89, 0xE5]);
    assert_eq!(compiled.code[4..8], [0x4C, 0x89, 0x55, 0x10]);
    assert_eq!(
        compiled.code[8..15],
        [0x49, 0xC7, 0xC3, 0x00, 0x00, 0x00, 0x00]
    );
    assert_eq!(compiled.code[15..19], [0x4C, 0x89, 0x5D, 0x18]);
    assert_eq!(compiled.code[19..23], [0x48, 0x83, 0xEC, 0x20]);
    assert_eq!(compiled.code[23..27], [0x48, 0x89, 0x75, 0xF8]);
    assert_eq!(compiled.code[27..31], [0x48, 0x89, 0x55, 0xF0]);
}

#[test]
fn golden_x86_64_prologue_stores_function_object_and_flags() {
    let mut builder =
        FunctionBuilder::new(ncl_ir::FunctionId(29), "frame-header", Vec::new(), vec![]);
    assert!(
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .is_ok()
    );
    let compiled_result = compile_function_x86_64(&builder.finish(), &X86_64FixtureAbi);
    assert!(compiled_result.is_ok());
    let Some(compiled) = compiled_result.ok() else {
        return;
    };
    assert_eq!(compiled.code[4..8], [0x4C, 0x89, 0x55, 0x10]);
    assert_eq!(compiled.code[15..19], [0x4C, 0x89, 0x5D, 0x18]);
}

#[test]
fn golden_x86_64_compare_materialises_full_word() {
    let mut builder = FunctionBuilder::new(ncl_ir::FunctionId(30), "compare", Vec::new(), vec![]);
    let left_constant = builder.add_constant(Constant::Fixnum(1));
    let left = builder.push_op(
        OpKind::Const {
            result: left_constant,
        },
        &[Ty::Word],
    );
    assert!(left.is_ok());
    let left = left.map_or(ncl_ir::ValueId(0), |ids| ids[0]);
    let right_constant = builder.add_constant(Constant::Fixnum(2));
    let right = builder.push_op(
        OpKind::Const {
            result: right_constant,
        },
        &[Ty::Word],
    );
    assert!(right.is_ok());
    let right = right.map_or(ncl_ir::ValueId(0), |ids| ids[0]);
    assert!(
        builder
            .push_op(
                OpKind::Compare {
                    op: Compare::Eq,
                    left,
                    right,
                },
                &[Ty::Word],
            )
            .is_ok()
    );
    assert!(
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .is_ok()
    );
    let compiled_result = compile_function_x86_64(&builder.finish(), &X86_64FixtureAbi);
    assert!(compiled_result.is_ok());
    let Some(compiled) = compiled_result.ok() else {
        return;
    };
    // `sete r10b` only writes the low byte, so `movzx r10, r10b` clears the rest.
    assert!(contains(
        &compiled.code,
        &[0x41, 0x0F, 0x94, 0xC2, 0x4D, 0x0F, 0xB6, 0xD2]
    ));
}

#[test]
fn golden_x86_64_fixnum_constant_uses_imm32() {
    let mut builder = FunctionBuilder::new(ncl_ir::FunctionId(31), "imm32", Vec::new(), vec![]);
    let constant = builder.add_constant(Constant::Fixnum(0x1234_5678));
    assert!(
        builder
            .push_op(OpKind::Const { result: constant }, &[Ty::Word])
            .is_ok()
    );
    assert!(
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .is_ok()
    );
    let compiled_result = compile_function_x86_64(&builder.finish(), &X86_64FixtureAbi);
    assert!(compiled_result.is_ok());
    let Some(compiled) = compiled_result.ok() else {
        return;
    };
    // ncl_sys::Word::fixnum(0x1234_5678).bits() = 0x2468ACF0 fits the signed imm32 form.
    assert!(contains(
        &compiled.code,
        &[0x49, 0xC7, 0xC2, 0xF0, 0xAC, 0x68, 0x24]
    ));
}

#[test]
fn golden_x86_64_call_map_matches_return_address() {
    let mut builder = FunctionBuilder::new(ncl_ir::FunctionId(32), "call-map", Vec::new(), vec![]);
    let callee_constant = builder.add_constant(Constant::Fixnum(0));
    let callee = builder.push_op(
        OpKind::Const {
            result: callee_constant,
        },
        &[Ty::Word],
    );
    assert!(callee.is_ok());
    let callee = callee.map_or(ncl_ir::ValueId(0), |ids| ids[0]);
    assert!(
        builder
            .push_op(
                OpKind::Call {
                    function: callee,
                    args: Vec::new(),
                },
                &[Ty::Word],
            )
            .is_ok()
    );
    assert!(
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .is_ok()
    );
    let compiled_result = compile_function_x86_64(&builder.finish(), &X86_64FixtureAbi);
    assert!(compiled_result.is_ok());
    let Some(compiled) = compiled_result.ok() else {
        return;
    };
    let Some(map) = compiled.safepoint_maps.first() else {
        return;
    };
    assert_ne!(map.map_flags & FLAG_CALL, 0);
    let end = usize::try_from(map.pc_offset).unwrap_or(0);
    // The map must sit on the callee's return address: the indirect call is the
    // three bytes before it, and the caller releases its header reservation after.
    assert_eq!(compiled.code[end - 3..end], [0x41, 0xFF, 0xD3]);
    assert_eq!(compiled.code[end..end + 4], [0x48, 0x83, 0xC4, 0x10]);
}

#[test]
fn golden_x86_64_switch_dispatches_on_value() {
    let mut builder = FunctionBuilder::new(ncl_ir::FunctionId(33), "switch", Vec::new(), vec![]);
    let selector_constant = builder.add_constant(Constant::Fixnum(1));
    let selector = builder.push_op(
        OpKind::Const {
            result: selector_constant,
        },
        &[Ty::Word],
    );
    assert!(selector.is_ok());
    let selector = selector.map_or(ncl_ir::ValueId(0), |ids| ids[0]);
    let case_zero = builder.create_block(Vec::new());
    let case_one = builder.create_block(Vec::new());
    let fallback = builder.create_block(Vec::new());
    assert!(builder.position_at(ncl_ir::BlockId(0)).is_ok());
    assert!(
        builder
            .terminate(Terminator::Switch {
                value: selector,
                cases: vec![(0, case_zero, Vec::new()), (1, case_one, Vec::new())],
                default: fallback,
                default_args: Vec::new(),
            })
            .is_ok()
    );
    for block in [case_zero, case_one, fallback] {
        assert!(builder.position_at(block).is_ok());
        assert!(
            builder
                .terminate(Terminator::Return { values: Vec::new() })
                .is_ok()
        );
    }
    let compiled_result = compile_function_x86_64(&builder.finish(), &X86_64FixtureAbi);
    assert!(
        compiled_result.is_ok(),
        "switch fixture failed: {compiled_result:?}"
    );
    let Some(compiled) = compiled_result.ok() else {
        return;
    };
    // One compare per case, and one conditional branch per case.
    assert!(contains(&compiled.code, &[0x49, 0x83, 0xFA, 0x00]));
    assert!(contains(&compiled.code, &[0x49, 0x83, 0xFA, 0x01]));
    assert_eq!(count_occurrences(&compiled.code, &[0x0F, 0x84]), 2);
}

#[test]
fn golden_x86_64_swapped_block_arguments_stage_on_the_stack() {
    let mut builder = FunctionBuilder::new(ncl_ir::FunctionId(34), "swap", Vec::new(), vec![]);
    let swap_block = builder.create_block(vec![
        (Ty::Word, ncl_ir::ValueId(100)),
        (Ty::Word, ncl_ir::ValueId(101)),
    ]);
    let first_constant = builder.add_constant(Constant::Fixnum(0));
    let first = builder.push_op(
        OpKind::Const {
            result: first_constant,
        },
        &[Ty::Word],
    );
    assert!(first.is_ok());
    let first = first.map_or(ncl_ir::ValueId(0), |ids| ids[0]);
    let second_constant = builder.add_constant(Constant::Fixnum(1));
    let second = builder.push_op(
        OpKind::Const {
            result: second_constant,
        },
        &[Ty::Word],
    );
    assert!(second.is_ok());
    let second = second.map_or(ncl_ir::ValueId(0), |ids| ids[0]);
    assert!(
        builder
            .terminate(Terminator::Jump {
                target: swap_block,
                args: vec![first, second],
            })
            .is_ok()
    );
    assert!(builder.position_at(swap_block).is_ok());
    // Passing the block's own parameters back in swapped order is a cycle, so a
    // sequential copy would clobber a source before it is read.
    assert!(
        builder
            .terminate(Terminator::Jump {
                target: swap_block,
                args: vec![ncl_ir::ValueId(101), ncl_ir::ValueId(100)],
            })
            .is_ok()
    );
    let compiled_result = compile_function_x86_64(&builder.finish(), &X86_64FixtureAbi);
    assert!(
        compiled_result.is_ok(),
        "swap fixture failed: {compiled_result:?}"
    );
    let Some(compiled) = compiled_result.ok() else {
        return;
    };
    assert!(contains(&compiled.code, &[0x41, 0x53]));
    assert!(contains(&compiled.code, &[0x41, 0x5B]));
}
