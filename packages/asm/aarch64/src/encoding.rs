use crate::assembler::{FixupKind, Label};
use crate::{EncodeError, Inst, Reg, RegOrSp, Shift};

fn r(r: Reg) -> u32 {
    r.0 as u32
}
fn rs(value: RegOrSp) -> u32 {
    match value {
        RegOrSp::Reg(x) => r(x),
        RegOrSp::Sp => 31,
    }
}
fn shift(s: Shift) -> (u32, u32) {
    match s {
        Shift::Lsl(n) => (0, n as u32),
        Shift::Lsr(n) => (1, n as u32),
        Shift::Asr(n) => (2, n as u32),
    }
}
fn imm(value: i64, bits: u8) -> Result<u32, EncodeError> {
    if value < 0 || value >= 1_i64 << bits {
        Err(EncodeError::ImmediateOutOfRange { value, bits })
    } else {
        Ok(value as u32)
    }
}
fn mem(m: &crate::MemOperand, size: u8, load: bool, rt: Reg) -> Result<u32, EncodeError> {
    let base = match m {
        crate::MemOperand::Unsigned { base, .. }
        | crate::MemOperand::Unscaled { base, .. }
        | crate::MemOperand::PreIndex { base, .. }
        | crate::MemOperand::PostIndex { base, .. }
        | crate::MemOperand::Register { base, .. } => rs(*base),
    };
    let rt_number = r(rt);
    match m {
        crate::MemOperand::Unsigned { offset, scale, .. } => {
            if *scale != size || (*offset as u32) % (*scale as u32) != 0 {
                return Err(EncodeError::ImmediateOutOfRange {
                    value: i64::from(*offset),
                    bits: 12,
                });
            };
            let off = imm(i64::from(*offset) / i64::from(*scale), 12)?;
            Ok((if load { 0x39400000 } else { 0x39000000 }) | off << 10 | base << 5 | rt_number)
        }
        crate::MemOperand::Unscaled { offset, .. }
        | crate::MemOperand::PreIndex { offset, .. }
        | crate::MemOperand::PostIndex { offset, .. } => {
            let o = *offset as i64;
            if !(-256..=255).contains(&o) {
                return Err(EncodeError::ImmediateOutOfRange { value: o, bits: 9 });
            };
            let mode = match m {
                crate::MemOperand::PreIndex { .. } => 3,
                crate::MemOperand::PostIndex { .. } => 1,
                _ => 0,
            };
            Ok((if load { 0x38400000 } else { 0x38000000 })
                | ((o as u32) & 0x1ff) << 12
                | mode << 10
                | base << 5
                | rt_number)
        }
        crate::MemOperand::Register {
            index,
            extend,
            shift,
            ..
        } => {
            let option = match extend {
                None => 3,
                Some(crate::Extend::Uxtw) => 2,
                Some(crate::Extend::Uxtx) => 3,
                Some(crate::Extend::Sxtw) => 6,
                Some(crate::Extend::Sxtx) => 7,
                _ => 0,
            };
            Ok((if load { 0x38600800 } else { 0x38200800 })
                | r(*index) << 16
                | option << 13
                | (*shift as u32 / size as u32) << 12
                | base << 5
                | rt_number)
        }
    }
}
/// Encodes one instruction as a little-endian 32-bit word.
pub fn encode(i: &Inst, _at: usize) -> Result<u32, EncodeError> {
    match i {
        Inst::MovZ { rd, imm, shift: s } => wide(0xD2800000, *rd, *imm, *s, 0),
        Inst::MovK { rd, imm, shift: s } => wide(0xF2800000, *rd, *imm, *s, 1),
        Inst::MovN { rd, imm, shift: s } => wide(0x92800000, *rd, *imm, *s, 0),
        Inst::Mov { rd, rn } => Ok(0xAA0003E0 | rs(*rn) << 16 | rs(*rd)),
        Inst::AddImm {
            rd,
            rn,
            imm,
            shift: s,
        } => addsub(*rd, *rn, *imm, *s, false),
        Inst::SubImm {
            rd,
            rn,
            imm,
            shift: s,
        } => addsub(*rd, *rn, *imm, *s, true),
        Inst::Add {
            rd,
            rn,
            rm,
            shift: s,
        } => reg3(0x8B000000, *rd, *rn, *rm, *s),
        Inst::Sub {
            rd,
            rn,
            rm,
            shift: s,
        } => reg3(0xCB000000, *rd, *rn, *rm, *s),
        Inst::Ldr { rt, mem: m } => mem(m, 8, true, *rt),
        Inst::Str { rt, mem: m } => mem(m, 8, false, *rt),
        Inst::LdrW { rt, mem: m } => mem(m, 4, true, *rt),
        Inst::StrW { rt, mem: m } => mem(m, 4, false, *rt),
        Inst::B { .. } => Ok(0x14000000),
        Inst::Bl { .. } => Ok(0x94000000),
        Inst::BCond { cond, .. } => Ok(0x54000000 | cond.bits()),
        Inst::Cbz { rt, .. } => Ok(0xB4000000 | r(*rt)),
        Inst::Cbnz { rt, .. } => Ok(0xB5000000 | r(*rt)),
        Inst::Adr { rd, .. } => Ok(0x10000000 | r(*rd)),
        Inst::Adrp { rd, .. } => Ok(0x90000000 | r(*rd)),
        Inst::Ret { rn } => Ok(0xD65F0000 | r(*rn) << 5),
        Inst::Br { rn } => Ok(0xD61F0000 | r(*rn) << 5),
        Inst::Blr { rn } => Ok(0xD63F0000 | r(*rn) << 5),
        Inst::Nop => Ok(0xD503201F),
        Inst::Brk { imm } => Ok(0xD4200000 | u32::from(*imm) << 5),
        Inst::Udf { imm } => Ok(0x00000000 | u32::from(*imm)),
        Inst::DmbIsh => Ok(0xD5033BBF),
        Inst::Cmp { rn, rm, shift: s } => reg3(0xEB00001F, Reg(31), RegOrSp::Reg(*rn), *rm, *s),
        Inst::Csel { rd, rn, rm, cond } => {
            Ok(0x9A800000 | r(*rm) << 16 | cond.bits() << 12 | r(*rn) << 5 | r(*rd))
        }
        Inst::Mul { rd, rn, rm } => Ok(0x9B007C00 | r(*rm) << 16 | r(*rn) << 5 | r(*rd)),
        Inst::Sdiv { rd, rn, rm } => Ok(0x9AC00C00 | r(*rm) << 16 | r(*rn) << 5 | r(*rd)),
        Inst::Udiv { rd, rn, rm } => Ok(0x9AC00800 | r(*rm) << 16 | r(*rn) << 5 | r(*rd)),
        Inst::And {
            rd,
            rn,
            rm,
            shift: s,
        } => reg3(0x8A000000, *rd, RegOrSp::Reg(*rn), *rm, *s),
        Inst::Orr {
            rd,
            rn,
            rm,
            shift: s,
        } => reg3(0xAA000000, *rd, RegOrSp::Reg(*rn), *rm, *s),
        Inst::Eor {
            rd,
            rn,
            rm,
            shift: s,
        } => reg3(0xCA000000, *rd, RegOrSp::Reg(*rn), *rm, *s),
        Inst::Tst { rn, rm, shift: s } => reg3(0xEA00001F, Reg(31), RegOrSp::Reg(*rn), *rm, *s),
    }
}
fn wide(base: u32, rd: Reg, immv: u16, s: u8, _k: u8) -> Result<u32, EncodeError> {
    if s % 16 != 0 || s > 48 {
        return Err(EncodeError::ImmediateOutOfRange {
            value: i64::from(s),
            bits: 6,
        });
    }
    Ok(base | u32::from(immv) << 5 | u32::from(s / 16) << 21 | rs(rd.into()))
}
fn addsub(rd: RegOrSp, rn: RegOrSp, im: u16, s: bool, sub: bool) -> Result<u32, EncodeError> {
    if s && im > 4095 {
        return Err(EncodeError::ImmediateOutOfRange {
            value: i64::from(im),
            bits: 12,
        });
    }
    Ok((if sub { 0xD1000000 } else { 0x91000000 })
        | if s { 1 << 22 } else { 0 }
        | u32::from(im) << 10
        | rs(rn) << 5
        | rs(rd))
}
fn reg3<R: Into<RegOrSp>>(
    base: u32,
    rd: R,
    rn: RegOrSp,
    rm: Reg,
    s: Shift,
) -> Result<u32, EncodeError> {
    let (k, n) = shift(s);
    if n > 63 {
        return Err(EncodeError::ImmediateOutOfRange {
            value: i64::from(n),
            bits: 6,
        });
    }
    Ok(base | r(rm) << 16 | k << 22 | n << 10 | rs(rn) << 5 | rs(rd.into()))
}
pub(crate) fn fixup(i: &Inst) -> Option<(FixupKind, Label)> {
    match i {
        Inst::B { label } => Some((FixupKind::Branch26, *label)),
        Inst::Bl { label } => Some((FixupKind::Branch26, *label)),
        Inst::BCond { label, .. } => Some((FixupKind::CondBranch19, *label)),
        Inst::Adrp { label, .. } | Inst::Adr { label, .. } => Some((FixupKind::Adrp21, *label)),
        Inst::Cbz { label, .. } | Inst::Cbnz { label, .. } => {
            Some((FixupKind::CondBranch19, *label))
        }
        _ => None,
    }
}
/// Decodes one supported word.
pub fn decode(word: u32) -> Result<Inst, EncodeError> {
    match word {
        0xD503201F => Ok(Inst::Nop),
        0xD65F03C0 => Ok(Inst::Ret { rn: Reg(30) }),
        _ => Err(EncodeError::UnsupportedInstruction(word)),
    }
}
