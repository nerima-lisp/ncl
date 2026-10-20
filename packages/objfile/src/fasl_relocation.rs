use crate::{ObjectError, RelocKind, Relocation, SectionId, SymbolRef};

pub(super) fn encode(items: &[Relocation]) -> Result<Vec<u8>, ObjectError> {
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

pub(super) fn decode(
    bytes: &[u8],
    version: u16,
    count: u32,
) -> Result<Vec<Relocation>, ObjectError> {
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

pub(super) const fn kind_number(kind: RelocKind) -> u32 {
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

pub(super) fn number_kind(value: u32) -> Result<RelocKind, ObjectError> {
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

pub(super) const fn width(kind: RelocKind) -> u32 {
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
