use crate::assembler::{FixupKind, Label};
use crate::{EncodeError, Inst, Reg, RegOrSp, Shift, VReg};

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
            let base_opcode = match (size, load) {
                (1, true) => 0x39400000,
                (1, false) => 0x39000000,
                (2, true) => 0x79400000,
                (2, false) => 0x79000000,
                (4, true) => 0xB9400000,
                (4, false) => 0xB9000000,
                (_, true) => 0xF9400000,
                (_, false) => 0xF9000000,
            };
            Ok(base_opcode | off << 10 | base << 5 | rt_number)
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
            let base_opcode = match (size, load) {
                (1, true) => 0x38400000,
                (1, false) => 0x38000000,
                (2, true) => 0x78400000,
                (2, false) => 0x78000000,
                (4, true) => 0xB8400000,
                (4, false) => 0xB8000000,
                (_, true) => 0xF8400000,
                (_, false) => 0xF8000000,
            };
            Ok(base_opcode | ((o as u32) & 0x1ff) << 12 | mode << 10 | base << 5 | rt_number)
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
            let base_opcode = match (size, load) {
                (1, true) => 0x38600800,
                (1, false) => 0x38200800,
                (2, true) => 0x78600800,
                (2, false) => 0x78200800,
                (4, true) => 0xB8600800,
                (4, false) => 0xB8200800,
                (_, true) => 0xF8600800,
                (_, false) => 0xF8200800,
            };
            if *shift % size != 0 || *shift / size > 1 {
                return Err(EncodeError::ImmediateOutOfRange {
                    value: i64::from(*shift),
                    bits: 1,
                });
            }
            Ok(base_opcode
                | r(*index) << 16
                | option << 13
                | (*shift as u32 / size as u32) << 12
                | base << 5
                | rt_number)
        }
    }
}
fn pair(m: &crate::MemOperand, rt: Reg, rt2: Reg, load: bool) -> Result<u32, EncodeError> {
    let (base, offset, mode) = match m {
        crate::MemOperand::Unsigned {
            base,
            offset,
            scale,
        } if *scale == 8 && *offset % 8 == 0 => (rs(*base), i64::from(*offset / 8), 0),
        crate::MemOperand::Unscaled { base, offset } => (rs(*base), i64::from(*offset), 0),
        crate::MemOperand::PreIndex { base, offset } => (rs(*base), i64::from(*offset), 3),
        crate::MemOperand::PostIndex { base, offset } => (rs(*base), i64::from(*offset), 1),
        _ => return Err(EncodeError::ImmediateOutOfRange { value: 0, bits: 7 }),
    };
    if !(-64..=63).contains(&offset) {
        return Err(EncodeError::ImmediateOutOfRange {
            value: offset,
            bits: 7,
        });
    }
    Ok((if load { 0xA9400000 } else { 0xA9000000 })
        | ((offset as u32) & 0x7f) << 15
        | u32::from(rt2.0) << 10
        | base << 5
        | u32::from(rt.0)
        | mode << 23)
}
fn ext3(
    base: u32,
    rd: RegOrSp,
    rn: RegOrSp,
    rm: Reg,
    e: crate::Extend,
    n: u8,
) -> Result<u32, EncodeError> {
    if n > 4 {
        return Err(EncodeError::ImmediateOutOfRange {
            value: i64::from(n),
            bits: 3,
        });
    }
    let option = match e {
        crate::Extend::Uxtb => 0,
        crate::Extend::Uxth => 1,
        crate::Extend::Uxtw => 2,
        crate::Extend::Uxtx => 3,
        crate::Extend::Sxtb => 4,
        crate::Extend::Sxth => 5,
        crate::Extend::Sxtw => 6,
        crate::Extend::Sxtx => 7,
    };
    Ok(base | r(rm) << 16 | option << 13 | u32::from(n) << 10 | rs(rn) << 5 | rs(rd))
}
fn logical_imm(base: u32, rd: Reg, rn: Reg, value: u64) -> Result<u32, EncodeError> {
    let (n, immr, imms) =
        encode_bitmask(value).ok_or(EncodeError::InvalidBitmaskImmediate(value))?;
    Ok(base | n << 22 | immr << 16 | imms << 10 | r(rn) << 5 | r(rd))
}
fn encode_bitmask(value: u64) -> Option<(u32, u32, u32)> {
    if value == 0 || value == u64::MAX {
        return None;
    }
    for width in [2_u32, 4, 8, 16, 32, 64] {
        for ones in 1..width {
            let pattern = if ones == 64 {
                u64::MAX
            } else {
                (1_u64 << ones) - 1
            };
            for rot in 0..width {
                let element = pattern.rotate_right(rot);
                let mut result = 0_u64;
                for pos in (0..64).step_by(width as usize) {
                    result |= element << pos;
                }
                if result == value {
                    let imms = ((!(width - 1) & 63) | (ones - 1)) & 63;
                    return Some((u32::from(width == 64), rot, imms));
                }
            }
        }
    }
    None
}
fn bit_shift(base: u32, rd: Reg, rn: Reg, amount: u8) -> Result<u32, EncodeError> {
    if amount > 63 {
        return Err(EncodeError::ImmediateOutOfRange {
            value: i64::from(amount),
            bits: 6,
        });
    }
    Ok(base | u32::from(63 - amount) << 16 | u32::from(amount) << 10 | r(rn) << 5 | r(rd))
}
fn test_branch(base: u32, rt: Reg, bit: u8) -> Result<u32, EncodeError> {
    if bit > 63 {
        return Err(EncodeError::ImmediateOutOfRange {
            value: i64::from(bit),
            bits: 6,
        });
    }
    Ok(base | u32::from(bit >> 5) << 31 | u32::from(bit & 31) << 19 | r(rt))
}
fn float2(base: u32, rd: crate::VReg, rn: crate::VReg) -> Result<u32, EncodeError> {
    if rd.number > 31 || rn.number > 31 || rd.double != rn.double {
        return Err(EncodeError::InvalidRegister(rd.number));
    }
    Ok(base | u32::from(rn.number) << 5 | u32::from(rd.number))
}
fn float3(
    base: u32,
    rd: crate::VReg,
    rn: crate::VReg,
    rm: crate::VReg,
) -> Result<u32, EncodeError> {
    if rd.number > 31
        || rn.number > 31
        || rm.number > 31
        || rd.double != rn.double
        || rd.double != rm.double
    {
        return Err(EncodeError::InvalidRegister(rd.number));
    }
    Ok(base | u32::from(rm.number) << 16 | u32::from(rn.number) << 5 | u32::from(rd.number))
}
fn float_mem(m: &crate::MemOperand, rt: crate::VReg, load: bool) -> Result<u32, EncodeError> {
    let base = match m {
        crate::MemOperand::Unsigned {
            base,
            offset,
            scale,
        } if *scale == 8 && *offset % 8 == 0 => {
            0xFD000000 | u32::from(*offset / 8) << 10 | rs(*base)
        }
        _ => return Err(EncodeError::ImmediateOutOfRange { value: 0, bits: 12 }),
    };
    Ok(base | u32::from(rt.number) | if load { 0x4000000 } else { 0 })
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
        Inst::Adds {
            rd,
            rn,
            rm,
            shift: s,
        } => reg3(0xAB000000, *rd, RegOrSp::Reg(*rn), *rm, *s),
        Inst::Subs {
            rd,
            rn,
            rm,
            shift: s,
        } => reg3(0xEB000000, *rd, RegOrSp::Reg(*rn), *rm, *s),
        Inst::AddExt {
            rd,
            rn,
            rm,
            extend: e,
            shift: n,
        } => ext3(0x8B000000, *rd, *rn, *rm, *e, *n),
        Inst::SubExt {
            rd,
            rn,
            rm,
            extend: e,
            shift: n,
        } => ext3(0xCB000000, *rd, *rn, *rm, *e, *n),
        Inst::Ldr { rt, mem: m } => mem(m, 8, true, *rt),
        Inst::Str { rt, mem: m } => mem(m, 8, false, *rt),
        Inst::LdrW { rt, mem: m } => mem(m, 4, true, *rt),
        Inst::StrW { rt, mem: m } => mem(m, 4, false, *rt),
        Inst::Ldrb { rt, mem: m } => mem(m, 1, true, *rt),
        Inst::Strb { rt, mem: m } => mem(m, 1, false, *rt),
        Inst::Ldrh { rt, mem: m } => mem(m, 2, true, *rt),
        Inst::Strh { rt, mem: m } => mem(m, 2, false, *rt),
        Inst::Ldrsw { rt, mem: m } => mem(m, 4, true, *rt),
        Inst::Ldp { rt, rt2, mem: m } => pair(m, *rt, *rt2, true),
        Inst::Stp { rt, rt2, mem: m } => pair(m, *rt, *rt2, false),
        Inst::LdrLiteral { rt, .. } => Ok(0x58000000 | r(*rt)),
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
        Inst::Cset { rd, cond } => Ok(0x9A9F07E0 | cond.bits() << 12 | r(*rd)),
        Inst::Cinc { rd, rn, cond } => {
            Ok(0x9A800400 | r(*rn) << 5 | (!cond.bits() & 0xf) << 12 | r(*rd))
        }
        Inst::Mul { rd, rn, rm } => Ok(0x9B007C00 | r(*rm) << 16 | r(*rn) << 5 | r(*rd)),
        Inst::Sdiv { rd, rn, rm } => Ok(0x9AC00C00 | r(*rm) << 16 | r(*rn) << 5 | r(*rd)),
        Inst::Udiv { rd, rn, rm } => Ok(0x9AC00800 | r(*rm) << 16 | r(*rn) << 5 | r(*rd)),
        Inst::Madd { rd, rn, rm, ra } => {
            Ok(0x9B000000 | r(*rm) << 16 | r(*ra) << 10 | r(*rn) << 5 | r(*rd))
        }
        Inst::Msub { rd, rn, rm, ra } => {
            Ok(0x9B008000 | r(*rm) << 16 | r(*ra) << 10 | r(*rn) << 5 | r(*rd))
        }
        Inst::Neg { rd, rn, shift: s } => reg3(0xCB0003E0, *rd, RegOrSp::Reg(Reg(31)), *rn, *s),
        Inst::Mvn { rd, rn, shift: s } => reg3(0xAA2003E0, *rd, RegOrSp::Reg(Reg(31)), *rn, *s),
        Inst::AddsImm {
            rd,
            rn,
            imm,
            shift: s,
        } => addsub(*rd, *rn, *imm, *s, false).map(|w| w | 1 << 29),
        Inst::SubsImm {
            rd,
            rn,
            imm,
            shift: s,
        } => addsub(*rd, *rn, *imm, *s, true).map(|w| w | 1 << 29),
        Inst::Cmn { rn, rm, shift: s } => reg3(0xAB00001F, Reg(31), RegOrSp::Reg(*rn), *rm, *s),
        Inst::AndImm { rd, rn, imm: v } => logical_imm(0x92000000, *rd, *rn, *v),
        Inst::OrrImm { rd, rn, imm: v } => logical_imm(0xB2000000, *rd, *rn, *v),
        Inst::EorImm { rd, rn, imm: v } => logical_imm(0xD2000000, *rd, *rn, *v),
        Inst::TstImm { rn, imm: v } => logical_imm(0xF200001F, Reg(31), *rn, *v),
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
        Inst::LslImm { rd, rn, amount } => bit_shift(0xD3400000, *rd, *rn, *amount),
        Inst::LsrImm { rd, rn, amount } => bit_shift(0xD340FC00, *rd, *rn, *amount),
        Inst::AsrImm { rd, rn, amount } => bit_shift(0x9340FC00, *rd, *rn, *amount),
        Inst::LslReg { rd, rn, rm } => Ok(0x9AC02000 | r(*rm) << 16 | r(*rn) << 5 | r(*rd)),
        Inst::LsrReg { rd, rn, rm } => Ok(0x9AC02400 | r(*rm) << 16 | r(*rn) << 5 | r(*rd)),
        Inst::AsrReg { rd, rn, rm } => Ok(0x9AC02800 | r(*rm) << 16 | r(*rn) << 5 | r(*rd)),
        Inst::Tbz { rt, bit, .. } => test_branch(0x36000000, *rt, *bit),
        Inst::Tbnz { rt, bit, .. } => test_branch(0x37000000, *rt, *bit),
        Inst::Fmov { rd, rn } => float2(0x1E604000, *rd, *rn),
        Inst::FmovGeneral { v, r: g, to_float } => {
            Ok(
                (if *to_float { 0x9E670000 } else { 0x9E660000 })
                    | r(*g)
                    | u32::from(v.number) << 5,
            )
        }
        Inst::Fadd { rd, rn, rm } => float3(0x1E602800, *rd, *rn, *rm),
        Inst::Fsub { rd, rn, rm } => float3(0x1E603800, *rd, *rn, *rm),
        Inst::Fmul { rd, rn, rm } => float3(0x1E600800, *rd, *rn, *rm),
        Inst::Fdiv { rd, rn, rm } => float3(0x1E601800, *rd, *rn, *rm),
        Inst::Fsqrt { rd, rn } => float2(0x1E61C000, *rd, *rn),
        Inst::Fneg { rd, rn } => float2(0x1E614000, *rd, *rn),
        Inst::Fcmp { rn, rm } => float3(
            0x1E602000,
            VReg {
                number: 0,
                double: rn.double,
            },
            *rn,
            *rm,
        ),
        Inst::Scvtf { rd, rn } => Ok(0x9E620000 | r(*rn) | u32::from(rd.number) << 5),
        Inst::Fcvtzs { rd, rn } => Ok(0x9E780000 | r(*rd) | u32::from(rn.number) << 5),
        Inst::LdrD { rt, mem: m } => float_mem(m, *rt, true),
        Inst::StrD { rt, mem: m } => float_mem(m, *rt, false),
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
    if im > 4095 {
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
        Inst::Adrp { label, .. } => Some((FixupKind::Adrp21, *label)),
        Inst::Adr { label, .. } => Some((FixupKind::Adr21, *label)),
        Inst::Cbz { label, .. } | Inst::Cbnz { label, .. } => {
            Some((FixupKind::CondBranch19, *label))
        }
        Inst::Tbz { label, .. } | Inst::Tbnz { label, .. } => {
            Some((FixupKind::TestBranch14, *label))
        }
        Inst::LdrLiteral { label, .. } => Some((FixupKind::Literal19, *label)),
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
