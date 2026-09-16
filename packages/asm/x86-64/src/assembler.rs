use crate::model::{Inst, Label};
use core::fmt;
use std::collections::HashMap;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixupKind {
    Rel32,
    Abs64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Fixup {
    pub offset: usize,
    pub kind: FixupKind,
    pub target: Label,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeBlob {
    pub bytes: Vec<u8>,
    pub fixups: Vec<Fixup>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EncodeError {
    UnboundLabel(Label),
    Rel32OutOfRange,
    InvalidOperand(&'static str),
    InvalidNopLength,
    InvalidRegister(u8),
    BufferTooLarge,
    UnsupportedFixup(FixupKind),
}
impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnboundLabel(l) => write!(f, "unbound label {:?}", l),
            Self::Rel32OutOfRange => f.write_str("rel32 out of range"),
            Self::InvalidOperand(s) => f.write_str(s),
            Self::InvalidNopLength => f.write_str("nop length must be 1..=9"),
            Self::InvalidRegister(n) => write!(f, "invalid register {n}"),
            Self::BufferTooLarge => f.write_str("buffer too large"),
            Self::UnsupportedFixup(kind) => write!(f, "unsupported fixup kind {kind:?}"),
        }
    }
}
impl std::error::Error for EncodeError {}
#[derive(Debug, Default)]
pub struct Assembler {
    bytes: Vec<u8>,
    labels: HashMap<Label, usize>,
    fixups: Vec<Fixup>,
    next: u32,
}
impl Assembler {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn new_label(&mut self) -> Label {
        let label = Label(self.next);
        self.next = self.next.wrapping_add(1);
        label
    }
    pub fn bind(&mut self, label: Label) {
        self.labels.insert(label, self.bytes.len());
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn emit(&mut self, inst: &Inst) -> Result<(), EncodeError> {
        crate::encode::encode_inst(inst, &mut self.bytes, &mut self.fixups)
    }
    #[cfg(test)]
    pub(crate) fn add_test_fixup(&mut self, fixup: Fixup) {
        self.fixups.push(fixup);
    }
    #[cfg(test)]
    pub(crate) fn set_test_label_position(&mut self, label: Label, position: usize) {
        self.labels.insert(label, position);
    }
    pub fn finish(mut self) -> Result<CodeBlob, EncodeError> {
        for fixup in &self.fixups {
            match fixup.kind {
                FixupKind::Rel32 => {}
                kind => return Err(EncodeError::UnsupportedFixup(kind)),
            }
            let Some(&at) = self.labels.get(&fixup.target) else {
                return Err(EncodeError::UnboundLabel(fixup.target));
            };
            let end = fixup.offset + 4;
            let distance = at as i64 - end as i64;
            if !(i32::MIN as i64..=i32::MAX as i64).contains(&distance) {
                return Err(EncodeError::Rel32OutOfRange);
            }
            self.bytes[fixup.offset..end].copy_from_slice(&(distance as i32).to_le_bytes());
        }
        Ok(CodeBlob {
            bytes: self.bytes,
            fixups: self.fixups,
        })
    }
}
