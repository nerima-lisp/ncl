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
    OffsetOutOfRange { offset: usize },
}

impl core::fmt::Display for RelocationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::OffsetOutOfRange { offset } => {
                write!(f, "relocation offset {offset} does not fit in u32")
            }
        }
    }
}

impl std::error::Error for RelocationError {}

impl TryFrom<Fixup> for Relocation {
    type Error = RelocationError;

    fn try_from(fixup: Fixup) -> Result<Self, Self::Error> {
        let offset =
            u32::try_from(fixup.offset).map_err(|_| RelocationError::OffsetOutOfRange {
                offset: fixup.offset,
            })?;
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

/// Converts assembler fixups into the codegen relocation representation.
///
/// # Errors
///
/// Returns an error if a fixup offset cannot be represented by the public
/// relocation format.
pub fn relocations_from_fixups(fixups: &[Fixup]) -> Result<Vec<Relocation>, RelocationError> {
    fixups.iter().copied().map(Relocation::try_from).collect()
}

#[cfg(test)]
#[allow(clippy::expect_used, missing_docs)]
mod tests {
    use super::*;

    #[test]
    fn converts_fixup_kinds_and_preserves_order() {
        let fixups = [
            Fixup {
                offset: 12,
                kind: FixupKind::Rel32,
                target: Label(3),
            },
            Fixup {
                offset: 24,
                kind: FixupKind::Abs64,
                target: Label(1),
            },
        ];

        assert_eq!(
            relocations_from_fixups(&fixups),
            Ok(vec![
                Relocation {
                    offset: 12,
                    kind: RelocationKind::PcRelative32,
                    target: Label(3),
                    addend: 0,
                },
                Relocation {
                    offset: 24,
                    kind: RelocationKind::Absolute64,
                    target: Label(1),
                    addend: 0,
                },
            ])
        );
    }

    #[test]
    fn rejects_fixup_offset_that_does_not_fit_public_format() {
        let fixup = Fixup {
            offset: usize::MAX,
            kind: FixupKind::Rel32,
            target: Label(0),
        };

        assert_eq!(
            relocations_from_fixups(&[fixup]),
            Err(RelocationError::OffsetOutOfRange { offset: usize::MAX })
        );
    }

    #[test]
    fn accepts_empty_fixup_lists_and_formats_conversion_errors() {
        assert_eq!(relocations_from_fixups(&[]), Ok(Vec::new()));
        assert_eq!(
            RelocationError::OffsetOutOfRange { offset: 1 }.to_string(),
            "relocation offset 1 does not fit in u32"
        );

        let relocation = Relocation::try_from(Fixup {
            offset: 0,
            kind: FixupKind::Abs64,
            target: Label(9),
        })
        .expect("zero-offset absolute fixup");
        assert_eq!(relocation.offset, 0);
        assert_eq!(relocation.kind, RelocationKind::Absolute64);
        assert_eq!(relocation.target, Label(9));
        assert_eq!(relocation.addend, 0);
    }
}
