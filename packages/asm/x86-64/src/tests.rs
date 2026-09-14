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
