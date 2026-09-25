#![allow(missing_docs)]
#![allow(clippy::unwrap_used)]

use ncl_asm_aarch64::{
    Assembler, Cond, Inst, Label, Reg, RegOrSp, Shift, decode, disassemble, encode, mov_imm64,
};

#[allow(
    clippy::option_if_let_else,
    reason = "The helper is const so test register literals remain compile-time values."
)]
const fn x(n: u8) -> Reg {
    match Reg::new(n) {
        Ok(register) => register,
        Err(_) => Reg(0),
    }
}

#[test]
fn golden_core_words() {
    assert_eq!(encode(&Inst::Nop, 0), Ok(0xD503_201F));
    assert_eq!(encode(&Inst::Ret { rn: x(30) }, 0), Ok(0xD65F_03C0));
    assert_eq!(
        encode(
            &Inst::MovZ {
                rd: x(0),
                imm: 1,
                shift: 0
            },
            0
        ),
        Ok(0xD280_0020)
    );
    assert_eq!(
        encode(
            &Inst::AddImm {
                rd: RegOrSp::Sp,
                rn: RegOrSp::Sp,
                imm: 1,
                shift: false
            },
            0
        ),
        Ok(0x9100_07FF)
    );
    assert_eq!(
        encode(
            &Inst::Ldp {
                rt: x(29),
                rt2: x(30),
                mem: ncl_asm_aarch64::MemOperand::PostIndex {
                    base: RegOrSp::Sp,
                    offset: 16,
                },
            },
            0,
        ),
        Ok(0xA8C1_7BFD)
    );
    assert_eq!(
        encode(
            &Inst::Mov {
                rd: RegOrSp::Reg(x(29)),
                rn: RegOrSp::Sp,
            },
            0,
        ),
        Ok(0x9100_03FD)
    );
}

#[test]
fn golden_coverage_corpus_is_present() {
    let lines = include_str!("golden/coverage.txt")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    assert!(lines >= 200);
}

#[test]
fn labels_resolve_and_are_retained() {
    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assert!(assembler.emit(&Inst::B { label }).is_ok());
    assert!(assembler.bind(label).is_ok());
    let result = assembler.finish();
    assert!(result.is_ok());
    if let Ok(blob) = result {
        assert_eq!(blob.bytes, [1, 0, 0, 20]);
        assert_eq!(blob.fixups[0].target, label);
    }
}

#[test]
fn immediate_sequence_and_decode() {
    assert_eq!(mov_imm64(x(0), 1).len(), 1);
    assert_eq!(decode(0xD503_201F), Ok(Inst::Nop));
    assert!(
        encode(
            &Inst::Add {
                rd: RegOrSp::Reg(x(0)),
                rn: RegOrSp::Reg(x(1)),
                rm: x(2),
                shift: Shift::Lsl(0)
            },
            0
        )
        .is_ok()
    );
}

#[test]
fn unbound_label_is_an_error() {
    let mut assembler = Assembler::new();
    assert!(assembler.emit(&Inst::B { label: Label(7) }).is_ok());
    assert!(assembler.finish().is_err());
}

#[test]
fn immediate_and_memory_boundaries_are_checked() {
    assert!(
        encode(
            &Inst::MovZ {
                rd: x(0),
                imm: 1,
                shift: 64
            },
            0
        )
        .is_err()
    );
    assert!(
        encode(
            &Inst::AddImm {
                rd: RegOrSp::Sp,
                rn: RegOrSp::Sp,
                imm: 4096,
                shift: false
            },
            0
        )
        .is_err()
    );
    assert!(
        encode(
            &Inst::Ldr {
                rt: x(0),
                mem: ncl_asm_aarch64::MemOperand::Unscaled {
                    base: RegOrSp::Sp,
                    offset: 256
                }
            },
            0
        )
        .is_err()
    );
    assert!(
        encode(
            &Inst::AndImm {
                rd: x(0),
                rn: x(1),
                imm: 0
            },
            0
        )
        .is_err()
    );
    assert!(
        encode(
            &Inst::AndImm {
                rd: x(0),
                rn: x(1),
                imm: u64::MAX
            },
            0
        )
        .is_err()
    );
}

