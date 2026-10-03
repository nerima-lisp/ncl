#![allow(missing_docs, clippy::expect_used)]

use ncl_asm_x86_64::{Assembler, Cond, EncodeError, Imm, Inst, Reg};

#[test]
fn public_x86_assembler_patches_backward_branch_with_exact_bytes() {
    let mut assembler = Assembler::new();
    let target = assembler.new_label();
    assembler.bind(target);
    assembler.emit(&Inst::Nop(1)).expect("nop encoding");
    assembler
        .emit(&Inst::Jcc(Cond::Ne, target))
        .expect("branch encoding");
    let blob = assembler.finish().expect("backward fixup");
    assert_eq!(blob.bytes, [0x90, 0x0f, 0x85, 0xf9, 0xff, 0xff, 0xff]);
    assert_eq!(blob.fixups.len(), 1);
}

#[test]
fn public_x86_assembler_rejects_unbound_label_with_exact_error() {
    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assembler.emit(&Inst::Jmp(label)).expect("branch emission");
    assert_eq!(assembler.finish(), Err(EncodeError::UnboundLabel(label)));
}

#[test]
fn public_x86_register_immediate_encoding_is_exact() {
    let mut assembler = Assembler::new();
    assembler
        .emit(&Inst::MovRI(Reg::R14, Imm::I64(-1)))
        .expect("mov encoding");
    assert_eq!(
        assembler.bytes(),
        &[0x49, 0xbe, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]
    );
}
