use crate::error::{FaslError, FaslRejectReason};
use crate::fasl::Architecture;
use crate::fasl_code::FaslCodeObject;
use crate::fasl_constants::{FaslConstant, FaslConstantGroup};
use crate::fasl_debug::{self, DebugRecord};
use crate::fasl_records::FaslCodeLayout;
use crate::fasl_relocation::{FaslRelocation, FaslRelocationKind, FaslRelocationTarget};
use crate::fasl_stack_map::SafepointGroup;
use crate::fasl_symbols::{FaslSymbol, SymbolBindingKind};
use crate::fasl_top_level::FaslTopLevelRecord;

const MAX_RECORDS: usize = 1_048_576;

pub fn validate_sections(
    fasl: &crate::fasl::FaslV1<'_>,
    relocation_section_offset: Option<usize>,
) -> Result<(), FaslError> {
    if fasl.code.len() > MAX_RECORDS {
        return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
    }
    let layouts = code_layouts(&fasl.code);
    validate_record_counts(
        &fasl.code,
        &fasl.constants,
        &fasl.stack_maps,
        &fasl.debug_records,
    )?;
    validate_references(&fasl.constants, &fasl.symbols)?;
    validate_stack_maps(&fasl.stack_maps, &layouts)?;
    fasl_debug::validate(&fasl.debug_files, &fasl.debug_records, &layouts)?;
    validate_top_level(&fasl.top_level, &layouts)?;
    validate_relocations(
        fasl.architecture,
        &fasl.relocations,
        &layouts,
        &fasl.symbols,
        relocation_section_offset,
    )
}

/// Checks symbol and intra-object constant references after all sections are parsed.
pub fn validate_references(
    groups: &[FaslConstantGroup],
    symbols: &[FaslSymbol],
) -> Result<(), FaslError> {
    for group in groups {
        let count = u32::try_from(group.constants.len())
            .map_err(|_| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?;
        for constant in &group.constants {
            match constant {
                FaslConstant::SymbolReference(symbol_id) => {
                    let symbol = usize::try_from(*symbol_id)
                        .ok()
                        .and_then(|id| symbols.get(id));
                    if symbol.is_none_or(|symbol| {
                        symbol.id != *symbol_id || symbol.binding_kind != SymbolBindingKind::Lisp
                    }) {
                        return Err(FaslError::new(FaslRejectReason::InvalidReference));
                    }
                }
                FaslConstant::Cons { car, cdr }
                | FaslConstant::Ratio {
                    numerator: car,
                    denominator: cdr,
                }
                | FaslConstant::Complex {
                    real: car,
                    imaginary: cdr,
                } if *car >= count || *cdr >= count => {
                    return Err(FaslError::new(FaslRejectReason::InvalidReference));
                }
                FaslConstant::SimpleVector(indices)
                    if indices.iter().any(|index| *index >= count) =>
                {
                    return Err(FaslError::new(FaslRejectReason::InvalidReference));
                }
                FaslConstant::Nil
                | FaslConstant::Truth
                | FaslConstant::Fixnum(_)
                | FaslConstant::Character(_)
                | FaslConstant::SingleFloat(_)
                | FaslConstant::DoubleFloat(_)
                | FaslConstant::String(_)
                | FaslConstant::Cons { .. }
                | FaslConstant::SimpleVector(_)
                | FaslConstant::Bignum { .. }
                | FaslConstant::Ratio { .. }
                | FaslConstant::Complex { .. } => {}
            }
        }
    }
    Ok(())
}

fn code_layouts<'a>(code: &[FaslCodeObject<'a>]) -> Vec<FaslCodeLayout<'a>> {
    code.iter()
        .map(|object| FaslCodeLayout {
            id: object.id,
            entry_offset: object.entry_offset,
            frame_words: object.frame_words,
            constant_count: object.constant_count,
            code: object.code,
        })
        .collect()
}

