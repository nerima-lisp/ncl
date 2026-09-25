#![allow(missing_docs)]
#![allow(clippy::unwrap_used)]

use ncl_asm_x86_64::*;

#[allow(clippy::needless_pass_by_value)]
fn assert_encoding(inst: Inst, expected: &[u8]) {
    let mut assembler = Assembler::new();
    assert!(
        assembler.emit(&inst).is_ok(),
        "encoding failed for {inst:?}"
    );
    assert_eq!(assembler.bytes(), expected, "wrong bytes for {inst:?}");
}

#[allow(clippy::needless_pass_by_value)]
fn assert_invalid_memory(inst: Inst) {
    let mut assembler = Assembler::new();
    assert_eq!(
        assembler.emit(&inst),
        Err(EncodeError::InvalidOperand("index without base")),
        "invalid memory must be rejected for {inst:?}"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn golden_uncovered_instruction_encodings() {
    assert_encoding(
        Inst::MovRI(Reg::R8, Imm::I64(-1)),
        &[0x49, 0xb8, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
    );
    assert_encoding(
        Inst::MovRI(Reg::R15, Imm::I32(-129)),
        &[0x49, 0xc7, 0xc7, 0x7f, 0xff, 0xff, 0xff],
    );
    assert_encoding(
        Inst::MovRI(Reg::Rax, Imm::I8(-1)),
        &[0x48, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff],
    );
    assert_encoding(
        Inst::MovMR(Mem::indexed(Reg::R12, Reg::R8, Scale::Eight, 0), Reg::R15),
        &[0x4f, 0x89, 0x3c, 0xc4],
    );
    assert_encoding(
        Inst::MovMI(Mem::base(Reg::Rsp, 0), 127),
        &[0x48, 0xc7, 0x04, 0x24, 0x7f, 0, 0, 0],
    );
    assert_encoding(
        Inst::BinRR(BinOp::Or, Reg::R15, Reg::R8),
        &[0x4d, 0x09, 0xc7],
    );
    assert_encoding(
        Inst::BinRI(BinOp::Xor, Reg::R8, 128),
        &[0x49, 0x81, 0xf0, 0x80, 0, 0, 0],
    );
    assert_encoding(
        Inst::BinRM(BinOp::And, Reg::R15, Mem::base(Reg::R13, -128)),
        &[0x4d, 0x23, 0x7d, 0x80],
    );
    assert_encoding(
        Inst::BinMR(BinOp::Sub, Mem::base(Reg::R8, 4096), Reg::R15),
        &[0x4d, 0x29, 0xb8, 0, 0x10, 0, 0],
    );
    assert_encoding(
        Inst::CmpRM(Reg::R8, Mem::base(Reg::R12, 8)),
        &[0x4d, 0x3b, 0x44, 0x24, 8],
    );
    assert_encoding(Inst::CmpRR(Reg::R15, Reg::R8), &[0x4d, 0x39, 0xc7]);
    assert_encoding(
        Inst::CmpRI(Reg::Rax, 128),
        &[0x48, 0x81, 0xf8, 0x80, 0, 0, 0],
    );
    assert_encoding(
        Inst::CmpMR(Mem::base(Reg::R12, 128), Reg::R8),
        &[0x4d, 0x39, 0x84, 0x24, 0x80, 0, 0, 0],
    );
    assert_encoding(Inst::TestRR(Reg::R15, Reg::R8), &[0x4d, 0x85, 0xc7]);
    assert_encoding(
        Inst::TestMR(Mem::indexed(Reg::R12, Reg::R8, Scale::Eight, 0), Reg::R15),
        &[0x4f, 0x85, 0x3c, 0xc4],
    );
    assert_encoding(Inst::ImulRR(Reg::R15, Reg::R8), &[0x4d, 0x0f, 0xaf, 0xf8]);
    assert_encoding(
        Inst::ImulRRI(Reg::R15, Reg::R8, -7),
        &[0x4d, 0x69, 0xf8, 0xf9, 0xff, 0xff, 0xff],
    );
    assert_encoding(Inst::Inc(Reg::R12), &[0x49, 0xf7, 0xc4]);
    assert_encoding(Inst::Dec(Reg::R12), &[0x49, 0xf7, 0xcc]);
    assert_encoding(Inst::Idiv(Reg::R15), &[0x49, 0xf7, 0xff]);
    assert_encoding(
        Inst::ShiftImm(Shift::Sar, Reg::R15, 8),
        &[0x49, 0xc1, 0xff, 8],
    );
    assert_encoding(
        Inst::ShiftImm(Shift::Shl, Reg::Rax, 1),
        &[0x48, 0xc1, 0xe0, 1],
    );
    assert_encoding(Inst::ShiftCl(Shift::Shr, Reg::R8), &[0x49, 0xd3, 0xe8]);
    assert_encoding(Inst::Push(Reg::R12), &[0x41, 0x54]);
    assert_encoding(Inst::Pop(Reg::R15), &[0x41, 0x5f]);
    assert_encoding(Inst::Push(Reg::Rax), &[0x50]);
    assert_encoding(Inst::Pop(Reg::Rax), &[0x58]);
    assert_encoding(Inst::Ud2, &[0x0f, 0x0b]);
    assert_encoding(Inst::Int3, &[0xcc]);
    assert_encoding(Inst::Setcc(Cond::E, Reg::Rax), &[0x0f, 0x94, 0xc0]);
    assert_encoding(Inst::Xchg(Reg::R13, Reg::R12), &[0x4d, 0x87, 0xe5]);
    assert_encoding(Inst::Bt(Reg::R15, 63), &[0x49, 0x0f, 0xba, 0xe7, 63]);

    let mut labels = Assembler::new();
    let target = labels.new_label();
    assert!(labels.emit(&Inst::Call(target)).is_ok());
    assert!(labels.emit(&Inst::Jcc(Cond::Ge, target)).is_ok());
    assert!(labels.emit(&Inst::JmpMem(Mem::base(Reg::Rsp, 0))).is_ok());
    assert!(
        labels
            .emit(&Inst::CallMem(Mem::indexed(
                Reg::R12,
                Reg::R8,
                Scale::Eight,
                0
            )))
            .is_ok()
    );
    assert_eq!(
        labels.bytes(),
        &[
            0xe8, 0, 0, 0, 0, 0x0f, 0x8d, 0, 0, 0, 0, 0xff, 0x24, 0x24, 0x43, 0xff, 0x14, 0xc4
        ]
    );

    for (length, expected) in [
        (2, &[0x66, 0x90][..]),
        (3, &[0x0f, 0x1f, 0x00][..]),
        (9, &[0x66, 0x0f, 0x1f, 0x84, 0, 0, 0, 0, 0][..]),
    ] {
        assert_encoding(Inst::Nop(length), expected);
    }
    assert_eq!(
        Assembler::new().emit(&Inst::Nop(10)),
        Err(EncodeError::InvalidNopLength)
    );
}

#[test]
fn golden_sse_encodings() {
    for (op, opcode) in [
        (SseOp::Subsd, 0x5c),
        (SseOp::Mulsd, 0x59),
        (SseOp::Divsd, 0x5e),
        (SseOp::Ucomisd, 0x2e),
        (SseOp::Sqrtsd, 0x51),
    ] {
        assert_encoding(
            Inst::Sse(op, Xmm(15), Xmm(8)),
            &[0xf2, 0x45, 0x0f, opcode, 0xf8],
        );
    }
    assert_encoding(
        Inst::MovsdMR(Mem::base(Reg::R13, 127), Xmm(15)),
        &[0xf2, 0x45, 0x0f, 0x11, 0x7d, 0x7f],
    );
    assert_encoding(
        Inst::MovqXR(Xmm(8), Reg::R15),
        &[0x66, 0x45, 0x0f, 0x6e, 0xc7],
    );
    assert_encoding(
        Inst::MovqRX(Reg::R15, Xmm(8)),
        &[0x66, 0x45, 0x0f, 0x7e, 0xc7],
    );
    assert_encoding(
        Inst::Xorpd(Xmm(15), Xmm(8)),
        &[0x66, 0x45, 0x0f, 0x57, 0xf8],
    );
    assert_encoding(
        Inst::Cvtsi2sd(Xmm(15), Reg::R12),
        &[0xf2, 0x4d, 0x0f, 0x2a, 0xfc],
    );
    assert_encoding(
        Inst::Cvttsd2si(Reg::R15, Xmm(15)),
        &[0xf2, 0x4d, 0x0f, 0x2c, 0xff],
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn model_conversions_and_invalid_memory_are_specific() {
    for id in 0..16 {
        assert_eq!(Reg::from_id(id).map(Reg::id), Some(id));
        assert_eq!(
            Reg::from_code(Reg::from_id(id).unwrap().code()),
            Reg::from_id(id)
        );
    }
    assert_eq!(Reg::from_id(16), None);
    assert_eq!(Reg::from_code(16), None);
    for register in [
        Reg::Rax,
        Reg::Rdx,
        Reg::Rdi,
        Reg::Rsi,
        Reg::Rcx,
        Reg::R8,
        Reg::R9,
        Reg::R10,
        Reg::R11,
        Reg::Rbp,
        Reg::Rbx,
        Reg::R12,
        Reg::R13,
        Reg::R14,
        Reg::R15,
        Reg::Rsp,
    ] {
        assert!(!register.to_string().is_empty());
    }
    for (condition, code) in [
        (Cond::O, 0),
        (Cond::No, 1),
        (Cond::B, 2),
        (Cond::Ae, 3),
        (Cond::E, 4),
        (Cond::Ne, 5),
        (Cond::Be, 6),
        (Cond::A, 7),
        (Cond::S, 8),
        (Cond::Ns, 9),
        (Cond::P, 10),
        (Cond::Np, 11),
        (Cond::L, 12),
        (Cond::Ge, 13),
        (Cond::Le, 14),
        (Cond::G, 15),
    ] {
        assert_eq!(condition.code(), code);
    }
    assert_eq!(
        [
            Scale::One.bits(),
            Scale::Two.bits(),
            Scale::Four.bits(),
            Scale::Eight.bits()
        ],
        [0, 1, 2, 3]
    );
    assert_eq!(
        (
            Imm::I8(-2).value(),
            Imm::I32(128).value(),
            Imm::I64(-1).value()
        ),
        (-2, 128, -1)
    );
    let mut assembler = Assembler::new();
    assert_eq!(
        assembler.emit(&Inst::MovRM(
            Reg::Rax,
            Mem {
                base: None,
                index: Some(Reg::R8),
                scale: Scale::Two,
                disp: 0,
                rip: false
            },
        )),
        Err(EncodeError::InvalidOperand("index without base")),
    );
    assert_eq!(
        assembler.emit(&Inst::MovRM(
            Reg::Rax,
            Mem {
                base: Some(Reg::Rax),
                index: None,
                scale: Scale::One,
                disp: 0,
                rip: true
            },
        )),
        Err(EncodeError::InvalidOperand(
            "rip-relative memory has base or index"
        )),
    );
    assert_encoding(
        Inst::MovRM(
            Reg::Rax,
            Mem {
                base: None,
                index: None,
                scale: Scale::Four,
                disp: 8,
                rip: false,
            },
        ),
        &[0x48, 0x8b, 0x04, 0x85, 8, 0, 0, 0],
    );
    assert_encoding(Inst::BinRI(BinOp::Add, Reg::Rax, 1), &[0x48, 0x83, 0xc0, 1]);
    assert_encoding(Inst::CmpRI(Reg::Rax, -1), &[0x48, 0x83, 0xf8, 0xff]);
    assert_encoding(Inst::Cqo, &[0x48, 0x99]);
    assert_encoding(
        Inst::Cmovcc(Cond::G, Reg::R15, Reg::R8),
        &[0x4d, 0x0f, 0x4f, 0xf8],
    );
    assert_eq!(
        assembler.emit(&Inst::Movzx(Reg::Rax, Reg::Rbx, 7)),
        Err(EncodeError::InvalidOperand("movzx width"))
    );
    assert_eq!(
        assembler.emit(&Inst::MovzxRM(Reg::Rax, Mem::base(Reg::Rbx, 0), 7)),
        Err(EncodeError::InvalidOperand("movzx width"))
    );
    assert_eq!(
        assembler.emit(&Inst::Movsx(Reg::Rax, Reg::Rbx, 7)),
        Err(EncodeError::InvalidOperand("movsx width"))
    );
    assert_eq!(
        assembler.emit(&Inst::MovsxRM(Reg::Rax, Mem::base(Reg::Rbx, 0), 7)),
        Err(EncodeError::InvalidOperand("movsx width"))
    );
    assert_encoding(
        Inst::Movzx(Reg::R15, Reg::R8, 16),
        &[0x4d, 0x0f, 0xb7, 0xf8],
    );
    assert_encoding(Inst::Movsx(Reg::R15, Reg::R8, 32), &[0x4d, 0x63, 0xf8]);
    assert_encoding(
        Inst::MovzxRM(Reg::R15, Mem::base(Reg::Rax, 0), 16),
        &[0x4c, 0x0f, 0xb7, 0x38],
    );
    assert_encoding(Inst::Movsx(Reg::R15, Reg::R8, 8), &[0x4d, 0x0f, 0xbe, 0xf8]);
    assert_encoding(
        Inst::Movsx(Reg::R15, Reg::R8, 16),
        &[0x4d, 0x0f, 0xbf, 0xf8],
    );
    assert_encoding(
        Inst::MovsxRM(Reg::R15, Mem::base(Reg::Rax, 0), 8),
        &[0x4c, 0x0f, 0xbe, 0x38],
    );
}

#[test]
fn reachable_error_displays_and_sse_memory_are_specific() {
    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assert!(assembler.emit(&Inst::Jmp(label)).is_ok());
    assert_eq!(assembler.finish(), Err(EncodeError::UnboundLabel(label)));
    assert_eq!(
        EncodeError::UnboundLabel(label).to_string(),
        "unbound label Label(0)"
    );
    assert_eq!(
        EncodeError::Rel32OutOfRange.to_string(),
        "rel32 out of range"
    );
    assert_eq!(EncodeError::InvalidOperand("bad").to_string(), "bad");
    assert_eq!(
        EncodeError::InvalidNopLength.to_string(),
        "nop length must be 1..=9"
    );
    assert_eq!(
        EncodeError::InvalidRegister(7).to_string(),
        "invalid register 7"
    );
    assert_eq!(EncodeError::BufferTooLarge.to_string(), "buffer too large");
    assert_eq!(
        EncodeError::UnsupportedFixup(FixupKind::Abs64).to_string(),
        "unsupported fixup kind Abs64"
    );
    assert_encoding(
        Inst::SseRM(SseOp::Addsd, Xmm(15), Mem::base(Reg::Rax, 0)),
        &[0xf2, 0x44, 0x0f, 0x58, 0x38],
    );
    assert_encoding(
        Inst::MovsdRM(Xmm(8), Mem::base(Reg::Rax, 0)),
        &[0xf2, 0x44, 0x0f, 0x10, 0x00],
    );
    assert_encoding(
        Inst::MovsdRM(Xmm(8), Mem::indexed(Reg::R12, Reg::R8, Scale::Four, 16)),
        &[0xf2, 0x47, 0x0f, 0x10, 0x44, 0x84, 0x10],
    );
    assert_encoding(
        Inst::MovsdMR(Mem::rip(-4), Xmm(8)),
        &[0xf2, 0x44, 0x0f, 0x11, 0x05, 0xfc, 0xff, 0xff, 0xff],
    );
    assert_encoding(
        Inst::SseRM(
            SseOp::Subsd,
            Xmm(1),
            Mem::indexed(Reg::R12, Reg::R8, Scale::Two, 0),
        ),
        &[0xf2, 0x43, 0x0f, 0x5c, 0x0c, 0x44],
    );
    assert_encoding(
        Inst::Cvtsi2sdRM(Xmm(1), Mem::rip(8)),
        &[0xf2, 0x48, 0x0f, 0x2a, 0x0d, 8, 0, 0, 0],
    );
    assert_encoding(
        Inst::Cvttsd2siRM(Reg::Rax, Mem::rip(8)),
        &[0xf2, 0x48, 0x0f, 0x2c, 0x05, 8, 0, 0, 0],
    );
}

#[test]
fn memory_encoding_errors_propagate_for_every_form() {
    let invalid = Mem {
        base: None,
        index: Some(Reg::R8),
        scale: Scale::One,
        disp: 0,
        rip: false,
    };
    assert_invalid_memory(Inst::MovRM(Reg::Rax, invalid));
    assert_invalid_memory(Inst::MovMR(invalid, Reg::Rax));
    assert_invalid_memory(Inst::MovMI(invalid, 0));
    assert_invalid_memory(Inst::Lea(Reg::Rax, invalid));
    assert_invalid_memory(Inst::BinRM(BinOp::Add, Reg::Rax, invalid));
    assert_invalid_memory(Inst::BinMR(BinOp::Add, invalid, Reg::Rax));
    assert_invalid_memory(Inst::CmpRM(Reg::Rax, invalid));
    assert_invalid_memory(Inst::CmpMR(invalid, Reg::Rax));
    assert_invalid_memory(Inst::TestRM(Reg::Rax, invalid));
    assert_invalid_memory(Inst::TestMR(invalid, Reg::Rax));
    assert_invalid_memory(Inst::JmpMem(invalid));
    assert_invalid_memory(Inst::CallMem(invalid));
    assert_invalid_memory(Inst::MovzxRM(Reg::Rax, invalid, 8));
    assert_invalid_memory(Inst::MovsxRM(Reg::Rax, invalid, 8));
    assert_invalid_memory(Inst::MovsdRM(Xmm(0), invalid));
    assert_invalid_memory(Inst::MovsdMR(invalid, Xmm(0)));
    assert_invalid_memory(Inst::SseRM(SseOp::Addsd, Xmm(0), invalid));
    assert_invalid_memory(Inst::Cvtsi2sdRM(Xmm(0), invalid));
    assert_invalid_memory(Inst::Cvttsd2siRM(Reg::Rax, invalid));
}
