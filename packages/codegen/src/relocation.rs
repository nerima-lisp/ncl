#![allow(missing_docs)]

use ncl_asm_x86_64::{Fixup, FixupKind, Label};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelocationKind {
    PcRelative32,
    Absolute64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Relocation {
    pub offset: u32,
    pub kind: RelocationKind,
    pub target: Label,
    pub addend: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelocationError {
    OffsetOutOfRange(usize),
}

impl TryFrom<Fixup> for Relocation {
    type Error = RelocationError;

    fn try_from(fixup: Fixup) -> Result<Self, Self::Error> {
        let offset = u32::try_from(fixup.offset)
            .map_err(|_| RelocationError::OffsetOutOfRange(fixup.offset))?;
        let kind = match fixup.kind {
            FixupKind::Rel32 => RelocationKind::PcRelative32,
            FixupKind::Abs64 => RelocationKind::Absolute64,
        };
        Ok(Self {
            offset,
            kind,
            target: fixup.target,
            addend: 0,
        })
    }
}

pub fn relocations_from_fixups(fixups: &[Fixup]) -> Result<Vec<Relocation>, RelocationError> {
    fixups.iter().copied().map(Relocation::try_from).collect()
}
