//! Dependency-free `AArch64` instruction encoding for NCL's native backend.
#![allow(missing_docs)]

mod assembler;
mod decoder;
mod encoding;
mod model;

pub use assembler::{Assembler, CodeBlob, Fixup, FixupKind, Label};
pub use decoder::decode;
pub use encoding::encode;
pub use model::{Cond, Extend, Inst, MemOperand, Reg, RegOrSp, RegOrZr, Shift, VReg};

/// Decodes one word and renders the supported instruction in a compact GNU-style form.
///
/// # Errors
///
/// Returns [`EncodeError::UnsupportedInstruction`] when the word is not supported.
pub fn disassemble(word: u32) -> Result<String, EncodeError> {
    match decode(word)? {
        Inst::Nop => Ok(String::from("nop")),
        Inst::Ret { rn } => Ok(format!("ret x{}", rn.number())),
        instruction => Ok(format!("{instruction:?}")),
    }
}

/// Produces a short MOV-wide sequence for a 64-bit constant.
#[must_use]
pub fn mov_imm64(rd: Reg, value: u64) -> Vec<Inst> {
    let mut parts = [0_u16; 4];
    for (index, part) in parts.iter_mut().enumerate() {
        *part = match u16::try_from((value >> (index * 16)) & u64::from(u16::MAX)) {
            Ok(part) => part,
            Err(_) => return Vec::new(),
        };
    }
    let use_n = parts.iter().filter(|&&part| part == u16::MAX).count()
        > parts.iter().filter(|&&part| part == 0).count();
    let first = parts
        .iter()
        .position(|&part| if use_n { part != u16::MAX } else { part != 0 });
    let Some(first) = first else {
        return vec![if use_n {
            Inst::MovN {
                rd,
                imm: 0,
                shift: 0,
            }
        } else {
            Inst::MovZ {
                rd,
                imm: 0,
                shift: 0,
            }
        }];
    };
    let mut result = vec![if use_n {
        Inst::MovN {
            rd,
            imm: !parts[first],
            shift: [0, 16, 32, 48][first],
        }
    } else {
        Inst::MovZ {
            rd,
            imm: parts[first],
            shift: [0, 16, 32, 48][first],
        }
    }];
    for (index, &part) in parts.iter().enumerate() {
        if index != first && ((use_n && part != u16::MAX) || (!use_n && part != 0)) {
            result.push(Inst::MovK {
                rd,
                imm: part,
                shift: [0, 16, 32, 48][index],
            });
        }
    }
    result
}

/// An instruction encoding or relocation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EncodeError {
    /// A register number is outside the architectural range.
    InvalidRegister(u8),
    /// An immediate cannot be represented by the selected instruction.
    ImmediateOutOfRange { value: i64, bits: u8 },
    /// An immediate is not representable as an `AArch64` logical immediate.
    InvalidBitmaskImmediate(u64),
    /// A label was referenced but never bound.
    UnboundLabel(Label),
    /// A label was bound more than once.
    DuplicateLabel(Label),
    /// A branch or PC-relative relocation does not fit.
    RelocationOutOfRange { offset: usize, target: Label },
    /// The byte stream does not contain a complete instruction.
    InvalidLength,
    /// The instruction is not in the supported `Phase 1` subset.
    UnsupportedInstruction(u32),
}

impl core::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidRegister(r) => write!(f, "invalid register x{r}"),
            Self::ImmediateOutOfRange { value, bits } => {
                write!(f, "immediate {value} does not fit {bits} bits")
            }
            Self::InvalidBitmaskImmediate(v) => write!(f, "invalid logical immediate 0x{v:x}"),
            Self::UnboundLabel(l) => write!(f, "unbound label {l:?}"),
            Self::DuplicateLabel(l) => write!(f, "duplicate label {l:?}"),
            Self::RelocationOutOfRange { offset, target } => {
                write!(f, "relocation at {offset} to {target:?} is out of range")
            }
            Self::InvalidLength => f.write_str("instruction bytes are not four-byte aligned"),
            Self::UnsupportedInstruction(w) => write!(f, "unsupported instruction 0x{w:08x}"),
        }
    }
}

impl std::error::Error for EncodeError {}