fn validate_record_counts(
    code: &[FaslCodeObject<'_>],
    constants: &[FaslConstantGroup],
    stack_maps: &[SafepointGroup],
    debug_records: &[DebugRecord],
) -> Result<(), FaslError> {
    if debug_records.len() > MAX_RECORDS {
        return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
    }
    if constants.len() != code.len() || stack_maps.len() != code.len() {
        return Err(FaslError::new(FaslRejectReason::InvalidCodeObject));
    }
    let mut debug_counts = vec![0_u32; code.len()];
    for record in debug_records {
        let index = usize::try_from(record.code_object_id)
            .map_err(|_| FaslError::new(FaslRejectReason::InvalidReference))?;
        let count = debug_counts
            .get_mut(index)
            .ok_or_else(|| FaslError::new(FaslRejectReason::InvalidReference))?;
        *count = count
            .checked_add(1)
            .ok_or_else(|| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?;
    }
    for (index, object) in code.iter().enumerate() {
        let Some(constant_group) = constants.get(index) else {
            return Err(FaslError::new(FaslRejectReason::InvalidCodeObject));
        };
        let Some(stack_group) = stack_maps.get(index) else {
            return Err(FaslError::new(FaslRejectReason::InvalidCodeObject));
        };
        if usize::try_from(object.id).ok() != Some(index)
            || constant_group.code_object_id != object.id
            || u32::try_from(constant_group.constants.len()).ok() != Some(object.constant_count)
            || stack_group.code_object_id != object.id
            || u32::try_from(stack_group.entries.len()).ok() != Some(object.stack_map_count)
        {
            return Err(FaslError::new(FaslRejectReason::InvalidCodeObject));
        }
        if debug_counts.get(index).copied() != Some(object.debug_record_count) {
            return Err(FaslError::new(FaslRejectReason::InvalidCodeObject));
        }
    }
    Ok(())
}

fn validate_stack_maps(
    groups: &[SafepointGroup],
    layouts: &[FaslCodeLayout<'_>],
) -> Result<(), FaslError> {
    if groups.len() != layouts.len() {
        return Err(FaslError::new(FaslRejectReason::InvalidSafepointMap));
    }
    for (index, group) in groups.iter().enumerate() {
        let Some(layout) = layouts.get(index) else {
            return Err(FaslError::new(FaslRejectReason::InvalidSafepointMap));
        };
        if usize::try_from(group.code_object_id).ok() != Some(index)
            || group.code_object_id != layout.id
        {
            return Err(FaslError::new(FaslRejectReason::InvalidSafepointMap));
        }
        let code_length = u32::try_from(layout.code.len())
            .map_err(|_| FaslError::new(FaslRejectReason::InvalidSafepointMap))?;
        for entry in &group.entries {
            if entry.pc_offset >= code_length || entry.frame_words != layout.frame_words {
                return Err(FaslError::new(FaslRejectReason::InvalidSafepointMap));
            }
        }
    }
    Ok(())
}

fn validate_top_level(
    records: &[FaslTopLevelRecord],
    layouts: &[FaslCodeLayout<'_>],
) -> Result<(), FaslError> {
    if records.len() > MAX_RECORDS {
        return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
    }
    for (ordinal, record) in records.iter().enumerate() {
        let layout = code_layout(
            layouts,
            record.code_object_id,
            FaslRejectReason::InvalidTopLevelRecord,
        )?;
        if usize::try_from(record.ordinal).ok() != Some(ordinal)
            || record.flags & !3 != 0
            || record.flags & 2 != 0 && record.flags & 1 == 0
            || record.entry_offset != layout.entry_offset
        {
            return Err(FaslError::new(FaslRejectReason::InvalidTopLevelRecord));
        }
    }
    Ok(())
}

fn validate_relocations(
    architecture: Architecture,
    relocations: &[FaslRelocation],
    layouts: &[FaslCodeLayout<'_>],
    symbols: &[FaslSymbol],
    section_offset: Option<usize>,
) -> Result<(), FaslError> {
    if relocations.len() > MAX_RECORDS {
        return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
    }
    let mut ordered = relocations.to_vec();
    ordered.sort_by_key(|item| (item.code_object_id, item.write_offset));
    let mut pairs = Vec::new();
    let mut previous_end = None;
    for (index, relocation) in ordered.iter().enumerate() {
        let record_offset = section_offset.and_then(|base| {
            index
                .checked_mul(32)
                .and_then(|delta| base.checked_add(4 + delta))
        });
        let source = code_layout(
            layouts,
            relocation.code_object_id,
            FaslRejectReason::InvalidReference,
        )
        .map_err(|_| rejection(FaslRejectReason::InvalidReference, record_offset))?;
        let width = usize::try_from(relocation.kind.width())
            .map_err(|_| rejection(FaslRejectReason::InvalidRelocation, record_offset))?;
        let offset = usize::try_from(relocation.write_offset)
            .map_err(|_| rejection(FaslRejectReason::InvalidRelocation, record_offset))?;
        let end = offset
            .checked_add(width)
            .ok_or_else(|| rejection(FaslRejectReason::InvalidRelocation, record_offset))?;
        if end > source.code.len()
            || previous_end
                .is_some_and(|(id, old_end)| id == relocation.code_object_id && offset < old_end)
            || !target_kind_allowed(relocation.kind, relocation.target_kind)
            || !architecture_allows(architecture, relocation.kind)
        {
            return Err(rejection(
                FaslRejectReason::InvalidRelocation,
                record_offset,
            ));
        }
        validate_target(relocation, source, layouts, symbols, record_offset)?;
        if needs_instruction_alignment(relocation.kind)
            && !relocation.write_offset.is_multiple_of(4)
        {
            return Err(rejection(
                FaslRejectReason::InvalidRelocation,
                record_offset,
            ));
        }
        if relocation.pair_id != 0 {
            if !matches!(
                relocation.kind,
                FaslRelocationKind::Adrp21 | FaslRelocationKind::Add12
            ) {
                return Err(rejection(
                    FaslRejectReason::InvalidRelocation,
                    record_offset,
                ));
            }
            pairs.push((relocation.pair_id, index));
        }
        previous_end = Some((relocation.code_object_id, end));
    }
    validate_relocation_pairs(&ordered, &pairs, layouts, section_offset)
}

const fn target_kind_allowed(kind: FaslRelocationKind, target: FaslRelocationTarget) -> bool {
    use FaslRelocationKind as Kind;
    use FaslRelocationTarget as Target;
    match kind {
        Kind::Abs64 | Kind::Branch26 | Kind::Adrp21 | Kind::Add12 => {
            matches!(target, Target::CodeObject | Target::NativeFunction)
        }
        Kind::PcRel32 | Kind::CondBranch19 | Kind::CodeEntry => {
            matches!(target, Target::CodeObject)
        }
        Kind::Plt32 | Kind::ExternalSymbol => matches!(target, Target::NativeFunction),
        Kind::ConstantIndex32 => matches!(target, Target::ConstantVector),
    }
}

const fn architecture_allows(architecture: Architecture, kind: FaslRelocationKind) -> bool {
    use FaslRelocationKind as Kind;
    match architecture {
        Architecture::X86_64 => !matches!(
            kind,
            Kind::Branch26 | Kind::Adrp21 | Kind::Add12 | Kind::CondBranch19
        ),
        Architecture::Aarch64 => !matches!(kind, Kind::PcRel32 | Kind::Plt32),
    }
}

const fn needs_instruction_alignment(kind: FaslRelocationKind) -> bool {
    matches!(
        kind,
        FaslRelocationKind::Branch26
            | FaslRelocationKind::Adrp21
            | FaslRelocationKind::Add12
            | FaslRelocationKind::CondBranch19
    )
}

fn validate_target(
    relocation: &FaslRelocation,
    source: &FaslCodeLayout<'_>,
    layouts: &[FaslCodeLayout<'_>],
    symbols: &[FaslSymbol],
    record_offset: Option<usize>,
) -> Result<(), FaslError> {
    match relocation.target_kind {
        FaslRelocationTarget::CodeObject => {
            let index = usize::try_from(relocation.target_index)
                .map_err(|_| rejection(FaslRejectReason::InvalidReference, record_offset))?;
            let target = layouts
                .get(index)
                .ok_or_else(|| rejection(FaslRejectReason::InvalidReference, record_offset))?;
            if target.id != relocation.target_index {
                return Err(rejection(FaslRejectReason::InvalidReference, record_offset));
            }
            let base = if relocation.kind == FaslRelocationKind::Abs64 {
                0
            } else {
                i64::from(target.entry_offset)
            };
            let target_offset = base
                .checked_add(relocation.addend)
                .ok_or_else(|| rejection(FaslRejectReason::InvalidRelocation, record_offset))?;
            if target_offset < 0
                || usize::try_from(target_offset)
                    .ok()
                    .is_none_or(|offset| offset >= target.code.len())
            {
                return Err(rejection(
                    FaslRejectReason::InvalidRelocation,
                    record_offset,
                ));
            }
        }
        FaslRelocationTarget::NativeFunction => {
            let valid = usize::try_from(relocation.target_index)
                .ok()
                .and_then(|index| symbols.get(index))
                .is_some_and(|symbol| {
                    symbol.id == relocation.target_index
                        && symbol.binding_kind == SymbolBindingKind::NativeFunction
                });
            if !valid {
                return Err(rejection(FaslRejectReason::InvalidReference, record_offset));
            }
        }
        FaslRelocationTarget::ConstantVector => {
            if relocation.kind != FaslRelocationKind::ConstantIndex32 || relocation.addend != 0 {
                return Err(rejection(
                    FaslRejectReason::InvalidRelocation,
                    record_offset,
                ));
            }
            if relocation.target_index >= source.constant_count {
                return Err(rejection(FaslRejectReason::InvalidReference, record_offset));
            }
        }
    }
    Ok(())
}

fn validate_relocation_pairs(
    relocations: &[FaslRelocation],
    pairs: &[(u32, usize)],
    layouts: &[FaslCodeLayout<'_>],
    section_offset: Option<usize>,
) -> Result<(), FaslError> {
    let mut ordered_pairs = pairs.to_vec();
    ordered_pairs.sort_unstable_by_key(|(pair_id, index)| (*pair_id, *index));
    let mut start = 0;
    while start < ordered_pairs.len() {
        let pair = ordered_pairs
            .get(start)
            .copied()
            .ok_or_else(|| FaslError::new(FaslRejectReason::InvalidRelocation))?;
        let pair_id = pair.0;
        let suffix = ordered_pairs
            .get(start..)
            .ok_or_else(|| pair_error(section_offset, pair.1))?;
        let end = suffix
            .iter()
            .position(|(candidate, _)| *candidate != pair_id)
            .map_or(ordered_pairs.len(), |length| start + length);
        if end - start != 2 {
            return Err(pair_error(section_offset, pair.1));
        }
        let first_index = pair.1;
        let second_position = start
            .checked_add(1)
            .ok_or_else(|| pair_error(section_offset, first_index))?;
        let second_index = ordered_pairs
            .get(second_position)
            .map(|(_, index)| *index)
            .ok_or_else(|| pair_error(section_offset, first_index))?;
        let Some(first) = relocations.get(first_index) else {
            return Err(pair_error(section_offset, first_index));
        };
        let Some(second) = relocations.get(second_index) else {
            return Err(pair_error(section_offset, second_index));
        };
        if first.kind != FaslRelocationKind::Adrp21
            || second.kind != FaslRelocationKind::Add12
            || first.code_object_id != second.code_object_id
            || first.write_offset.checked_add(4) != Some(second.write_offset)
            || first.target_kind != second.target_kind
            || first.target_index != second.target_index
            || first.addend != second.addend
            || !pair_immediates_are_zero(first, second, layouts)
        {
            return Err(pair_error(section_offset, first_index));
        }
        start = end;
    }
    Ok(())
}

fn pair_immediates_are_zero(
    adrp: &FaslRelocation,
    add: &FaslRelocation,
    layouts: &[FaslCodeLayout<'_>],
) -> bool {
    let Some(code) = code_layout(
        layouts,
        adrp.code_object_id,
        FaslRejectReason::InvalidRelocation,
    )
    .ok() else {
        return false;
    };
    let Some(adrp_word) = instruction_word(code.code, adrp.write_offset) else {
        return false;
    };
    let Some(add_word) = instruction_word(code.code, add.write_offset) else {
        return false;
    };
    let adrp_immediate_mask = (0b11_u32 << 29) | (0x7ffff_u32 << 5);
    let add_immediate_mask = 0xfff_u32 << 10;
    let add_shift_mask = 1_u32 << 22;
    adrp_word & adrp_immediate_mask == 0 && add_word & (add_immediate_mask | add_shift_mask) == 0
}

fn instruction_word(code: &[u8], offset: u32) -> Option<u32> {
    let start = usize::try_from(offset).ok()?;
    let end = start.checked_add(4)?;
    let bytes = code.get(start..end)?;
    let mut word = [0; 4];
    word.copy_from_slice(bytes);
    Some(u32::from_le_bytes(word))
}

fn code_layout<'a, 'b>(
    layouts: &'a [FaslCodeLayout<'b>],
    id: u32,
    reason: FaslRejectReason,
) -> Result<&'a FaslCodeLayout<'b>, FaslError> {
    let index = usize::try_from(id).map_err(|_| FaslError::new(reason))?;
    layouts
        .get(index)
        .filter(|layout| layout.id == id)
        .ok_or_else(|| FaslError::new(reason))
}

fn pair_error(section_offset: Option<usize>, index: usize) -> FaslError {
    let offset = section_offset.and_then(|base| {
        index
            .checked_mul(32)
            .and_then(|delta| base.checked_add(4 + delta))
    });
    rejection(FaslRejectReason::InvalidRelocation, offset)
}

fn rejection(reason: FaslRejectReason, offset: Option<usize>) -> FaslError {
    offset.map_or_else(
        || FaslError::new(reason),
        |offset| FaslError::at(reason, offset),
    )
}

#[cfg(test)]
#[path = "fasl_validate_tests.rs"]
mod private_tests;

#[cfg(test)]
#[path = "fasl_validate_relocation_tests.rs"]
mod relocation_tests;
