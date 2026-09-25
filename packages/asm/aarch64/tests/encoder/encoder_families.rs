#![allow(missing_docs)]
#![allow(clippy::unwrap_used)]

use super::common::*;
use ncl_asm_aarch64::*;

#[test]
fn instruction_families_encode() {
    use ncl_asm_aarch64::{Extend, MemOperand, VReg};

    let r0 = x(0);
    let r1 = x(1);
    let r2 = x(2);
    let sp = RegOrSp::Sp;
    let mem = MemOperand::Unscaled {
        base: sp,
        offset: 0,
    };
    let memd = MemOperand::Unsigned {
        base: sp,
        offset: 8,
        scale: 8,
    };
    let shifted = Shift::Lsr(7);
    let v0 = VReg {
        number: 0,
        double: true,
    };
    let v1 = VReg {
        number: 1,
        double: true,
    };
    let instructions = [
        Inst::MovK {
            rd: r0,
            imm: 2,
            shift: 16,
        },
        Inst::MovN {
            rd: r0,
            imm: 2,
            shift: 32,
        },
        Inst::Mov {
            rd: sp,
            rn: RegOrSp::Reg(r0),
        },
        Inst::SubImm {
            rd: sp,
            rn: sp,
            imm: 2,
            shift: true,
        },
        Inst::Adds {
            rd: r0,
            rn: r1,
            rm: r2,
            shift: shifted,
        },
        Inst::Subs {
            rd: r0,
            rn: r1,
            rm: r2,
            shift: shifted,
        },
        Inst::AddExt {
            rd: sp,
            rn: sp,
            rm: r0,
            extend: Extend::Uxtw,
            shift: 0,
        },
        Inst::SubExt {
            rd: sp,
            rn: sp,
            rm: r0,
            extend: Extend::Sxtw,
            shift: 1,
        },
        Inst::Ldr { rt: r0, mem },
        Inst::Str { rt: r0, mem },
        Inst::LdrW { rt: r0, mem },
        Inst::StrW { rt: r0, mem },
        Inst::Ldrb { rt: r0, mem },
        Inst::Strb { rt: r0, mem },
        Inst::Ldrh { rt: r0, mem },
        Inst::Strh { rt: r0, mem },
        Inst::Ldrsw { rt: r0, mem },
        Inst::Ldp {
            rt: r0,
            rt2: r1,
            mem,
        },
        Inst::Stp {
            rt: r0,
            rt2: r1,
            mem,
        },
        Inst::Ret { rn: r1 },
        Inst::Blr { rn: r1 },
        Inst::Br { rn: r1 },
        Inst::Nop,
        Inst::Brk { imm: 7 },
        Inst::Cmp {
            rn: r0,
            rm: r1,
            shift: Shift::Lsl(0),
        },
        Inst::Csel {
            rd: r0,
            rn: r1,
            rm: r2,
            cond: Cond::Ne,
        },
        Inst::Cset {
            rd: r0,
            cond: Cond::Eq,
        },
        Inst::Cinc {
            rd: r0,
            rn: r1,
            cond: Cond::Gt,
        },
        Inst::Mul {
            rd: r0,
            rn: r1,
            rm: r2,
        },
        Inst::Sdiv {
            rd: r0,
            rn: r1,
            rm: r2,
        },
        Inst::Udiv {
            rd: r0,
            rn: r1,
            rm: r2,
        },
        Inst::Madd {
            rd: r0,
            rn: r1,
            rm: r2,
            ra: r0,
        },
        Inst::Msub {
            rd: r0,
            rn: r1,
            rm: r2,
            ra: r0,
        },
        Inst::Neg {
            rd: r0,
            rn: r1,
            shift: Shift::Lsl(0),
        },
        Inst::Mvn {
            rd: r0,
            rn: r1,
            shift: Shift::Lsr(1),
        },
        Inst::AddsImm {
            rd: sp,
            rn: sp,
            imm: 3,
            shift: false,
        },
        Inst::SubsImm {
            rd: sp,
            rn: sp,
            imm: 3,
            shift: false,
        },
        Inst::Cmn {
            rn: r0,
            rm: r1,
            shift: Shift::Lsl(0),
        },
        Inst::And {
            rd: r0,
            rn: r1,
            rm: r2,
            shift: shifted,
        },
        Inst::Orr {
            rd: r0,
            rn: r1,
            rm: r2,
            shift: shifted,
        },
        Inst::Eor {
            rd: r0,
            rn: r1,
            rm: r2,
            shift: shifted,
        },
        Inst::Tst {
            rn: r0,
            rm: r1,
            shift: shifted,
        },
        Inst::LslImm {
            rd: r0,
            rn: r1,
            amount: 3,
        },
        Inst::LsrImm {
            rd: r0,
            rn: r1,
            amount: 3,
        },
        Inst::AsrImm {
            rd: r0,
            rn: r1,
            amount: 3,
        },
        Inst::LslReg {
            rd: r0,
            rn: r1,
            rm: r2,
        },
        Inst::LsrReg {
            rd: r0,
            rn: r1,
            rm: r2,
        },
        Inst::AsrReg {
            rd: r0,
            rn: r1,
            rm: r2,
        },
        Inst::Fmov { rd: v0, rn: v1 },
        Inst::FmovGeneral {
            v: v0,
            r: r0,
            to_float: true,
        },
        Inst::Fadd {
            rd: v0,
            rn: v1,
            rm: v0,
        },
        Inst::Fsub {
            rd: v0,
            rn: v1,
            rm: v0,
        },
        Inst::Fmul {
            rd: v0,
            rn: v1,
            rm: v0,
        },
        Inst::Fdiv {
            rd: v0,
            rn: v1,
            rm: v0,
        },
        Inst::Fsqrt { rd: v0, rn: v1 },
        Inst::Fneg { rd: v0, rn: v1 },
        Inst::Fcmp { rn: v0, rm: v1 },
        Inst::Scvtf { rd: v0, rn: r0 },
        Inst::Fcvtzs { rd: r0, rn: v0 },
        Inst::LdrD { rt: v0, mem: memd },
        Inst::StrD { rt: v0, mem: memd },
        Inst::Udf { imm: 1 },
        Inst::DmbIsh,
    ];
    for instruction in instructions {
        let word = encode(&instruction, 0);
        assert!(word.is_ok(), "failed to encode {instruction:?}: {word:?}");
        assert_ne!(word.unwrap(), 0, "zero encoding for {instruction:?}");
    }
}

#[test]
fn public_helpers_cover_success_and_error_paths() {
    assert_eq!(disassemble(0xD503_201F), Ok("nop".to_owned()));
    assert!(disassemble(0).is_err());
    assert!(decode(0xD503_201F).is_ok());
    assert_eq!(mov_imm64(x(0), 0).len(), 1);
    assert_eq!(mov_imm64(x(0), u64::MAX).len(), 1);
    assert_eq!(mov_imm64(x(0), 0x0001_0000_0000_0001).len(), 2);

    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assert_eq!(assembler.offset(), 0);
    assert!(assembler.bind(label).is_ok());
    assert_eq!(
        assembler.bind(label),
        Err(ncl_asm_aarch64::EncodeError::DuplicateLabel(label))
    );
}

