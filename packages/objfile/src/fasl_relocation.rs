use crate::error::{FaslError, FaslRejectReason};
use crate::fasl_records::{SectionReader, SectionWriter};
use crate::{ObjectError, RelocKind, Relocation, SectionId, SymbolRef};

const MAX_FASL_RECORDS: usize = 1_048_576;
const FASL_RELOCATION_RECORD_SIZE: usize = 32;

pub fn encode(items: &[Relocation]) -> Result<Vec<u8>, ObjectError> {
    let mut out = Vec::new();
    for item in items {
        out.extend_from_slice(&item.section.0.to_le_bytes());
        out.extend_from_slice(&item.offset.to_le_bytes());
        out.extend_from_slice(&kind_number(item.kind).to_le_bytes());
        out.extend_from_slice(
            &i32::try_from(item.addend)
                .map_err(|_| ObjectError::InvalidField {
                    field: "relocation addend",
                    value: item.addend.unsigned_abs(),
                })?
                .to_le_bytes(),
        );
        match &item.symbol {
            SymbolRef::Local(index) => {
                out.extend_from_slice(&0u32.to_le_bytes());
                out.extend_from_slice(&index.to_le_bytes());
            }
            SymbolRef::External(name) => {
                let size = u32::try_from(name.len()).map_err(|_| ObjectError::InvalidField {
                    field: "symbol name size",
                    value: u64::try_from(name.len()).unwrap_or(u64::MAX),
                })?;
                out.extend_from_slice(&1u32.to_le_bytes());
                out.extend_from_slice(&size.to_le_bytes());
                out.extend_from_slice(name.as_bytes());
            }
        }
    }
    Ok(out)
}

pub fn decode(bytes: &[u8], version: u16, count: u32) -> Result<Vec<Relocation>, ObjectError> {
    if version == 1 {
        if !bytes.len().is_multiple_of(16)
            || bytes.len() / 16 != usize::try_from(count).unwrap_or(usize::MAX)
        {
            return Err(ObjectError::InvalidField {
                field: "relocation size",
                value: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            });
        }
        let (records, _) = bytes.as_chunks::<16>();
        return records
            .iter()
            .map(|record| decode_core(record, SymbolRef::Local(0)))
            .collect();
    }

    let count = usize::try_from(count).map_err(|_| ObjectError::InvalidField {
        field: "relocation count",
        value: u64::MAX,
    })?;
    let mut cursor = 0;
    let mut relocations = Vec::with_capacity(count.min(bytes.len() / 24));
    for _ in 0..count {
        let core = take::<16>(bytes, &mut cursor)?;
        let symbol_kind = u32::from_le_bytes(take::<4>(bytes, &mut cursor)?);
        let symbol = match symbol_kind {
            0 => SymbolRef::Local(u32::from_le_bytes(take::<4>(bytes, &mut cursor)?)),
            1 => {
                let size = usize::try_from(u32::from_le_bytes(take::<4>(bytes, &mut cursor)?))
                    .map_err(|_| ObjectError::InvalidField {
                        field: "symbol name size",
                        value: u64::MAX,
                    })?;
                let end = cursor.checked_add(size).ok_or(ObjectError::Truncated {
                    offset: cursor,
                    needed: size,
                })?;
                let name = bytes.get(cursor..end).ok_or(ObjectError::Truncated {
                    offset: cursor,
                    needed: size,
                })?;
                cursor = end;
                SymbolRef::External(
                    core::str::from_utf8(name)
                        .map_err(|_| ObjectError::InvalidName)?
                        .to_owned(),
                )
            }
            value => {
                return Err(ObjectError::InvalidField {
                    field: "relocation symbol kind",
                    value: u64::from(value),
                });
            }
        };
        relocations.push(decode_core(&core, symbol)?);
    }
    if cursor != bytes.len() {
        return Err(ObjectError::InvalidField {
            field: "relocation size",
            value: u64::try_from(bytes.len() - cursor).unwrap_or(u64::MAX),
        });
    }
    Ok(relocations)
}

fn decode_core(bytes: &[u8], symbol: SymbolRef) -> Result<Relocation, ObjectError> {
    Ok(Relocation {
        section: SectionId(u32_at(bytes, 0)?),
        offset: u32_at(bytes, 4)?,
        kind: number_kind(u32_at(bytes, 8)?)?,
        symbol,
        addend: i64::from(i32::from_le_bytes(take_at(bytes, 12)?)),
    })
}

fn take<const N: usize>(bytes: &[u8], cursor: &mut usize) -> Result<[u8; N], ObjectError> {
    let end = cursor.checked_add(N).ok_or(ObjectError::Truncated {
        offset: *cursor,
        needed: N,
    })?;
    let value = bytes.get(*cursor..end).ok_or(ObjectError::Truncated {
        offset: *cursor,
        needed: N,
    })?;
    *cursor = end;
    value.try_into().map_err(|_| ObjectError::Truncated {
        offset: *cursor,
        needed: N,
    })
}

