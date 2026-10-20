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
    OffsetOutOfRange {
        offset: usize,
    },
    PatchOutOfBounds {
        offset: u32,
        width: usize,
        length: usize,
    },
    AddressOverflow {
        kind: RelocationKind,
    },
    ValueOutOfRange {
        kind: RelocationKind,
        value: i128,
    },
}

impl core::fmt::Display for RelocationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::OffsetOutOfRange { offset } => {
                write!(f, "relocation offset {offset} does not fit in u32")
            }
            Self::PatchOutOfBounds {
                offset,
                width,
                length,
            } => write!(
                f,
                "relocation patch {offset}..{} exceeds byte length {length}",
                u64::from(*offset) + u64::try_from(*width).unwrap_or(u64::MAX)
            ),
            Self::AddressOverflow { kind } => {
                write!(f, "{kind:?} relocation place overflows the address space")
            }
            Self::ValueOutOfRange { kind, value } => {
                write!(f, "{kind:?} relocation value {value} is out of range")
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

impl Relocation {
    /// Applies the value at the address of the first patch byte.
    ///
    /// # Errors
    ///
    /// Returns an error if the patch is out of bounds, the PC-relative patch
    /// end address overflows, or the relocated value does not fit the selected
    /// relocation kind.
    pub fn apply_to_bytes(
        &self,
        bytes: &mut [u8],
        relocation_place: u64,
        target_address: u64,
    ) -> Result<(), RelocationError> {
        let width = match self.kind {
            RelocationKind::PcRelative32 => 4,
            RelocationKind::Absolute64 => 8,
        };
        let offset =
            usize::try_from(self.offset).map_err(|_| RelocationError::PatchOutOfBounds {
                offset: self.offset,
                width,
                length: bytes.len(),
            })?;
        let end = offset
            .checked_add(width)
            .ok_or(RelocationError::PatchOutOfBounds {
                offset: self.offset,
                width,
                length: bytes.len(),
            })?;
        let length = bytes.len();
        let patch = bytes
            .get_mut(offset..end)
            .ok_or(RelocationError::PatchOutOfBounds {
                offset: self.offset,
                width,
                length,
            })?;
        let value = i128::from(target_address) + i128::from(self.addend);
        match self.kind {
            RelocationKind::PcRelative32 => {
                let place_end = relocation_place
                    .checked_add(4)
                    .ok_or(RelocationError::AddressOverflow { kind: self.kind })?;
                let displacement = value - i128::from(place_end);
                let displacement =
                    i32::try_from(displacement).map_err(|_| RelocationError::ValueOutOfRange {
                        kind: self.kind,
                        value: displacement,
                    })?;
                patch.copy_from_slice(&displacement.to_le_bytes());
            }
            RelocationKind::Absolute64 => {
                let address =
                    u64::try_from(value).map_err(|_| RelocationError::ValueOutOfRange {
                        kind: self.kind,
                        value,
                    })?;
                patch.copy_from_slice(&address.to_le_bytes());
            }
        }
        Ok(())
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
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use ncl_asm_x86_64::{Assembler, Inst};
    use ncl_objfile::{
        Architecture as ObjectArchitecture, Fasl, FaslHeader, FaslReader, FaslSection, FaslWriter,
        RelocKind as ObjectRelocKind, Relocation as ObjectRelocation, SectionId, SymbolRef,
    };

    fn relocation(offset: u32, kind: RelocationKind, addend: i32) -> Relocation {
        Relocation {
            offset,
            kind,
            target: Label(0),
            addend,
        }
    }

    #[test]
    fn applies_pc_relative_and_absolute_relocations() {
        let mut bytes = [0; 16];
        relocation(4, RelocationKind::PcRelative32, 2)
            .apply_to_bytes(&mut bytes, 0x1000, 0x1010)
            .expect("pc relative relocation");
        relocation(8, RelocationKind::Absolute64, -4)
            .apply_to_bytes(&mut bytes, 0, 0x1000)
            .expect("absolute relocation");
        assert_eq!(&bytes[4..8], &14i32.to_le_bytes());
        assert_eq!(&bytes[8..16], &0xffcu64.to_le_bytes());
    }

    #[test]
    fn rejects_start_and_width_boundary_overruns() {
        for (offset, width) in [(8, 8), (1, 8), (5, 4)] {
            let kind = if width == 8 {
                RelocationKind::Absolute64
            } else {
                RelocationKind::PcRelative32
            };
            let mut bytes = [0; 8];
            assert_eq!(
                relocation(offset, kind, 0).apply_to_bytes(&mut bytes, 0, 0),
                Err(RelocationError::PatchOutOfBounds {
                    offset,
                    width,
                    length: 8,
                })
            );
        }
    }

    #[test]
    fn rejects_address_and_value_overflow_without_mutating_bytes() {
        let mut bytes = [0; 8];
        let absolute = relocation(0, RelocationKind::Absolute64, -1);
        assert_eq!(
            absolute.apply_to_bytes(&mut bytes, 0, 0),
            Err(RelocationError::ValueOutOfRange {
                kind: RelocationKind::Absolute64,
                value: -1,
            })
        );
        assert_eq!(bytes, [0; 8]);

        let relative = relocation(0, RelocationKind::PcRelative32, 0);
        assert_eq!(
            relative.apply_to_bytes(&mut bytes, u64::MAX - 2, 0),
            Err(RelocationError::AddressOverflow {
                kind: RelocationKind::PcRelative32,
            })
        );
        assert_eq!(bytes, [0; 8]);

        assert_eq!(
            relative.apply_to_bytes(&mut bytes, 0, u64::from(i32::MAX as u32) + 5),
            Err(RelocationError::ValueOutOfRange {
                kind: RelocationKind::PcRelative32,
                value: i128::from(i32::MAX) + 1,
            })
        );
        assert_eq!(bytes, [0; 8]);
    }

    #[test]
    fn x86_code_blob_fixup_matches_fasl_relocation_round_trip() {
        let mut assembler = Assembler::new();
        let target = assembler.new_label();
        assembler.emit(&Inst::Jmp(target)).expect("jump");
        assembler.emit(&Inst::Nop(3)).expect("padding");
        let target_offset = u64::try_from(assembler.bytes().len()).expect("target offset");
        assembler.bind(target);
        assembler.emit(&Inst::Nop(1)).expect("target");
        let unpatched = assembler.bytes().to_vec();
        let blob = assembler.finish().expect("resolved local label");
        let codegen_relocation = relocations_from_fixups(&blob.fixups)
            .expect("relocation conversion")
            .into_iter()
            .next()
            .expect("jump fixup");
        let fasl_relocation = ObjectRelocation {
            section: SectionId(0),
            offset: codegen_relocation.offset,
            kind: ObjectRelocKind::PcRel32,
            symbol: SymbolRef::External("local_jump_target".into()),
            addend: i64::from(codegen_relocation.addend),
        };
        let fasl = Fasl {
            header: FaslHeader {
                architecture: ObjectArchitecture::X86_64,
                features: 0,
            },
            sections: FaslSection {
                code: unpatched,
                relocations: vec![fasl_relocation.clone()],
                constants: vec![],
                symbols: vec![],
                stack_maps: vec![],
                debug: vec![],
            },
        };
        let bytes = FaslWriter::write(&fasl).expect("write FASL v2");
        let decoded =
            FaslReader::read(&bytes, ObjectArchitecture::X86_64, 0).expect("read FASL v2");
        assert_eq!(decoded.sections.relocations, vec![fasl_relocation]);
        assert_eq!(
            decoded.sections.relocations[0].symbol,
            SymbolRef::External("local_jump_target".into())
        );
        assert_eq!(decoded.sections.code, fasl.sections.code);

        let decoded_relocation = &decoded.sections.relocations[0];
        let relocation = Relocation {
            offset: decoded_relocation.offset,
            kind: RelocationKind::PcRelative32,
            target: codegen_relocation.target,
            addend: i32::try_from(decoded_relocation.addend).expect("FASL addend fits i32"),
        };
        let mut applied = decoded.sections.code;
        relocation
            .apply_to_bytes(
                &mut applied,
                0x4000 + u64::from(decoded_relocation.offset),
                0x4000 + target_offset,
            )
            .expect("apply decoded relocation");
        assert_eq!(applied, blob.bytes);
    }
}
