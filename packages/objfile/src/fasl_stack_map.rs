use crate::error::{FaslError, FaslRejectReason};
use crate::fasl_records::{FaslCodeLayout, SectionReader, SectionWriter};

const HEADER_SIZE: usize = 16;
const MAX_RECORDS: usize = 1_048_576;

/// One decoded entry in the existing `SafepointMap` wire representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SafepointEntry {
    /// Code-relative safepoint PC.
    pub pc_offset: u32,
    /// Number of words in the native frame.
    pub frame_words: u16,
    /// Number of frame slots represented by the bitmap.
    pub slot_words: u16,
    /// Number of bitmap slots containing Lisp words.
    pub word_slot_count: u16,
    /// Saved-register mask, in the target's register numbering.
    pub register_mask: u16,
    /// Backend-specific map flags.
    pub map_flags: u32,
    /// Live-slot bitmap bytes.
    pub slot_bitmap: Vec<u8>,
    /// Register IDs in increasing mask-bit order.
    pub register_ids: Vec<u16>,
}

/// The stack-map entries belonging to one code object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SafepointGroup {
    /// Dense code-object identifier.
    pub code_object_id: u32,
    /// Safepoints in strictly increasing PC order.
    pub entries: Vec<SafepointEntry>,
}

/// Decodes and validates the canonical stack-map section.
pub fn decode_section(
    bytes: &[u8],
    base_offset: usize,
    code_objects: &[FaslCodeLayout<'_>],
) -> Result<Vec<SafepointGroup>, FaslError> {
    let mut cursor = SectionReader::new(bytes, base_offset, FaslRejectReason::InvalidSafepointMap);
    let group_count_offset = cursor.absolute_position();
    let group_count = bounded_count(cursor.u32()?, group_count_offset)?;
    if group_count != code_objects.len() || group_count > cursor.remaining() / 8 {
        return Err(FaslError::at(
            FaslRejectReason::InvalidSafepointMap,
            group_count_offset,
        ));
    }
    let mut groups = Vec::with_capacity(group_count);
    for expected_id in 0..group_count {
        let id_offset = cursor.absolute_position();
        let code_object_id = cursor.u32()?;
        if usize::try_from(code_object_id).ok() != Some(expected_id) {
            return Err(FaslError::at(
                FaslRejectReason::InvalidSafepointMap,
                id_offset,
            ));
        }
        let Some(code_object) = code_objects.get(expected_id) else {
            return Err(FaslError::at(
                FaslRejectReason::InvalidSafepointMap,
                id_offset,
            ));
        };
        if code_object.id != code_object_id {
            return Err(FaslError::at(
                FaslRejectReason::InvalidSafepointMap,
                id_offset,
            ));
        }
        let code_length = u32::try_from(code_object.code.len())
            .map_err(|_| FaslError::at(FaslRejectReason::ResourceLimitExceeded, id_offset))?;
        let count_offset = cursor.absolute_position();
        let entry_count = bounded_count(cursor.u32()?, count_offset)?;
        if entry_count > cursor.remaining() / HEADER_SIZE {
            return Err(FaslError::at(
                FaslRejectReason::InvalidSafepointMap,
                count_offset,
            ));
        }
        let mut entries = Vec::with_capacity(entry_count);
        let mut previous_pc = None;
        for _ in 0..entry_count {
            let entry_offset = cursor.absolute_position();
            let entry = decode_entry(&mut cursor)?;
            if previous_pc.is_some_and(|pc| entry.pc_offset <= pc)
                || entry.pc_offset >= code_length
                || entry.frame_words != code_object.frame_words
            {
                return Err(FaslError::at(
                    FaslRejectReason::InvalidSafepointMap,
                    entry_offset,
                ));
            }
            previous_pc = Some(entry.pc_offset);
            entries.push(entry);
        }
        groups.push(SafepointGroup {
            code_object_id,
            entries,
        });
    }
    cursor.finish()?;
    Ok(groups)
}

/// Encodes stack-map groups in code-object and increasing-PC order.
pub fn encode_section(groups: &[SafepointGroup]) -> Result<Vec<u8>, FaslError> {
    if groups.len() > MAX_RECORDS {
        return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
    }
    let group_count = u32::try_from(groups.len())
        .map_err(|_| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?;
    let mut ordered_groups = groups.to_vec();
    ordered_groups.sort_by_key(|group| group.code_object_id);
    if ordered_groups
        .windows(2)
        .any(|pair| matches!(pair, [left, right] if left.code_object_id == right.code_object_id))
    {
        return Err(FaslError::new(FaslRejectReason::InvalidSafepointMap));
    }
    let mut out = SectionWriter::new();
    out.put_u32(group_count);
    for (expected_id, group) in ordered_groups.iter_mut().enumerate() {
        if usize::try_from(group.code_object_id).ok() != Some(expected_id)
            || group.entries.len() > MAX_RECORDS
        {
            return Err(FaslError::new(FaslRejectReason::InvalidSafepointMap));
        }
        group.entries.sort_by_key(|entry| entry.pc_offset);
        validate_entries(&group.entries)?;
        let entry_count = u32::try_from(group.entries.len())
            .map_err(|_| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?;
        out.put_u32(group.code_object_id);
        out.put_u32(entry_count);
        for entry in &group.entries {
            encode_entry(entry, &mut out)?;
        }
    }
    Ok(out.into_bytes())
}

fn decode_entry(cursor: &mut SectionReader<'_>) -> Result<SafepointEntry, FaslError> {
    let pc_offset = cursor.u32()?;
    let frame_words = cursor.u16()?;
    let slot_words = cursor.u16()?;
    let word_slot_count = cursor.u16()?;
    let register_mask = cursor.u16()?;
    let map_flags = cursor.u32()?;
    if slot_words > frame_words || slot_words < 3 || word_slot_count > slot_words {
        return Err(FaslError::at(
            FaslRejectReason::InvalidSafepointMap,
            cursor.absolute_position().saturating_sub(HEADER_SIZE),
        ));
    }
    let bitmap_len = usize::from(slot_words).div_ceil(8);
    let bitmap_offset = cursor.absolute_position();
    let slot_bitmap = cursor.take(bitmap_len)?.to_vec();
    if slot_bitmap
        .first()
        .is_none_or(|bits| bits & 0b0100 == 0 || bits & 0b1011 != 0)
        || unused_bitmap_bits_are_set(&slot_bitmap, slot_words)
    {
        return Err(FaslError::at(
            FaslRejectReason::InvalidSafepointMap,
            bitmap_offset,
        ));
    }
    let register_count = usize::try_from(register_mask.count_ones()).map_err(|_| {
        FaslError::at(
            FaslRejectReason::InvalidSafepointMap,
            cursor.absolute_position(),
        )
    })?;
    let mut register_ids = Vec::with_capacity(register_count);
    for bit in 0..u16::BITS {
        let Some(mask) = 1_u16.checked_shl(bit) else {
            return Err(FaslError::at(
                FaslRejectReason::InvalidSafepointMap,
                cursor.absolute_position(),
            ));
        };
        if register_mask & mask != 0 {
            register_ids.push(cursor.u16()?);
        }
    }
    if register_ids
        .iter()
        .enumerate()
        .any(|(bit, id)| Some(usize::from(*id)) != bit_for_register(register_mask, bit))
    {
        return Err(FaslError::at(
            FaslRejectReason::InvalidSafepointMap,
            cursor
                .absolute_position()
                .saturating_sub(register_count * 2),
        ));
    }
    Ok(SafepointEntry {
        pc_offset,
        frame_words,
        slot_words,
        word_slot_count,
        register_mask,
        map_flags,
        slot_bitmap,
        register_ids,
    })
}

fn encode_entry(entry: &SafepointEntry, out: &mut SectionWriter) -> Result<(), FaslError> {
    validate_entry(entry)?;
    out.put_u32(entry.pc_offset);
    out.put_u16(entry.frame_words);
    out.put_u16(entry.slot_words);
    out.put_u16(entry.word_slot_count);
    out.put_u16(entry.register_mask);
    out.put_u32(entry.map_flags);
    out.put_bytes(&entry.slot_bitmap);
    for register_id in &entry.register_ids {
        out.put_u16(*register_id);
    }
    Ok(())
}

fn validate_entries(entries: &[SafepointEntry]) -> Result<(), FaslError> {
    if entries.len() > MAX_RECORDS {
        return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
    }
    let mut previous_pc = None;
    for entry in entries {
        if previous_pc.is_some_and(|pc| entry.pc_offset <= pc) {
            return Err(FaslError::new(FaslRejectReason::InvalidSafepointMap));
        }
        validate_entry(entry)?;
        previous_pc = Some(entry.pc_offset);
    }
    Ok(())
}

fn validate_entry(entry: &SafepointEntry) -> Result<(), FaslError> {
    let expected_bitmap_len = usize::from(entry.slot_words).div_ceil(8);
    if entry.slot_words < 3
        || entry.slot_words > entry.frame_words
        || entry.word_slot_count > entry.slot_words
        || entry.slot_bitmap.len() != expected_bitmap_len
        || entry
            .slot_bitmap
            .first()
            .is_none_or(|bits| bits & 0b0100 == 0 || bits & 0b1011 != 0)
        || unused_bitmap_bits_are_set(&entry.slot_bitmap, entry.slot_words)
    {
        return Err(FaslError::new(FaslRejectReason::InvalidSafepointMap));
    }
    let mut expected_ids = Vec::new();
    for bit in 0..u16::BITS {
        let mask = 1_u16
            .checked_shl(bit)
            .ok_or_else(|| FaslError::new(FaslRejectReason::InvalidSafepointMap))?;
        if entry.register_mask & mask != 0 {
            expected_ids.push(
                u16::try_from(bit)
                    .map_err(|_| FaslError::new(FaslRejectReason::InvalidSafepointMap))?,
            );
        }
    }
    if expected_ids != entry.register_ids {
        return Err(FaslError::new(FaslRejectReason::InvalidSafepointMap));
    }
    Ok(())
}

fn bit_for_register(mask: u16, ordinal: usize) -> Option<usize> {
    (0..u16::BITS)
        .filter(|bit| mask & (1_u16 << bit) != 0)
        .nth(ordinal)
        .and_then(|bit| usize::try_from(bit).ok())
}

const fn unused_bitmap_bits_are_set(bitmap: &[u8], slot_words: u16) -> bool {
    let used = slot_words % 8;
    if used == 0 {
        return false;
    }
    let Some(last) = bitmap.last() else {
        return true;
    };
    *last & !((1_u8 << used) - 1) != 0
}

fn bounded_count(count: u32, offset: usize) -> Result<usize, FaslError> {
    let count = usize::try_from(count)
        .map_err(|_| FaslError::at(FaslRejectReason::ResourceLimitExceeded, offset))?;
    if count > MAX_RECORDS {
        return Err(FaslError::at(
            FaslRejectReason::ResourceLimitExceeded,
            offset,
        ));
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> SafepointEntry {
        SafepointEntry {
            pc_offset: 7,
            frame_words: 8,
            slot_words: 5,
            word_slot_count: 2,
            register_mask: 0b1001,
            map_flags: 0,
            slot_bitmap: vec![0b0000_0100],
            register_ids: vec![0, 3],
        }
    }

    fn must_encode(groups: &[SafepointGroup]) -> Vec<u8> {
        encode_section(groups).unwrap_or_else(|error| panic!("valid stack map rejected: {error}"))
    }

    #[test]
    fn safepoint_header_matches_the_existing_sixteen_byte_layout() {
        let bytes = encode_section(&[SafepointGroup {
            code_object_id: 0,
            entries: vec![entry()],
        }]);
        assert_eq!(
            bytes,
            Ok(vec![
                1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 7, 0, 0, 0, 8, 0, 5, 0, 2, 0, 9, 0, 0, 0, 0, 0,
                4, 0, 0, 3, 0,
            ])
        );
    }

    #[test]
    fn safepoint_rejects_unsorted_pc_and_nonzero_unused_bitmap_bits() {
        let layout = [FaslCodeLayout {
            id: 0,
            entry_offset: 0,
            code: &[0; 16],
            frame_words: 8,
            constant_count: 0,
        }];
        let mut unsorted = encode_section(&[SafepointGroup {
            code_object_id: 0,
            entries: vec![
                entry(),
                SafepointEntry {
                    pc_offset: 8,
                    ..entry()
                },
            ],
        }])
        .unwrap_or_default();
        unsorted[33..37].copy_from_slice(&6_u32.to_le_bytes());
        assert!(matches!(
            decode_section(&unsorted, 0, &layout),
            Err(FaslError {
                reason: FaslRejectReason::InvalidSafepointMap,
                ..
            })
        ));

        let mut malformed = encode_section(&[SafepointGroup {
            code_object_id: 0,
            entries: vec![entry()],
        }])
        .unwrap_or_default();
        malformed[28] |= 0b1000_0000;
        assert!(matches!(
            decode_section(&malformed, 0, &layout),
            Err(FaslError {
                reason: FaslRejectReason::InvalidSafepointMap,
                ..
            })
        ));
    }

    #[test]
    fn safepoint_rejects_register_ids_that_do_not_match_the_mask() {
        let mut bad_ids = entry();
        bad_ids.register_ids = vec![0, 2];
        assert_eq!(
            encode_section(&[SafepointGroup {
                code_object_id: 0,
                entries: vec![bad_ids],
            }]),
            Err(FaslError::new(FaslRejectReason::InvalidSafepointMap))
        );

        let layout = [FaslCodeLayout {
            id: 0,
            entry_offset: 0,
            code: &[0; 16],
            frame_words: 8,
            constant_count: 0,
        }];
        let mut bytes = must_encode(&[SafepointGroup {
            code_object_id: 0,
            entries: vec![entry()],
        }]);
        bytes[31..33].copy_from_slice(&2_u16.to_le_bytes());
        assert_eq!(
            decode_section(&bytes, 300, &layout),
            Err(FaslError::at(FaslRejectReason::InvalidSafepointMap, 329))
        );
    }

    #[test]
    fn safepoint_encoder_rejects_duplicate_pcs() {
        assert_eq!(
            encode_section(&[SafepointGroup {
                code_object_id: 0,
                entries: vec![entry(), entry()],
            }]),
            Err(FaslError::new(FaslRejectReason::InvalidSafepointMap))
        );
    }

    #[test]
    fn safepoint_with_eight_slots_round_trips_zero_unused_bits() {
        let groups = [SafepointGroup {
            code_object_id: 0,
            entries: vec![SafepointEntry {
                pc_offset: 7,
                frame_words: 8,
                slot_words: 8,
                word_slot_count: 1,
                register_mask: 0,
                map_flags: 0,
                slot_bitmap: vec![0b0000_0100],
                register_ids: Vec::new(),
            }],
        }];
        let layout = [FaslCodeLayout {
            id: 0,
            entry_offset: 0,
            code: &[0; 8],
            frame_words: 8,
            constant_count: 0,
        }];
        let bytes = must_encode(&groups);
        assert_eq!(decode_section(&bytes, 0, &layout), Ok(groups.to_vec()));
    }
}