fn take_at<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], ObjectError> {
    bytes
        .get(offset..offset + N)
        .ok_or(ObjectError::Truncated { offset, needed: N })?
        .try_into()
        .map_err(|_| ObjectError::Truncated { offset, needed: N })
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, ObjectError> {
    Ok(u32::from_le_bytes(take_at(bytes, offset)?))
}

pub const fn kind_number(kind: RelocKind) -> u32 {
    match kind {
        RelocKind::Abs64 => 0,
        RelocKind::PcRel32 => 1,
        RelocKind::Plt32 => 2,
        RelocKind::Branch26 => 3,
        RelocKind::Adrp21 => 4,
        RelocKind::Add12 => 5,
        RelocKind::CondBranch19 => 6,
        RelocKind::CodeEntry => 7,
        RelocKind::ExternalSymbol => 8,
    }
}

pub fn number_kind(value: u32) -> Result<RelocKind, ObjectError> {
    let index = usize::try_from(value).map_err(|_| ObjectError::InvalidField {
        field: "relocation kind",
        value: u64::from(value),
    })?;
    [
        RelocKind::Abs64,
        RelocKind::PcRel32,
        RelocKind::Plt32,
        RelocKind::Branch26,
        RelocKind::Adrp21,
        RelocKind::Add12,
        RelocKind::CondBranch19,
        RelocKind::CodeEntry,
        RelocKind::ExternalSymbol,
    ]
    .get(index)
    .copied()
    .ok_or_else(|| ObjectError::InvalidField {
        field: "relocation kind",
        value: u64::from(value),
    })
}

pub const fn width(kind: RelocKind) -> u32 {
    match kind {
        RelocKind::Abs64 | RelocKind::CodeEntry | RelocKind::ExternalSymbol => 8,
        RelocKind::PcRel32
        | RelocKind::Plt32
        | RelocKind::Branch26
        | RelocKind::Adrp21
        | RelocKind::Add12
        | RelocKind::CondBranch19 => 4,
    }
}

/// Relocation operations in the native FASL v1 format.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FaslRelocationKind {
    /// Absolute 64-bit code-object or native-function address.
    Abs64,
    /// Signed 32-bit PC-relative displacement to code.
    PcRel32,
    /// Signed 32-bit PLT displacement to an imported function.
    Plt32,
    /// `AArch64` scaled unconditional-branch displacement.
    Branch26,
    /// First half of an `AArch64` page-address pair.
    Adrp21,
    /// Second half of an `AArch64` page-address pair.
    Add12,
    /// `AArch64` scaled conditional-branch displacement.
    CondBranch19,
    /// Absolute code-object entry address.
    CodeEntry,
    /// Absolute imported-function address.
    ExternalSymbol,
    /// Non-pointer constant-vector index.
    ConstantIndex32,
}

impl FaslRelocationKind {
    const fn tag(self) -> u32 {
        match self {
            Self::Abs64 => 0,
            Self::PcRel32 => 1,
            Self::Plt32 => 2,
            Self::Branch26 => 3,
            Self::Adrp21 => 4,
            Self::Add12 => 5,
            Self::CondBranch19 => 6,
            Self::CodeEntry => 7,
            Self::ExternalSymbol => 8,
            Self::ConstantIndex32 => 9,
        }
    }

    const fn from_tag(tag: u32, offset: usize) -> Result<Self, FaslError> {
        match tag {
            0 => Ok(Self::Abs64),
            1 => Ok(Self::PcRel32),
            2 => Ok(Self::Plt32),
            3 => Ok(Self::Branch26),
            4 => Ok(Self::Adrp21),
            5 => Ok(Self::Add12),
            6 => Ok(Self::CondBranch19),
            7 => Ok(Self::CodeEntry),
            8 => Ok(Self::ExternalSymbol),
            9 => Ok(Self::ConstantIndex32),
            10..=u32::MAX => Err(FaslError::at(FaslRejectReason::InvalidRelocation, offset)),
        }
    }

    /// Returns the number of bytes written at a relocation site.
    #[must_use]
    pub const fn width(self) -> u32 {
        match self {
            Self::Abs64 | Self::CodeEntry | Self::ExternalSymbol => 8,
            Self::PcRel32
            | Self::Plt32
            | Self::Branch26
            | Self::Adrp21
            | Self::Add12
            | Self::CondBranch19
            | Self::ConstantIndex32 => 4,
        }
    }
}

/// Relocation target category in native FASL v1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FaslRelocationTarget {
    /// A dense ID in the code-object section.
    CodeObject,
    /// A symbol ID bound to an imported native function.
    NativeFunction,
    /// A constant-vector index in the owning code object.
    ConstantVector,
}

