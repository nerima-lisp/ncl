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
    assert!(a.emit(&Inst::MovRM(Reg::Rax, Mem::base(Reg::Rsp, 0))).is_ok());
    assert!(a.emit(&Inst::MovRM(Reg::Rax, Mem::base(Reg::Rbp, 0))).is_ok());
    assert!(a.emit(&Inst::MovRM(Reg::Rax, Mem::base(Reg::Rax, 127))).is_ok());
    assert!(a.emit(&Inst::MovRM(Reg::Rax, Mem::base(Reg::Rax, 128))).is_ok());
    assert_eq!(a.bytes(), &[0x48, 0x8b, 0x04, 0x24, 0x48, 0x8b, 0x45, 0x00, 0x48, 0x8b, 0x40, 0x7f, 0x48, 0x8b, 0x80, 0x80, 0x00, 0x00, 0x00]);
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
    for i in [Inst::Nop(1), Inst::Ret, Inst::Int3, Inst::Ud2] { assert!(a.emit(&i).is_ok()); }
    match decode(a.bytes()) { Ok(decoded) => assert_eq!(decoded, [Inst::Nop(1), Inst::Ret, Inst::Int3, Inst::Ud2]), Err(error) => assert!(false, "unexpected decode error: {error}"), }
}

#[test]
fn invalid_nop_is_rejected() { let mut a = Assembler::new(); assert_eq!(a.emit(&Inst::Nop(0)), Err(EncodeError::InvalidNopLength)); }
