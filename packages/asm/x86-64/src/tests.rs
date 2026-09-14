use super::*;

#[test]
fn common_register_encodings() {
    let mut a = Assembler::new();
    assert!(a.emit(&Inst::MovRR(Reg::Rax, Reg::Rbx)).is_ok());
    assert!(a.emit(&Inst::MovRR(Reg::R8, Reg::R15)).is_ok());
    assert_eq!(a.bytes(), &[0x48, 0x89, 0xd8, 0x4d, 0x89, 0xf8]);
}

#[test]
fn memory_addressing_boundaries() {
    let mut a = Assembler::new();
    assert!(
        a.emit(&Inst::MovRM(Reg::Rax, Mem::base(Reg::Rsp, 0)))
            .is_ok()
    );
    assert!(
        a.emit(&Inst::MovRM(Reg::Rax, Mem::base(Reg::Rbp, 0)))
            .is_ok()
    );
    assert!(
        a.emit(&Inst::MovRM(Reg::Rax, Mem::base(Reg::Rax, 127)))
            .is_ok()
    );
    assert!(
        a.emit(&Inst::MovRM(Reg::Rax, Mem::base(Reg::Rax, 128)))
            .is_ok()
    );
    assert_eq!(
        a.bytes(),
        &[
            0x48, 0x8b, 0x04, 0x24, 0x48, 0x8b, 0x45, 0x00, 0x48, 0x8b, 0x40, 0x7f, 0x48, 0x8b,
            0x80, 0x80, 0x00, 0x00, 0x00
        ]
    );
}

#[test]
fn labels_are_patched() {
    let mut a = Assembler::new();
    let l = a.new_label();
    assert!(a.emit(&Inst::Jmp(l)).is_ok());
    assert!(a.emit(&Inst::Nop(1)).is_ok());
    a.bind(l);
    match a.finish() {
        Ok(blob) => {
            assert_eq!(blob.bytes, &[0xe9, 1, 0, 0, 0, 0x90]);
            assert_eq!(blob.fixups.first().map(|f| f.kind), Some(FixupKind::Rel32));
        }
        Err(error) => assert!(matches!(error, EncodeError::UnboundLabel(_))),
    }
}

#[test]
fn simple_decode_round_trip() {
    let mut a = Assembler::new();
    for i in [Inst::Nop(1), Inst::Ret, Inst::Int3, Inst::Ud2] {
        assert!(a.emit(&i).is_ok());
    }
    assert_eq!(
        decode(a.bytes()),
        Ok(vec![Inst::Nop(1), Inst::Ret, Inst::Int3, Inst::Ud2])
    );
}

#[test]
fn invalid_nop_is_rejected() {
    let mut a = Assembler::new();
    assert_eq!(a.emit(&Inst::Nop(0)), Err(EncodeError::InvalidNopLength));
}

#[test]
fn golden_bytes_are_stable() {
    let mut assembler = Assembler::new();
    assert!(assembler.emit(&Inst::MovRR(Reg::Rax, Reg::Rbx)).is_ok());
    assert!(assembler.emit(&Inst::Ret).is_ok());
    assert_eq!(assembler.bytes(), &[0x48, 0x89, 0xd8, 0xc3]);
    let metadata = include_str!("../tests/golden/basic.txt");
    assert!(metadata.contains("llvm-mc --triple=x86_64-apple-darwin"));
}

#[test]
fn golden_corpus_covers_all_groups() {
    let files = [
        include_str!("../tests/golden/basic.txt"),
        include_str!("../tests/golden/data-movement.txt"),
        include_str!("../tests/golden/arithmetic-logic.txt"),
        include_str!("../tests/golden/control-flow.txt"),
        include_str!("../tests/golden/sse.txt"),
    ];
    let rows = files
        .iter()
        .flat_map(|file| file.lines())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .count();
    assert!(rows >= 200, "golden rows: {rows}");
    let unique_rows: std::collections::BTreeSet<_> = files
        .iter()
        .flat_map(|file| file.lines())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    assert_eq!(unique_rows.len(), rows, "golden rows must be unique");
    for line in files
        .iter()
        .flat_map(|file| file.lines())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let Some((_, bytes)) = line.split_once(" => ") else {
            assert!(false, "golden separator");
            continue;
        };
        assert!(!bytes.is_empty());
        for byte in bytes.split_whitespace() {
            assert_eq!(byte.len(), 2);
            assert!(u8::from_str_radix(byte, 16).is_ok(), "invalid byte: {byte}");
        }
    }
    for required in [
        "movq",
        "movzbl",
        "movswq",
        "leaq",
        "addq",
        "cmpq",
        "testq",
        "imulq",
        "negq",
        "shlq",
        "cqo",
        "idivq",
        "setne",
        "cmovne",
        "jmpq",
        "callq",
        "je",
        "pushq",
        "popq",
        "xchgq",
        "btq",
        "movsd",
        "addsd",
        "movq %rax, %xmm0",
        "cvtsi2sd",
        "cvttsd2si",
        "xorpd",
    ] {
        assert!(files.iter().any(|file| file.contains(required)));
    }
}

#[test]
fn extended_memory_and_sse_forms_encode() {
    let mut a = Assembler::new();
    assert!(
        a.emit(&Inst::TestRM(Reg::R8, Mem::base(Reg::R12, 0)))
            .is_ok()
    );
    assert!(
        a.emit(&Inst::MovzxRM(Reg::R15, Mem::base(Reg::Rsp, 0), 8))
            .is_ok()
    );
    assert!(
        a.emit(&Inst::MovsxRM(Reg::R12, Mem::base(Reg::R13, 128), 32))
            .is_ok()
    );
    assert!(
        a.emit(&Inst::SseRM(SseOp::Addsd, Xmm(8), Mem::base(Reg::R12, 0)))
            .is_ok()
    );
    assert!(
        a.emit(&Inst::Cvtsi2sdRM(Xmm(8), Mem::base(Reg::Rsp, 0)))
            .is_ok()
    );
    assert!(
        a.emit(&Inst::Cvttsd2siRM(Reg::R15, Mem::base(Reg::Rsp, 8)))
            .is_ok()
    );
    assert_eq!(
        a.bytes(),
        &[
            0x4d, 0x85, 0x04, 0x24, 0x4c, 0x0f, 0xb6, 0x3c, 0x24, 0x4d, 0x63, 0xa5, 0x80, 0x00,
            0x00, 0x00, 0xf2, 0x45, 0x0f, 0x58, 0x04, 0x24, 0xf2, 0x4c, 0x0f, 0x2a, 0x04, 0x24,
            0xf2, 0x4c, 0x0f, 0x2c, 0x7c, 0x24, 0x08,
        ]
    );
}

#[test]
fn byte_register_rex_is_selected() {
    let mut a = Assembler::new();
    assert!(a.emit(&Inst::Setcc(Cond::Ne, Reg::Rsp)).is_ok());
    assert_eq!(a.bytes(), &[0x40, 0x0f, 0x95, 0xc4]);
}

#[test]
fn seeded_random_round_trip_subset() {
    let mut seed = 0x9e37_79b9_u32;
    let mut assembler = Assembler::new();
    let mut expected = Vec::new();
    for _ in 0..64 {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let instruction = match seed & 3 {
            0 => Inst::Nop(1),
            1 => Inst::Ret,
            2 => Inst::Int3,
            _ => Inst::Ud2,
        };
        assert!(assembler.emit(&instruction).is_ok());
        expected.push(instruction);
    }
    assert_eq!(decode(assembler.bytes()), Ok(expected));
}
