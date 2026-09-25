use super::registry::CodeRegistry;
use crate::Word;

/// One decoded safepoint entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Safepoint {
    /// Code-relative return-PC offset selected by this entry.
    pub pc_offset: u32,
    /// Number of words occupied by the complete frame.
    pub frame_words: u16,
    /// Number of frame words covered by the bitmap.
    pub slot_words: u16,
    /// Number of bitmap slots that contain Lisp words.
    pub word_slot_count: u16,
    /// Bit mask identifying callee-saved registers to scan.
    pub register_mask: u16,
    /// Backend-specific flags associated with this safepoint.
    pub map_flags: u32,
    /// Bitset of live frame slots, indexed from the frame header.
    pub slot_bitmap: Vec<u8>,
    /// Register indices corresponding to set bits in `register_mask`.
    pub register_ids: Vec<u16>,
}
/// Sorted safepoint metadata for one published code object.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SafepointMap {
    entries: Vec<Safepoint>,
}
/// The fixed four-word native frame header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameHeader {
    /// Word index of the caller frame, or zero at the chain terminus.
    pub previous: usize,
    /// Native return address used to resolve the code object and map.
    pub return_pc: usize,
    /// Function object associated with this invocation.
    pub function: Word,
    /// Backend-defined frame flags.
    pub flags: u64,
}

/// Read a bounded chain of four-word frame headers from a word slice.
#[must_use]
pub fn walk_frame_headers(words: &[Word], first: usize, limit: usize) -> Vec<FrameHeader> {
    let mut result = Vec::new();
    let mut at = first;
    while result.len() < limit && at.checked_add(3).is_some_and(|end| end < words.len()) {
        let header = FrameHeader {
            previous: words[at].address(),
            return_pc: usize::try_from(words[at + 1].bits()).unwrap_or(0),
            function: words[at + 2],
            flags: words[at + 3].bits(),
        };
        result.push(header);
        if header.previous == 0 || header.previous == at {
            break;
        }
        at = header.previous;
    }
    result
}

impl SafepointMap {
    /// Decode entries from the fixed little-endian wire representation.
    ///
    /// # Errors
    ///
    /// Returns an error when a header, bitmap, register list, or slot contract is invalid.
    pub fn decode(bytes: &[u8], count: usize) -> Result<Self, &'static str> {
        let mut at = 0;
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            if bytes.len().saturating_sub(at) < 16 {
                return Err("truncated safepoint header");
            }
            let pc = u32::from_le_bytes(bytes[at..at + 4].try_into().map_err(|_| "invalid pc")?);
            let frame = u16::from_le_bytes(
                bytes[at + 4..at + 6]
                    .try_into()
                    .map_err(|_| "invalid frame")?,
            );
            let slots = u16::from_le_bytes(
                bytes[at + 6..at + 8]
                    .try_into()
                    .map_err(|_| "invalid slots")?,
            );
            let words = u16::from_le_bytes(
                bytes[at + 8..at + 10]
                    .try_into()
                    .map_err(|_| "invalid word slots")?,
            );
            let mask = u16::from_le_bytes(
                bytes[at + 10..at + 12]
                    .try_into()
                    .map_err(|_| "invalid register mask")?,
            );
            let flags = u32::from_le_bytes(
                bytes[at + 12..at + 16]
                    .try_into()
                    .map_err(|_| "invalid flags")?,
            );
            if words > slots || slots < 3 {
                return Err("invalid slot counts");
            }
            let bitmap_len = usize::from(slots).div_ceil(8);
            at += 16;
            if bytes.len().saturating_sub(at) < bitmap_len {
                return Err("truncated slot bitmap");
            }
            let bitmap = bytes[at..at + bitmap_len].to_vec();
            at += bitmap_len;
            if bitmap
                .first()
                .is_none_or(|bits| bits & 0b0100 == 0 || bits & 0b1011 != 0)
            {
                return Err("invalid header bitmap");
            }
            let register_count =
                usize::try_from(mask.count_ones()).map_err(|_| "invalid register mask")?;
            if bytes.len().saturating_sub(at) < register_count * 2 {
                return Err("truncated register ids");
            }
            let mut ids = Vec::with_capacity(register_count);
            for _ in 0..register_count {
                ids.push(u16::from_le_bytes(
                    bytes[at..at + 2]
                        .try_into()
                        .map_err(|_| "invalid register id")?,
                ));
                at += 2;
            }
            let mut decoded_mask = 0_u16;
            for &register_id in &ids {
                if u32::from(register_id) >= u16::BITS {
                    return Err("invalid register id");
                }
                let bit = 1_u16 << register_id;
                if decoded_mask & bit != 0 || mask & bit == 0 {
                    return Err("register ids do not match register mask");
                }
                decoded_mask |= bit;
            }
            if decoded_mask != mask {
                return Err("register ids do not match register mask");
            }
            entries.push(Safepoint {
                pc_offset: pc,
                frame_words: frame,
                slot_words: slots,
                word_slot_count: words,
                register_mask: mask,
                map_flags: flags,
                slot_bitmap: bitmap,
                register_ids: ids,
            });
        }
        entries.sort_by_key(|entry| entry.pc_offset);
        Ok(Self { entries })
    }
    /// Find the nearest map at or before a code-relative PC offset.
    #[must_use]
    pub fn find_map(&self, pc_offset: u32) -> Option<&Safepoint> {
        self.lookup(pc_offset)
    }
    #[must_use]
    /// Find the map selected by a code-relative PC offset.
    pub fn lookup(&self, pc_offset: u32) -> Option<&Safepoint> {
        self.entries[..]
            .binary_search_by_key(&pc_offset, |entry| entry.pc_offset)
            .map_or_else(
                |index| index.checked_sub(1).and_then(|i| self.entries.get(i)),
                |index| self.entries.get(index),
            )
    }
    #[must_use]
    /// Return all entries in increasing PC order.
    pub fn entries(&self) -> &[Safepoint] {
        &self.entries
    }
    #[must_use]
    /// Test whether a frame slot is a live Lisp word for this map.
    pub fn is_slot_live(map: &Safepoint, slot: usize) -> bool {
        slot < usize::from(map.word_slot_count)
            && map
                .slot_bitmap
                .get(slot / 8)
                .is_some_and(|bits| bits & (1 << (slot % 8)) != 0)
    }
}

