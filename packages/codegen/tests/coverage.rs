#![allow(clippy::expect_used, missing_docs)]

use ncl_codegen::{
    AArch64TargetIsa, Aarch64Abi, CodegenError, FrameLayout, TargetIsa, TemplateKind,
    compile_function_aarch64,
};
use ncl_ir::{FunctionBuilder, OpKind, Param, Prim, Terminator, Ty};

#[test]
fn aarch64_templates_encode_expected_instruction_words() {
    let isa = AArch64TargetIsa;
    let cases = [
        (TemplateKind::Return, [0xc0, 0x03, 0x5f, 0xd6]),
        (TemplateKind::Branch, [0x21, 0x00, 0x00, 0x54]),
        (TemplateKind::Jump, [0x01, 0x00, 0x00, 0x14]),
        (TemplateKind::Call, [0x20, 0x02, 0x3f, 0xd6]),
        (TemplateKind::Alloc, [0x1f, 0x20, 0x03, 0xd5]),
    ];

    for (kind, expected) in cases {
        let encoded = isa.encode_template(kind).expect("template encoding");
        assert_eq!(encoded.as_slice(), expected, "{kind:?}");
    }
}

#[test]
fn frame_layout_rounds_size_up_to_16_byte_alignment() {
    let unpadded = FrameLayout::new(1, 2, 3).expect("representable frame");
    assert_eq!(unpadded.argument_words, 1);
    assert_eq!(unpadded.local_words, 2);
    assert_eq!(unpadded.outgoing_words, 3);
    assert_eq!(unpadded.frame_words, 10);
    assert_eq!(unpadded.size_bytes(), 80);
    assert_eq!(unpadded.local_offset_bytes(), 40);

    let padded = FrameLayout::new(2, 1, 0).expect("representable frame");
    assert_eq!(padded.frame_words, 8);
    assert_eq!(padded.size_bytes(), 64);
}

#[test]
fn frame_layout_reports_each_word_count_overflow_as_frame_overflow() {
    let cases = [
        (u32::MAX, 0, 0),
        (u32::MAX - 4, 1, 0),
        (u32::MAX - 4, 0, 1),
        (u32::MAX - 4, 0, 0),
    ];

    for (arguments, locals, outgoing) in cases {
        assert_eq!(
            FrameLayout::new(arguments, locals, outgoing),
            Err(CodegenError::FrameOverflow),
            "arguments={arguments}, locals={locals}, outgoing={outgoing}"
        );
    }
}

#[test]
fn fixnum_lt_compiles_to_true_or_nil_selected_by_signed_less_than() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(1),
        "fixnum-lt",
        vec![
            Param {
                name: "left".into(),
                ty: Ty::I64,
            },
            Param {
                name: "right".into(),
                ty: Ty::I64,
            },
        ],
        vec![Ty::Bool],
    );
    let left = builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::I64])
        .expect("left argument")[0];
    let right = builder
        .push_op(OpKind::LoadArg { index: 1 }, &[Ty::I64])
        .expect("right argument")[0];
    let result = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumLt,
                args: vec![left, right],
                condition: None,
            },
            &[Ty::Bool],
        )
        .expect("less-than operation");
    builder
        .terminate(Terminator::Return { values: result })
        .expect("return terminator");

    let compiled =
        compile_function_aarch64(&builder.finish(), &Aarch64Abi).expect("AArch64 compilation");
    let (instructions, trailing_bytes) = compiled.code.as_chunks::<4>();
    assert!(
        trailing_bytes.is_empty(),
        "instruction bytes must be aligned"
    );
    let position = |word: u32| {
        instructions
            .iter()
            .position(|bytes| *bytes == word.to_le_bytes())
            .expect("expected instruction in compiled function")
    };
    let compare = position(0xeb11_021f); // cmp x16, x17
    let load_true = position(0xd280_00f0); // movz x16, #7
    let load_nil = position(0xd280_0031); // movz x17, #1
    let select = position(0x9a91_b210); // csel x16, x16, x17, lt

    assert!(
        compare < select,
        "comparison must precede boolean selection"
    );
    assert!(
        compare < load_true,
        "comparison must precede loading the selected constants"
    );
    assert!(
        compare < load_nil,
        "comparison must precede loading the selected constants"
    );
    assert!(
        load_true < select,
        "true must be ready before boolean selection"
    );
    assert!(
        load_nil < select,
        "NIL must be ready before boolean selection"
    );
}
