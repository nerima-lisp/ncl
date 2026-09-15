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
        _ => Err(EncodeError::UnsupportedInstruction(word)),
    }
}
