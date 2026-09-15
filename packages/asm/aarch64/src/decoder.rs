use crate::{EncodeError, Inst, Reg};

/// Decodes one supported word.
///
/// # Errors
///
/// Returns [`EncodeError::UnsupportedInstruction`] for an unknown word.
pub const fn decode(word: u32) -> Result<Inst, EncodeError> {
    match word {
        0xD503_201F => Ok(Inst::Nop),
        0xD65F_03C0 => Ok(Inst::Ret { rn: Reg(30) }),
        _ if word & 0xFF80_0000 == 0xD280_0000 => Ok(Inst::MovZ {
            rd: Reg((word & 0x1f) as u8),
            imm: ((word >> 5) & 0xffff) as u16,
            shift: (((word >> 21) & 0x3) as u8) * 16,
        }),
        _ if word & 0xFF00_0000 == 0x9100_0000 => Ok(Inst::AddImm {
            rd: reg_or_sp((word & 0x1f) as u8),
            rn: reg_or_sp(((word >> 5) & 0x1f) as u8),
            imm: ((word >> 10) & 0xfff) as u16,
            shift: word & (1 << 22) != 0,
        }),
        _ if word & 0xFF00_0000 == 0xD100_0000 => Ok(Inst::SubImm {
            rd: reg_or_sp((word & 0x1f) as u8),
            rn: reg_or_sp(((word >> 5) & 0x1f) as u8),
            imm: ((word >> 10) & 0xfff) as u16,
            shift: word & (1 << 22) != 0,
        }),
        _ if word & 0xFF80_0000 == 0xF800_0000 => {
            let raw = ((word >> 12) & 0x1ff) as i16;
            let offset = (raw << 7 >> 7) as i16;
            let base = reg_or_sp(((word >> 5) & 0x1f) as u8);
            let mode = (word >> 10) & 0x3;
            let mem = match mode {
                3 => crate::MemOperand::PreIndex { base, offset },
                1 => crate::MemOperand::PostIndex { base, offset },
                _ => crate::MemOperand::Unscaled { base, offset },
            };
            if word & 0x0040_0000 != 0 {
                Ok(Inst::Ldr {
                    rt: Reg((word & 0x1f) as u8),
                    mem,
                })
            } else {
                Ok(Inst::Str {
                    rt: Reg((word & 0x1f) as u8),
                    mem,
                })
            }
        }
        _ if word & 0xFFE0_FFE0 == 0xAA00_03E0 => Ok(Inst::Mov {
            rd: crate::RegOrSp::Reg(Reg((word & 0x1f) as u8)),
            rn: crate::RegOrSp::Reg(Reg(((word >> 16) & 0x1f) as u8)),
        }),
        _ if word & 0xFFFF_FC1F == 0xD63F_0000 => Ok(Inst::Blr {
            rn: Reg(((word >> 5) & 0x1f) as u8),
        }),
        _ if word & 0x7F00_0000 == 0x3400_0000 => Ok(Inst::Cbz {
            rt: Reg((word & 0x1f) as u8),
            label: crate::Label(0),
        }),
        _ if word & 0x7F00_0000 == 0x3500_0000 => Ok(Inst::Cbnz {
            rt: Reg((word & 0x1f) as u8),
            label: crate::Label(0),
        }),
        _ if word & 0xFFE0_001F == 0xD420_0000 => Ok(Inst::Brk {
            imm: ((word >> 5) & 0xffff) as u16,
        }),
        _ if word & 0xFC00_0000 == 0x1400_0000 => Ok(Inst::B {
            label: crate::Label(0),
        }),
        _ if word & 0xFF00_0010 == 0x5400_0000 => Ok(Inst::BCond {
            cond: cond((word & 0xf) as u8),
            label: crate::Label(0),
        }),
        _ if word & 0x3E00_0000 == 0x2800_0000 => {
            let load = word & 0x0040_0000 != 0;
            let mode = (word >> 23) & 0x3;
            let offset = ((((word >> 15) & 0x7f) as i16) << 9 >> 9) * 8;
            let base = match ((word >> 5) & 0x1f) as u8 {
                31 => crate::RegOrSp::Sp,
                value => crate::RegOrSp::Reg(Reg(value)),
            };
            let mem = match mode {
                3 => crate::MemOperand::PreIndex { base, offset },
                1 => crate::MemOperand::PostIndex { base, offset },
                _ => crate::MemOperand::Unscaled { base, offset },
            };
            if load {
                Ok(Inst::Ldp {
                    rt: Reg((word & 0x1f) as u8),
                    rt2: Reg(((word >> 10) & 0x1f) as u8),
                    mem,
                })
            } else {
                Ok(Inst::Stp {
                    rt: Reg((word & 0x1f) as u8),
                    rt2: Reg(((word >> 10) & 0x1f) as u8),
                    mem,
                })
            }
        }
        _ => Err(EncodeError::UnsupportedInstruction(word)),
    }
}

const fn reg_or_sp(value: u8) -> crate::RegOrSp {
    match value {
        31 => crate::RegOrSp::Sp,
        value => crate::RegOrSp::Reg(Reg(value)),
    }
}

const fn cond(value: u8) -> crate::model::Cond {
    match value {
        0 => crate::model::Cond::Eq,
        1 => crate::model::Cond::Ne,
        2 => crate::model::Cond::Cs,
        3 => crate::model::Cond::Cc,
        4 => crate::model::Cond::Mi,
        5 => crate::model::Cond::Pl,
        6 => crate::model::Cond::Vs,
        7 => crate::model::Cond::Vc,
        8 => crate::model::Cond::Hi,
        9 => crate::model::Cond::Ls,
        10 => crate::model::Cond::Ge,
        11 => crate::model::Cond::Lt,
        12 => crate::model::Cond::Gt,
        13 => crate::model::Cond::Le,
        _ => crate::model::Cond::Al,
    }
}
