use crate::{EncodeError, Inst, Reg, RegOrSp, VReg};
use encoding_helpers::{addsub, reg3, wide};

#[path = "encoding_helpers.rs"]
mod encoding_helpers;
fn r(r: Reg) -> u32 {
    u32::from(r.0)
}
fn rs(value: RegOrSp) -> u32 {
    match value {
        RegOrSp::Reg(x) => r(x),
        RegOrSp::Sp => 31,
    }
}
fn imm(value: i64, bits: u8) -> Result<u32, EncodeError> {
    if value < 0 || value >= 1_i64 << bits {
        Err(EncodeError::ImmediateOutOfRange { value, bits })
    } else {
        u32::try_from(value).map_or(Err(EncodeError::ImmediateOutOfRange { value, bits }), Ok)
    }
}
#[allow(
    clippy::too_many_lines,
    reason = "Memory addressing forms share one validated encoder."
)]
fn mem(
    m: crate::MemOperand,
    size: u8,
    load: bool,
    signed_word: bool,
    rt: Reg,
) -> Result<u32, EncodeError> {
    let base = match m {
        crate::MemOperand::Unsigned { base, .. }
        | crate::MemOperand::Unscaled { base, .. }
        | crate::MemOperand::PreIndex { base, .. }
        | crate::MemOperand::PostIndex { base, .. }
        | crate::MemOperand::Register { base, .. } => rs(base),
    };
    let rt_number = r(rt);
    match m {
        crate::MemOperand::Unsigned { offset, scale, .. } => {
            if scale != size || !u32::from(offset).is_multiple_of(u32::from(scale)) {
                return Err(EncodeError::ImmediateOutOfRange {
                    value: i64::from(offset),
                    bits: 12,
                });
            }
            let off = imm(i64::from(offset) / i64::from(scale), 12)?;
            let base_opcode = match (size, load) {
                (1, true) => 0x3940_0000,
                (1, false) => 0x3900_0000,
                (2, true) => 0x7940_0000,
                (2, false) => 0x7900_0000,
                (4, true) => 0xB940_0000,
                (4, false) => 0xB900_0000,
                (_, true) => 0xF940_0000,
                (_, false) => 0xF900_0000,
            };
            Ok(base_opcode
                | if signed_word { 0x0040_0000 } else { 0 }
                | off << 10
                | base << 5
                | rt_number)
        }
        crate::MemOperand::Unscaled { offset, .. }
        | crate::MemOperand::PreIndex { offset, .. }
        | crate::MemOperand::PostIndex { offset, .. } => {
            let o = i64::from(offset);
            if !(-256..=255).contains(&o) {
                return Err(EncodeError::ImmediateOutOfRange { value: o, bits: 9 });
            }
            let mode = match m {
                crate::MemOperand::PreIndex { .. } => 3,
                crate::MemOperand::PostIndex { .. } => 1,
                _ => 0,
            };
            let base_opcode = match (size, load) {
                (1, true) => 0x3840_0000,
                (1, false) => 0x3800_0000,
                (2, true) => 0x7840_0000,
                (2, false) => 0x7800_0000,
                (4, true) => 0xB840_0000,
                (4, false) => 0xB800_0000,
                (_, true) => 0xF840_0000,
                (_, false) => 0xF800_0000,
            };
            let Ok(encoded) = u32::try_from(o) else {
                return Err(EncodeError::ImmediateOutOfRange { value: o, bits: 9 });
            };
            Ok(base_opcode
                | if signed_word { 0x0040_0000 } else { 0 }
                | (encoded & 0x1ff) << 12
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
                None | Some(crate::Extend::Uxtx) => 3,
                Some(crate::Extend::Uxtw) => 2,
                Some(crate::Extend::Sxtw) => 6,
                Some(crate::Extend::Sxtx) => 7,
                _ => 0,
            };
            let base_opcode = match (size, load) {
                (1, true) => 0x3860_0800,
                (1, false) => 0x3820_0800,
                (2, true) => 0x7860_0800,
                (2, false) => 0x7820_0800,
                (4, true) => 0xB860_0800,
                (4, false) => 0xB820_0800,
                (_, true) => 0xF860_0800,
                (_, false) => 0xF820_0800,
            };
            if !shift.is_multiple_of(size) || shift / size > 1 {
                return Err(EncodeError::ImmediateOutOfRange {
                    value: i64::from(shift),
                    bits: 1,
                });
            }
            Ok(base_opcode
                | r(index) << 16
                | option << 13
                | (u32::from(shift) / u32::from(size)) << 12
                | base << 5
                | rt_number)
        }
    }
}
fn pair(m: crate::MemOperand, rt: Reg, rt2: Reg, load: bool) -> Result<u32, EncodeError> {
    let (base, offset, mode) = match m {
        crate::MemOperand::Unsigned {
            base,
            offset,
            scale,
        } if scale == 8 && offset % 8 == 0 => (rs(base), i64::from(offset / 8), 0),
        crate::MemOperand::Unscaled { base, offset }
        | crate::MemOperand::PreIndex { base, offset }
        | crate::MemOperand::PostIndex { base, offset }
            if offset % 8 == 0 =>
        {
            let mode = match m {
                crate::MemOperand::PreIndex { .. } => 3,
                crate::MemOperand::PostIndex { .. } => 1,
                _ => 0,
            };
            (rs(base), i64::from(offset / 8), mode)
        }
        _ => return Err(EncodeError::ImmediateOutOfRange { value: 0, bits: 7 }),
    };
    if !(-64..=63).contains(&offset) {
        return Err(EncodeError::ImmediateOutOfRange {
            value: offset,
            bits: 7,
        });
    }
    let Ok(offset) = i32::try_from(offset) else {
        return Err(EncodeError::ImmediateOutOfRange {
            value: offset,
            bits: 7,
        });
    };
    let encoded = u32::from_ne_bytes(offset.to_ne_bytes());
    Ok((if load { 0xA940_0000 } else { 0xA900_0000 })
        | (encoded & 0x7f) << 15
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
    let (n_bit, rotate, mask) =
        encode_bitmask(value).ok_or(EncodeError::InvalidBitmaskImmediate(value))?;
    Ok(base | n_bit << 22 | rotate << 16 | mask << 10 | r(rn) << 5 | r(rd))
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
fn float_mem(m: crate::MemOperand, rt: crate::VReg, load: bool) -> Result<u32, EncodeError> {
    let base = match m {
        crate::MemOperand::Unsigned {
            base,
            offset,
            scale,
        } if scale == 8 && offset % 8 == 0 => 0xFD00_0000 | u32::from(offset / 8) << 10 | rs(base),
        _ => return Err(EncodeError::ImmediateOutOfRange { value: 0, bits: 12 }),
    };
    Ok(base | u32::from(rt.number) | if load { 0x0400_0000 } else { 0 })
}
/// Encodes one instruction as a little-endian 32-bit word.
///
/// # Errors
///
/// Returns an error when an operand cannot be represented by the instruction.
#[allow(
    clippy::too_many_lines,
    reason = "The closed instruction enum is dispatched in one encoding function."
)]
pub fn encode(i: &Inst, _at: usize) -> Result<u32, EncodeError> {
    match i {
        Inst::MovZ { rd, imm, shift: s } => wide(0xD280_0000, *rd, *imm, *s, 0),
        Inst::MovK { rd, imm, shift: s } => wide(0xF280_0000, *rd, *imm, *s, 1),
        Inst::MovN { rd, imm, shift: s } => wide(0x9280_0000, *rd, *imm, *s, 0),
        Inst::Mov { rd, rn } => Ok(0xAA00_03E0 | rs(*rn) << 16 | rs(*rd)),
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
        } => reg3(0x8B00_0000, *rd, *rn, *rm, *s),
        Inst::Sub {
            rd,
            rn,
            rm,
            shift: s,
        } => reg3(0xCB00_0000, *rd, *rn, *rm, *s),
        Inst::Adds {
            rd,
            rn,
            rm,
            shift: s,
        } => reg3(0xAB00_0000, *rd, RegOrSp::Reg(*rn), *rm, *s),
        Inst::Subs {
            rd,
            rn,
            rm,
            shift: s,
        } => reg3(0xEB00_0000, *rd, RegOrSp::Reg(*rn), *rm, *s),
        Inst::AddExt {
            rd,
            rn,
            rm,
            extend: e,
            shift: n,
        } => ext3(0x8B00_0000, *rd, *rn, *rm, *e, *n),
        Inst::SubExt {
            rd,
            rn,
            rm,
            extend: e,
            shift: n,
        } => ext3(0xCB00_0000, *rd, *rn, *rm, *e, *n),
        Inst::Ldr { rt, mem: m } => mem(*m, 8, true, false, *rt),
        Inst::Str { rt, mem: m } => mem(*m, 8, false, false, *rt),
        Inst::LdrW { rt, mem: m } => mem(*m, 4, true, false, *rt),
        Inst::Ldrsw { rt, mem: m } => mem(*m, 4, true, true, *rt),
        Inst::StrW { rt, mem: m } => mem(*m, 4, false, false, *rt),
        Inst::Ldrb { rt, mem: m } => mem(*m, 1, true, false, *rt),
        Inst::Strb { rt, mem: m } => mem(*m, 1, false, false, *rt),
        Inst::Ldrh { rt, mem: m } => mem(*m, 2, true, false, *rt),
        Inst::Strh { rt, mem: m } => mem(*m, 2, false, false, *rt),
        Inst::Ldp { rt, rt2, mem: m } => pair(*m, *rt, *rt2, true),
        Inst::Stp { rt, rt2, mem: m } => pair(*m, *rt, *rt2, false),
        Inst::LdrLiteral { rt, .. } => Ok(0x5800_0000 | r(*rt)),
        Inst::B { .. } => Ok(0x1400_0000),
        Inst::Bl { .. } => Ok(0x9400_0000),
        Inst::BCond { cond, .. } => Ok(0x5400_0000 | cond.bits()),
        Inst::Cbz { rt, .. } => Ok(0xB400_0000 | r(*rt)),
        Inst::Cbnz { rt, .. } => Ok(0xB500_0000 | r(*rt)),
        Inst::Adr { rd, .. } => Ok(0x1000_0000 | r(*rd)),
        Inst::Adrp { rd, .. } => Ok(0x9000_0000 | r(*rd)),
        Inst::Ret { rn } => Ok(0xD65F_0000 | r(*rn) << 5),
        Inst::Br { rn } => Ok(0xD61F_0000 | r(*rn) << 5),
        Inst::Blr { rn } => Ok(0xD63F_0000 | r(*rn) << 5),
        Inst::Nop => Ok(0xD503_201F),
        Inst::Brk { imm } => Ok(0xD420_0000 | u32::from(*imm) << 5),
        Inst::Udf { imm } => Ok(u32::from(*imm) << 5),
        Inst::DmbIsh => Ok(0xD503_3BBF),
        Inst::Cmp { rn, rm, shift: s } => reg3(0xEB00_001F, Reg(31), RegOrSp::Reg(*rn), *rm, *s),
        Inst::Csel { rd, rn, rm, cond } => {
            Ok(0x9A80_0000 | r(*rm) << 16 | cond.bits() << 12 | r(*rn) << 5 | r(*rd))
        }
        Inst::Cset { rd, cond } => Ok(0x9A9F_07E0 | (!cond.bits() & 0xf) << 12 | r(*rd)),
        Inst::Cinc { rd, rn, cond } => {
            Ok(0x9A80_0400 | r(*rn) << 5 | (!cond.bits() & 0xf) << 12 | r(*rd))
        }
        Inst::Mul { rd, rn, rm } => Ok(0x9B00_7C00 | r(*rm) << 16 | r(*rn) << 5 | r(*rd)),
        Inst::Sdiv { rd, rn, rm } => Ok(0x9AC0_0C00 | r(*rm) << 16 | r(*rn) << 5 | r(*rd)),
        Inst::Udiv { rd, rn, rm } => Ok(0x9AC0_0800 | r(*rm) << 16 | r(*rn) << 5 | r(*rd)),
        Inst::Madd { rd, rn, rm, ra } => {
            Ok(0x9B00_0000 | r(*rm) << 16 | r(*ra) << 10 | r(*rn) << 5 | r(*rd))
        }
        Inst::Msub { rd, rn, rm, ra } => {
            Ok(0x9B00_8000 | r(*rm) << 16 | r(*ra) << 10 | r(*rn) << 5 | r(*rd))
        }
        Inst::Neg { rd, rn, shift: s } => reg3(0xCB00_03E0, *rd, RegOrSp::Reg(Reg(31)), *rn, *s),
        Inst::Mvn { rd, rn, shift: s } => reg3(0xAA20_03E0, *rd, RegOrSp::Reg(Reg(31)), *rn, *s),
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
        Inst::Cmn { rn, rm, shift: s } => reg3(0xAB00_001F, Reg(31), RegOrSp::Reg(*rn), *rm, *s),
        Inst::AndImm { rd, rn, imm: v } => logical_imm(0x9200_0000, *rd, *rn, *v),
        Inst::OrrImm { rd, rn, imm: v } => logical_imm(0xB200_0000, *rd, *rn, *v),
        Inst::EorImm { rd, rn, imm: v } => logical_imm(0xD200_0000, *rd, *rn, *v),
        Inst::TstImm { rn, imm: v } => logical_imm(0xF200_001F, Reg(31), *rn, *v),
        Inst::And {
            rd,
            rn,
            rm,
            shift: s,
        } => reg3(0x8A00_0000, *rd, RegOrSp::Reg(*rn), *rm, *s),
        Inst::Orr {
            rd,
            rn,
            rm,
            shift: s,
        } => reg3(0xAA00_0000, *rd, RegOrSp::Reg(*rn), *rm, *s),
        Inst::Eor {
            rd,
            rn,
            rm,
            shift: s,
        } => reg3(0xCA00_0000, *rd, RegOrSp::Reg(*rn), *rm, *s),
        Inst::Tst { rn, rm, shift: s } => reg3(0xEA00_001F, Reg(31), RegOrSp::Reg(*rn), *rm, *s),
        Inst::LslImm { rd, rn, amount } => bit_shift(0xD340_0000, *rd, *rn, *amount),
        Inst::LsrImm { rd, rn, amount } => bit_shift(0xD340_FC00, *rd, *rn, *amount),
        Inst::AsrImm { rd, rn, amount } => bit_shift(0x9340_FC00, *rd, *rn, *amount),
        Inst::LslReg { rd, rn, rm } => Ok(0x9AC0_2000 | r(*rm) << 16 | r(*rn) << 5 | r(*rd)),
        Inst::LsrReg { rd, rn, rm } => Ok(0x9AC0_2400 | r(*rm) << 16 | r(*rn) << 5 | r(*rd)),
        Inst::AsrReg { rd, rn, rm } => Ok(0x9AC0_2800 | r(*rm) << 16 | r(*rn) << 5 | r(*rd)),
        Inst::Tbz { rt, bit, .. } => test_branch(0x3600_0000, *rt, *bit),
        Inst::Tbnz { rt, bit, .. } => test_branch(0x3700_0000, *rt, *bit),
        Inst::Fmov { rd, rn } => float2(0x1E60_4000, *rd, *rn),
        Inst::FmovGeneral { v, r: g, to_float } => {
            Ok((if *to_float { 0x9E67_0000 } else { 0x9E66_0000 })
                | r(*g)
                | u32::from(v.number) << 5)
        }
        Inst::Fadd { rd, rn, rm } => float3(0x1E60_2800, *rd, *rn, *rm),
        Inst::Fsub { rd, rn, rm } => float3(0x1E60_3800, *rd, *rn, *rm),
        Inst::Fmul { rd, rn, rm } => float3(0x1E60_0800, *rd, *rn, *rm),
        Inst::Fdiv { rd, rn, rm } => float3(0x1E60_1800, *rd, *rn, *rm),
        Inst::Fsqrt { rd, rn } => float2(0x1E61_C000, *rd, *rn),
        Inst::Fneg { rd, rn } => float2(0x1E61_4000, *rd, *rn),
        Inst::Fcmp { rn, rm } => float3(
            0x1E60_2000,
            VReg {
                number: 0,
                double: rn.double,
            },
            *rn,
            *rm,
        ),
        Inst::Scvtf { rd, rn } => Ok(0x9E62_0000 | r(*rn) | u32::from(rd.number) << 5),
        Inst::Fcvtzs { rd, rn } => Ok(0x9E78_0000 | r(*rd) | u32::from(rn.number) << 5),
        Inst::LdrD { rt, mem: m } => float_mem(*m, *rt, true),
        Inst::StrD { rt, mem: m } => float_mem(*m, *rt, false),
    }
}