#[test]
fn branch_kinds_are_retained() {
    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assert!(
        assembler
            .emit(&Inst::Tbz {
                rt: x(0),
                bit: 3,
                label
            })
            .is_ok()
    );
    assert!(
        assembler
            .emit(&Inst::LdrLiteral { rt: x(1), label })
            .is_ok()
    );
    assert!(assembler.bind(label).is_ok());
    let blob = assembler.finish();
    assert!(blob.is_ok());
    if let Ok(blob) = blob {
        assert_eq!(blob.fixups.len(), 2);
    }
}

#[test]
fn pc_relative_fixups_use_signed_aarch64_layout() {
    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assembler.emit(&Inst::Adr { rd: x(2), label }).unwrap();
    assembler.emit(&Inst::Nop).unwrap();
    assembler.bind(label).unwrap();
    assert_eq!(
        assembler.finish().unwrap().bytes[..4],
        [0x42, 0x00, 0x00, 0x10]
    );

    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assembler.bind(label).unwrap();
    assembler.emit(&Inst::Nop).unwrap();
    assembler.emit(&Inst::Adr { rd: x(2), label }).unwrap();
    assert_eq!(
        assembler.finish().unwrap().bytes[4..8],
        [0xe2, 0xff, 0xff, 0x10]
    );

    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assembler.bind(label).unwrap();
    assembler.emit(&Inst::Nop).unwrap();
    assembler.emit(&Inst::B { label }).unwrap();
    assert_eq!(
        assembler.finish().unwrap().bytes[4..8],
        [0xff, 0xff, 0xff, 0x17]
    );

    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assembler.emit(&Inst::Adrp { rd: x(2), label }).unwrap();
    for _ in 0..1024 {
        assembler.emit(&Inst::Nop).unwrap();
    }
    assembler.bind(label).unwrap();
    assert_eq!(
        assembler.finish().unwrap().bytes[..4],
        [0x02, 0x00, 0x00, 0xb0]
    );

    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assembler.bind(label).unwrap();
    for _ in 0..1024 {
        assembler.emit(&Inst::Nop).unwrap();
    }
    assembler.emit(&Inst::Adrp { rd: x(2), label }).unwrap();
    assert_eq!(
        assembler.finish().unwrap().bytes[4096..4100],
        [0xe2, 0xff, 0xff, 0xf0]
    );
}

#[test]
fn pc_relative_adr_rejects_out_of_range_target() {
    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assembler.emit(&Inst::Adr { rd: x(2), label }).unwrap();
    for _ in 0..262_144 {
        assembler.emit(&Inst::Nop).unwrap();
    }
    assembler.bind(label).unwrap();
    assert!(matches!(
        assembler.finish(),
        Err(ncl_asm_aarch64::EncodeError::RelocationOutOfRange { .. })
    ));
}

fn word_at(blob: &ncl_asm_aarch64::CodeBlob, offset: usize) -> u32 {
    u32::from_le_bytes(blob.bytes[offset..offset + 4].try_into().unwrap())
}

fn assert_negative_delta<F>(make: F, expected: u32)
where
    F: FnOnce(Label) -> Inst,
{
    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assembler.bind(label).unwrap();
    assembler.emit(&Inst::Nop).unwrap();
    assembler.emit(&make(label)).unwrap();
    assert_eq!(word_at(&assembler.finish().unwrap(), 4), expected);
}

fn assert_boundary_deltas<F>(make: F, max_delta: usize)
where
    F: Fn(Label) -> Inst + Copy,
{
    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assembler.emit(&make(label)).unwrap();
    for _ in 0..(max_delta / 4 - 1) {
        assembler.emit(&Inst::Nop).unwrap();
    }
    assembler.bind(label).unwrap();
    assert!(assembler.finish().is_ok());

    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assembler.emit(&make(label)).unwrap();
    for _ in 0..(max_delta / 4) {
        assembler.emit(&Inst::Nop).unwrap();
    }
    assembler.bind(label).unwrap();
    assert!(matches!(
        assembler.finish(),
        Err(ncl_asm_aarch64::EncodeError::RelocationOutOfRange { .. })
    ));

    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assembler.bind(label).unwrap();
    for _ in 0..=(max_delta / 4) {
        assembler.emit(&Inst::Nop).unwrap();
    }
    assembler.emit(&make(label)).unwrap();
    assert!(assembler.finish().is_ok());

    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assembler.bind(label).unwrap();
    for _ in 0..(max_delta / 4 + 2) {
        assembler.emit(&Inst::Nop).unwrap();
    }
    assembler.emit(&make(label)).unwrap();
    assert!(matches!(
        assembler.finish(),
        Err(ncl_asm_aarch64::EncodeError::RelocationOutOfRange { .. })
    ));
}

