#![allow(clippy::expect_used, clippy::too_many_lines)]

use super::{decode, Architecture};

fn aarch64_words(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

fn aarch64_encoded(instructions: &[ncl_asm_aarch64::Inst]) -> Vec<u8> {
    instructions
        .iter()
        .enumerate()
        .flat_map(|(offset, instruction)| {
            ncl_asm_aarch64::encode(instruction, offset * 4)
                .expect("encode")
                .to_le_bytes()
        })
        .collect()
}

#[test]
fn aarch64_decodes_encoded_instruction_families() {
    use ncl_asm_aarch64::{Extend, Inst, MemOperand, Reg, RegOrSp, Shift};

    let x0 = Reg(0);
    let x1 = Reg(1);
    let x2 = Reg(2);
    let sp = RegOrSp::Sp;
    let unsigned = MemOperand::Unsigned {
        base: sp,
        offset: 8,
        scale: 8,
    };
    let instructions = [
        Inst::DmbIsh,
        Inst::Brk { imm: 7 },
        Inst::Ret { rn: x0 },
        Inst::Br { rn: x1 },
        Inst::Blr { rn: x2 },
        Inst::MovZ {
            rd: x0,
            imm: 0x12,
            shift: 16,
        },
        Inst::MovK {
            rd: x0,
            imm: 0x34,
            shift: 32,
        },
        Inst::MovN {
            rd: x0,
            imm: 0x56,
            shift: 48,
        },
        Inst::AddImm {
            rd: sp,
            rn: sp,
            imm: 1,
            shift: true,
        },
        Inst::SubsImm {
            rd: sp,
            rn: sp,
            imm: 2,
            shift: false,
        },
        Inst::Mov {
            rd: RegOrSp::Reg(x0),
            rn: RegOrSp::Reg(x1),
        },
        Inst::Add {
            rd: RegOrSp::Reg(x0),
            rn: RegOrSp::Reg(x1),
            rm: x2,
            shift: Shift::Lsl(3),
        },
        Inst::Sub {
            rd: RegOrSp::Reg(x0),
            rn: RegOrSp::Reg(x1),
            rm: x2,
            shift: Shift::Lsr(2),
        },
        Inst::Adds {
            rd: x0,
            rn: x1,
            rm: x2,
            shift: Shift::Asr(1),
        },
        Inst::Subs {
            rd: x0,
            rn: x1,
            rm: x2,
            shift: Shift::Lsl(1),
        },
        Inst::Add {
            rd: sp,
            rn: sp,
            rm: x0,
            shift: Shift::Lsl(1),
        },
        Inst::And {
            rd: x0,
            rn: x1,
            rm: x2,
            shift: Shift::Lsr(1),
        },
        Inst::Orr {
            rd: x0,
            rn: x1,
            rm: x2,
            shift: Shift::Lsl(1),
        },
        Inst::Eor {
            rd: x0,
            rn: x1,
            rm: x2,
            shift: Shift::Asr(1),
        },
        Inst::Tst {
            rn: x1,
            rm: x2,
            shift: Shift::Lsl(1),
        },
        Inst::AndImm {
            rd: x0,
            rn: x1,
            imm: 0xff,
        },
        Inst::OrrImm {
            rd: x0,
            rn: x1,
            imm: 0xff,
        },
        Inst::EorImm {
            rd: x0,
            rn: x1,
            imm: 0xff,
        },
        Inst::TstImm { rn: x1, imm: 0xff },
        Inst::LslReg {
            rd: x0,
            rn: x1,
            rm: x2,
        },
        Inst::LsrReg {
            rd: x0,
            rn: x1,
            rm: x2,
        },
        Inst::AsrReg {
            rd: x0,
            rn: x1,
            rm: x2,
        },
        Inst::LslImm {
            rd: x0,
            rn: x1,
            amount: 3,
        },
        Inst::LsrImm {
            rd: x0,
            rn: x1,
            amount: 3,
        },
        Inst::AsrImm {
            rd: x0,
            rn: x1,
            amount: 3,
        },
        Inst::Csel {
            rd: x0,
            rn: x1,
            rm: x2,
            cond: ncl_asm_aarch64::Cond::Ne,
        },
        Inst::Mul {
            rd: x0,
            rn: x1,
            rm: x2,
        },
        Inst::Udiv {
            rd: x0,
            rn: x1,
            rm: x2,
        },
        Inst::Sdiv {
            rd: x0,
            rn: x1,
            rm: x2,
        },
        Inst::Ldr {
            rt: x0,
            mem: unsigned,
        },
        Inst::StrW {
            rt: x0,
            mem: MemOperand::Unsigned {
                base: sp,
                offset: 4,
                scale: 4,
            },
        },
        Inst::Ldrb {
            rt: x0,
            mem: MemOperand::Unsigned {
                base: sp,
                offset: 1,
                scale: 1,
            },
        },
        Inst::Ldrh {
            rt: x0,
            mem: MemOperand::Unsigned {
                base: sp,
                offset: 2,
                scale: 2,
            },
        },
        Inst::Ldr {
            rt: x0,
            mem: MemOperand::Unscaled {
                base: sp,
                offset: -8,
            },
        },
        Inst::Str {
            rt: x0,
            mem: MemOperand::PreIndex {
                base: sp,
                offset: 8,
            },
        },
        Inst::Ldr {
            rt: x0,
            mem: MemOperand::PostIndex {
                base: sp,
                offset: 8,
            },
        },
        Inst::Ldr {
            rt: x0,
            mem: MemOperand::Register {
                base: sp,
                index: x1,
                extend: None,
                shift: 0,
            },
        },
        Inst::Ldr {
            rt: x0,
            mem: MemOperand::Register {
                base: sp,
                index: x1,
                extend: Some(Extend::Uxtw),
                shift: 0,
            },
        },
        Inst::Ldr {
            rt: x0,
            mem: MemOperand::Register {
                base: sp,
                index: x1,
                extend: Some(Extend::Uxtw),
                shift: 8,
            },
        },
        Inst::Ldp {
            rt: x0,
            rt2: x1,
            mem: unsigned,
        },
        Inst::Stp {
            rt: x0,
            rt2: x1,
            mem: MemOperand::PreIndex {
                base: sp,
                offset: -8,
            },
        },
        Inst::Ldr {
            rt: x0,
            mem: unsigned,
        },
        Inst::Str {
            rt: x0,
            mem: unsigned,
        },
    ];
    let decoded =
        decode(Architecture::Aarch64, &aarch64_encoded(&instructions), 0).expect("decode");
    assert_eq!(decoded.len(), instructions.len());
    assert!(decoded.iter().all(|instruction| instruction.size == 4));
    assert!(decoded
        .iter()
        .all(|instruction| instruction.branch_target.is_none()));
    assert_eq!(decoded[0].text, "dmb ish");
    assert_eq!(decoded[1].text, "brk #7");
    assert_eq!(decoded[2].text, "ret x0");
    assert_eq!(decoded[3].text, "br x1");
    assert_eq!(decoded[4].text, "blr x2");
    assert_eq!(decoded[5].text, "movz x0, #0x12, lsl #16");
    assert_eq!(decoded[6].text, "movk x0, #0x34, lsl #32");
    assert_eq!(decoded[7].text, "movn x0, #0x56, lsl #48");
    assert_eq!(decoded[8].text, "add sp, sp, #4096 (lsl #12)");
    assert_eq!(decoded[9].text, "subs sp, sp, #2");
    assert_eq!(decoded[10].text, "orr x0, x31, x1, lsl #0");
    assert_eq!(decoded[11].text, "add x0, x1, x2, lsl #3");
    assert_eq!(decoded[12].text, "sub x0, x1, x2, lsl #2");
    assert_eq!(decoded[13].text, "adds x0, x1, x2, lsl #1");
    assert_eq!(decoded[14].text, "subs x0, x1, x2, lsl #1");
    assert_eq!(decoded[15].text, "add sp, sp, x0, lsl #1");
    assert_eq!(decoded[16].text, "and x0, x1, x2, lsl #1");
    assert_eq!(decoded[17].text, "orr x0, x1, x2, lsl #1");
    assert_eq!(decoded[18].text, "eor x0, x1, x2, lsl #1");
    assert_eq!(decoded[19].text, "tst xzr, x1, x2, lsl #1");
    assert_eq!(decoded[20].text, "and x0, x1, #0xff");
    assert_eq!(decoded[21].text, "orr x0, x1, #0xff");
    assert_eq!(decoded[22].text, "eor x0, x1, #0xff");
    assert_eq!(decoded[23].text, "tst xzr, x1, #0xff");
    assert_eq!(decoded[24].text, "lsl x0, x1, x2");
    assert_eq!(decoded[25].text, "lsr x0, x1, x2");
    assert_eq!(decoded[26].text, "asr x0, x1, x2");
    assert_eq!(decoded[27].text, "lsl x0, x1, #3");
    assert_eq!(decoded[28].text, "lsr x0, x1, #3");
    assert_eq!(decoded[29].text, "asr x0, x1, #3");
    assert_eq!(decoded[30].text, "csel x0, x1, x2, ne");
    assert_eq!(decoded[31].text, "mul x0, x1, x2");
    assert_eq!(decoded[32].text, "udiv x0, x1, x2");
    assert_eq!(decoded[33].text, "sdiv x0, x1, x2");
    assert_eq!(decoded[34].text, "ldr x0, [sp, #8]");
    assert_eq!(decoded[35].text, "str w0, [sp, #4]");
    assert_eq!(decoded[36].text, "ldr w0, [sp, #1]");
    assert_eq!(decoded[37].text, "ldr w0, [sp, #2]");
    assert_eq!(decoded[38].text, "ldr x0, [sp, #-8]");
    assert_eq!(decoded[39].text, "str x0, [sp, #8]!");
    assert_eq!(decoded[40].text, "ldr x0, [sp], #8");
    assert_eq!(decoded[41].text, "ldr x0, [sp, x1]");
    assert_eq!(decoded[42].text, "ldr x0, [sp, x1, uxtw]");
    assert_eq!(decoded[43].text, "ldr x0, [sp, x1, uxtw #8]");
    assert_eq!(decoded[44].text, "ldp x0, x1, [sp, #8]");
    assert_eq!(decoded[45].text, "stp x0, x1, [sp, #-8]!");
    assert_eq!(decoded[46].text, "ldr x0, [sp, #8]");
    assert_eq!(decoded[47].text, "str x0, [sp, #8]");
    let floating_point = decode(
        Architecture::Aarch64,
        &aarch64_words(&[0xfd40_07e0, 0xfd00_07e0]),
        0,
    )
    .expect("decode");
    assert_eq!(floating_point[0].text, "ldr d0, [sp, #8]");
    assert_eq!(floating_point[1].text, "str d0, [sp, #8]");
}