impl FaslRelocationTarget {
    const fn tag(self) -> u32 {
        match self {
            Self::CodeObject => 1,
            Self::NativeFunction => 2,
            Self::ConstantVector => 3,
        }
    }

    const fn from_tag(tag: u32, offset: usize) -> Result<Self, FaslError> {
        match tag {
            1 => Ok(Self::CodeObject),
            2 => Ok(Self::NativeFunction),
            3 => Ok(Self::ConstantVector),
            0 | 4..=u32::MAX => Err(FaslError::at(FaslRejectReason::InvalidRelocation, offset)),
        }
    }
}

/// One FASL v1 relocation record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FaslRelocation {
    /// Code object containing the patch site.
    pub code_object_id: u32,
    /// Byte offset of the patch site within its code object.
    pub write_offset: u32,
    /// Relocation operation.
    pub kind: FaslRelocationKind,
    /// Category of the referenced target.
    pub target_kind: FaslRelocationTarget,
    /// Code-object ID, symbol ID, or constant index selected by `target_kind`.
    pub target_index: u32,
    /// Signed adjustment applied to the selected target.
    pub addend: i64,
    /// `AArch64` address-pair identifier, or zero for an unpaired relocation.
    pub pair_id: u32,
}

/// Decodes the count-prefixed fixed-width FASL v1 relocation section.
pub fn decode_v1_section(
    bytes: &[u8],
    base_offset: usize,
) -> Result<Vec<FaslRelocation>, FaslError> {
    let mut reader = SectionReader::new(bytes, base_offset, FaslRejectReason::InvalidRelocation);
    let count_offset = reader.absolute_position();
    let count = bounded_count(reader.u32()?, count_offset)?;
    let expected_length = count
        .checked_mul(FASL_RELOCATION_RECORD_SIZE)
        .and_then(|size| size.checked_add(4))
        .ok_or_else(|| FaslError::at(FaslRejectReason::ResourceLimitExceeded, count_offset))?;
    if expected_length != bytes.len() {
        return Err(FaslError::at(
            FaslRejectReason::InvalidRelocation,
            count_offset,
        ));
    }
    let mut items = Vec::with_capacity(count);
    for _ in 0..count {
        let record_offset = reader.absolute_position();
        let code_object_id = reader.u32()?;
        let write_offset = reader.u32()?;
        let kind_offset = reader.absolute_position();
        let kind = FaslRelocationKind::from_tag(reader.u32()?, kind_offset)?;
        let target_kind_offset = reader.absolute_position();
        let target_kind = FaslRelocationTarget::from_tag(reader.u32()?, target_kind_offset)?;
        let target_index = reader.u32()?;
        let addend = reader.i64()?;
        let pair_id = reader.u32()?;
        let relocation = FaslRelocation {
            code_object_id,
            write_offset,
            kind,
            target_kind,
            target_index,
            addend,
            pair_id,
        };
        if items.last().is_some_and(|previous: &FaslRelocation| {
            (previous.code_object_id, previous.write_offset)
                >= (relocation.code_object_id, relocation.write_offset)
        }) {
            return Err(FaslError::at(
                FaslRejectReason::InvalidRelocation,
                record_offset,
            ));
        }
        items.push(relocation);
    }
    reader.finish()?;
    Ok(items)
}

/// Encodes FASL v1 relocation records in canonical code-object/site order.
pub fn encode_v1_section(items: &[FaslRelocation]) -> Result<Vec<u8>, FaslError> {
    if items.len() > MAX_FASL_RECORDS {
        return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
    }
    let count = u32::try_from(items.len())
        .map_err(|_| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?;
    let mut ordered = items.to_vec();
    ordered.sort_by_key(|item| (item.code_object_id, item.write_offset));
    if ordered.windows(2).any(|pair| {
        matches!(pair, [left, right] if left.code_object_id == right.code_object_id && left.write_offset == right.write_offset)
    }) {
        return Err(FaslError::new(FaslRejectReason::InvalidRelocation));
    }
    let mut writer = SectionWriter::new();
    writer.put_u32(count);
    for item in ordered {
        writer.put_u32(item.code_object_id);
        writer.put_u32(item.write_offset);
        writer.put_u32(item.kind.tag());
        writer.put_u32(item.target_kind.tag());
        writer.put_u32(item.target_index);
        writer.put_i64(item.addend);
        writer.put_u32(item.pair_id);
    }
    Ok(writer.into_bytes())
}

fn bounded_count(count: u32, offset: usize) -> Result<usize, FaslError> {
    let count = usize::try_from(count)
        .map_err(|_| FaslError::at(FaslRejectReason::ResourceLimitExceeded, offset))?;
    if count > MAX_FASL_RECORDS {
        return Err(FaslError::at(
            FaslRejectReason::ResourceLimitExceeded,
            offset,
        ));
    }
    Ok(count)
}