#[test]
fn conditional_fixups_encode_negative_four_bytes() {
    assert_negative_delta(
        |label| Inst::BCond {
            cond: Cond::Eq,
            label,
        },
        0x54ff_ffe0,
    );
    assert_negative_delta(|label| Inst::Cbz { rt: x(0), label }, 0xb4ff_ffe0);
    assert_negative_delta(|label| Inst::Cbnz { rt: x(0), label }, 0xb5ff_ffe0);
    assert_negative_delta(|label| Inst::LdrLiteral { rt: x(1), label }, 0x58ff_ffe1);
    assert_negative_delta(
        |label| Inst::Tbz {
            rt: x(0),
            bit: 0,
            label,
        },
        0x3607_ffe0,
    );
    assert_negative_delta(
        |label| Inst::Tbnz {
            rt: x(0),
            bit: 0,
            label,
        },
        0x3707_ffe0,
    );
}

#[test]
fn conditional_fixups_reject_deltas_just_past_each_signed_limit() {
    assert_boundary_deltas(
        |label| Inst::BCond {
            cond: Cond::Eq,
            label,
        },
        4 * ((1 << 18) - 1),
    );
    assert_boundary_deltas(|label| Inst::Cbz { rt: x(0), label }, 4 * ((1 << 18) - 1));
    assert_boundary_deltas(|label| Inst::Cbnz { rt: x(0), label }, 4 * ((1 << 18) - 1));
    assert_boundary_deltas(
        |label| Inst::LdrLiteral { rt: x(1), label },
        4 * ((1 << 18) - 1),
    );
    assert_boundary_deltas(
        |label| Inst::Tbz {
            rt: x(0),
            bit: 0,
            label,
        },
        4 * ((1 << 13) - 1),
    );
    assert_boundary_deltas(
        |label| Inst::Tbnz {
            rt: x(0),
            bit: 0,
            label,
        },
        4 * ((1 << 13) - 1),
    );
}

#[test]
fn instruction_families_encode() {
    use ncl_asm_aarch64::{Extend, MemOperand, VReg};

    let r0 = x(0);
    let r1 = x(1);
    let r2 = x(2);
    let sp = RegOrSp::Sp;
    let mem = MemOperand::Unscaled { base: sp, offset: 0 };
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
        Inst::Mov { rd: sp, rn: RegOrSp::Reg(r0) },
        Inst::SubImm { rd: sp, rn: sp, imm: 2, shift: true },
        Inst::Adds { rd: r0, rn: r1, rm: r2, shift: shifted },
        Inst::Subs { rd: r0, rn: r1, rm: r2, shift: shifted },
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
        Inst::Ldp { rt: r0, rt2: r1, mem },
        Inst::Stp { rt: r0, rt2: r1, mem },
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
        Inst::Mul { rd: r0, rn: r1, rm: r2 },
        Inst::Sdiv { rd: r0, rn: r1, rm: r2 },
        Inst::Udiv { rd: r0, rn: r1, rm: r2 },
        Inst::Madd { rd: r0, rn: r1, rm: r2, ra: r0 },
        Inst::Msub { rd: r0, rn: r1, rm: r2, ra: r0 },
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
        Inst::LslImm { rd: r0, rn: r1, amount: 3 },
        Inst::LsrImm { rd: r0, rn: r1, amount: 3 },
        Inst::AsrImm { rd: r0, rn: r1, amount: 3 },
        Inst::LslReg { rd: r0, rn: r1, rm: r2 },
        Inst::LsrReg { rd: r0, rn: r1, rm: r2 },
        Inst::AsrReg { rd: r0, rn: r1, rm: r2 },
        Inst::Fmov { rd: v0, rn: v1 },
        Inst::FmovGeneral { v: v0, r: r0, to_float: true },
        Inst::Fadd { rd: v0, rn: v1, rm: v0 },
        Inst::Fsub { rd: v0, rn: v1, rm: v0 },
        Inst::Fmul { rd: v0, rn: v1, rm: v0 },
        Inst::Fdiv { rd: v0, rn: v1, rm: v0 },
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