/// Forward the function object and live Word slots in one mapped frame.
pub fn scan_frame(
    words: &mut [Word],
    frame_start: usize,
    map: &Safepoint,
    mut forward: impl FnMut(Word) -> Word,
) -> Option<usize> {
    let frame_end = frame_start.checked_add(usize::from(map.frame_words))?;
    if frame_end > words.len() || !SafepointMap::is_slot_live(map, 2) {
        return None;
    }
    let mut updated = 0;
    for slot in 0..usize::from(map.slot_words).min(frame_end - frame_start) {
        if SafepointMap::is_slot_live(map, slot) {
            words[frame_start + slot] = forward(words[frame_start + slot]);
            updated += 1;
        }
    }
    Some(updated)
}

/// Scan one frame and its mapped callee-saved register slots.
pub fn scan_frame_with_registers(
    words: &mut [Word],
    frame_start: usize,
    map: &Safepoint,
    registers: &mut [Word],
    mut forward: impl FnMut(Word) -> Word,
) -> Option<usize> {
    if map.register_ids.iter().any(|register_id| {
        u32::from(*register_id) >= u16::BITS || map.register_mask & (1_u16 << register_id) == 0
    }) {
        return None;
    }
    let mut updated = scan_frame(words, frame_start, map, &mut forward)?;
    for register_id in &map.register_ids {
        let index = usize::from(*register_id);
        if let Some(register) = registers.get_mut(index) {
            *register = forward(*register);
            updated += 1;
        }
    }
    Some(updated)
}

/// Scan a chain of mapped four-word frames and forward precise roots in place.
pub fn scan_frame_chain(
    words: &mut [Word],
    first: usize,
    code_base: usize,
    maps: &SafepointMap,
    mut forward: impl FnMut(Word) -> Word,
) -> Option<usize> {
    let mut at = first;
    let mut updated = 0;
    let mut frames = 0;
    while at.checked_add(3).is_some_and(|end| end < words.len()) {
        let return_pc = usize::try_from(words[at + 1].bits()).ok()?;
        let offset = return_pc.checked_sub(code_base)?;
        let map = maps.find_map(u32::try_from(offset).ok()?)?;
        updated += scan_frame(words, at, map, &mut forward)?;
        frames += 1;
        let previous = words[at].address();
        if previous == 0 || previous == at {
            break;
        }
        at = previous;
    }
    (frames > 0).then_some(updated)
}

/// Scan a frame chain by resolving each return PC through the code registry.
pub fn scan_frame_chain_with_registry(
    words: &mut [Word],
    first: usize,
    registry: &CodeRegistry,
    registers: &mut [Word],
    mut forward: impl FnMut(Word) -> Word,
) -> Option<usize> {
    let mut at = first;
    let mut updated = 0;
    let mut frames = 0;
    while at.checked_add(3).is_some_and(|end| end < words.len()) {
        let return_pc = usize::try_from(words[at + 1].bits()).ok()?;
        let (metadata, offset) = registry.find(return_pc)?;
        let map = metadata.safepoint_map.find_map(offset)?;
        updated += scan_frame_with_registers(words, at, map, registers, &mut forward)?;
        frames += 1;
        let previous = words[at].address();
        if previous == 0 || previous == at {
            break;
        }
        at = previous;
    }
    (frames > 0).then_some(updated)
}
