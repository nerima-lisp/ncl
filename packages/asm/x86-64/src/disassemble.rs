use crate::model::Inst;
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodeError {
    Truncated,
    Unsupported,
    Invalid,
}
impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Truncated => "truncated instruction",
            Self::Unsupported => "unsupported instruction",
            Self::Invalid => "invalid instruction",
        })
    }
}
impl std::error::Error for DecodeError {}
pub fn decode(bytes: &[u8]) -> Result<Vec<Inst>, DecodeError> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        match bytes[pos] {
            0x90 => {
                out.push(Inst::Nop(1));
                pos += 1
            }
            0xc3 => {
                out.push(Inst::Ret);
                pos += 1
            }
            0xcc => {
                out.push(Inst::Int3);
                pos += 1
            }
            0x0f if bytes.get(pos + 1) == Some(&0x0b) => {
                out.push(Inst::Ud2);
                pos += 2
            }
            _ => return Err(DecodeError::Unsupported),
        }
    }
    Ok(out)
}
#[must_use]
pub fn display(inst: &Inst) -> String {
    format!("{inst:?}")
}
