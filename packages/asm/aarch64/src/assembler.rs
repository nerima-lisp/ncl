use crate::{EncodeError, Inst, encode};
use std::collections::BTreeMap;

/// A symbolic assembler label.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct Label(pub u32);
/// The relocation operation represented by a fixup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixupKind {
    /// 26-bit branch.
    Branch26,
    /// 19-bit conditional branch.
    CondBranch19,
    /// 21-bit page-relative address.
    Adrp21,
    /// 21-bit byte-relative address.
    Adr21,
    /// 14-bit test-and-branch displacement.
    TestBranch14,
    /// 19-bit literal displacement.
    Literal19,
    /// 64-bit absolute data.
    Abs64,
    /// Twelve-bit add relocation.
    Add12,
}
/// A symbolic relocation in a finished code blob.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Fixup {
    /// Byte offset of the instruction or data.
    pub offset: usize,
    /// Relocation kind.
    pub kind: FixupKind,
    /// Target label.
    pub target: Label,
}
/// Emitted machine code and its symbolic relocations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeBlob {
    /// Little-endian machine code bytes.
    pub bytes: Vec<u8>,
    /// Relocations retained for object-file conversion.
    pub fixups: Vec<Fixup>,
}
/// A small label-aware `AArch64` assembler.
#[derive(Clone, Debug, Default)]
pub struct Assembler {
    bytes: Vec<u8>,
    labels: BTreeMap<Label, usize>,
    fixups: Vec<Fixup>,
    next: u32,
}
impl Assembler {
    /// Creates an empty assembler.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Allocates a fresh label.
    pub const fn new_label(&mut self) -> Label {
        let l = Label(self.next);
        self.next = self.next.saturating_add(1);
        l
    }
    /// Binds a label at the current byte position.
    ///
    /// # Errors
    ///
    /// Returns [`EncodeError::DuplicateLabel`] if the label is already bound.
    pub fn bind(&mut self, label: Label) -> Result<(), EncodeError> {
        if self.labels.insert(label, self.bytes.len()).is_some() {
            return Err(EncodeError::DuplicateLabel(label));
        }
        Ok(())
    }
    /// Encodes and appends an instruction.
    ///
    /// # Errors
    ///
    /// Returns an encoding error if the instruction operands are invalid.
    pub fn emit(&mut self, inst: &Inst) -> Result<(), EncodeError> {
        let at = self.bytes.len();
        let word = encode(inst, at)?;
        self.bytes.extend_from_slice(&word.to_le_bytes());
        if let Some((kind, target)) = fixup(inst) {
            self.fixups.push(Fixup {
                offset: at,
                kind,
                target,
            });
        }
        Ok(())
    }
    /// Resolves local labels and returns code plus retained fixups.
    ///
    /// # Errors
    ///
    /// Returns an error for an unbound label or an out-of-range relocation.
    pub fn finish(mut self) -> Result<CodeBlob, EncodeError> {
        for f in &self.fixups {
            let target = *self
                .labels
                .get(&f.target)
                .ok_or(EncodeError::UnboundLabel(f.target))?;
            let Ok(place) = i64::try_from(f.offset) else {
                return Err(EncodeError::RelocationOutOfRange {
                    offset: f.offset,
                    target: f.target,
                });
            };
            let Ok(target_i64) = i64::try_from(target) else {
                return Err(EncodeError::RelocationOutOfRange {
                    offset: f.offset,
                    target: f.target,
                });
            };
            let delta = target_i64 - place;
            let word = u32::from_le_bytes(
                self.bytes[f.offset..f.offset + 4]
                    .try_into()
                    .map_err(|_| EncodeError::InvalidLength)?,
            );
            let patched = match f.kind {
                FixupKind::Branch26 => patch_signed(word, delta, 26, 2, 0, *f)?,
                FixupKind::CondBranch19 | FixupKind::Literal19 => {
                    patch_signed(word, delta, 19, 2, 5, *f)?
                }
                FixupKind::Adrp21 => {
                    let d = (target_i64 / 4096) - (place / 4096);
                    patch_signed(word, d, 21, 0, 5, *f)?
                }
                FixupKind::Adr21 => patch_signed(word, delta, 21, 0, 5, *f)?,
                FixupKind::TestBranch14 => patch_signed(word, delta, 14, 2, 5, *f)?,
                FixupKind::Abs64 | FixupKind::Add12 => word,
            };
            self.bytes[f.offset..f.offset + 4].copy_from_slice(&patched.to_le_bytes());
        }
        Ok(CodeBlob {
            bytes: self.bytes,
            fixups: self.fixups,
        })
    }
}

const fn fixup(i: &Inst) -> Option<(FixupKind, Label)> {
    match i {
        Inst::B { label } | Inst::Bl { label } => Some((FixupKind::Branch26, *label)),
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
fn patch_signed(
    word: u32,
    delta: i64,
    bits: u8,
    shift: u8,
    lsb: u8,
    fixup: Fixup,
) -> Result<u32, EncodeError> {
    let v = delta >> shift;
    let min = -(1_i64 << (bits - 1));
    let max = (1_i64 << (bits - 1)) - 1;
    if v < min || v > max || delta & ((1 << shift) - 1) != 0 {
        return Err(EncodeError::RelocationOutOfRange {
            offset: fixup.offset,
            target: fixup.target,
        });
    }
    let Ok(narrowed) = u32::try_from(v) else {
        return Err(EncodeError::RelocationOutOfRange {
            offset: fixup.offset,
            target: fixup.target,
        });
    };
    Ok(word | (narrowed & ((1 << bits) - 1)) << lsb)
}
