use crate::Word;

/// A non-moving executable allocation.
#[derive(Debug, Eq, PartialEq)]
pub struct CodePtr {
    bytes: Box<[u8]>,
    published: bool,
}
impl CodePtr {
    /// Return the writable code bytes.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }
    /// Return the writable code bytes for relocation.
    pub fn as_mut_slice(&mut self) -> Option<&mut [u8]> {
        (!self.published).then_some(&mut self.bytes)
    }
    /// Return the address of the code allocation.
    #[must_use]
    pub fn address(&self) -> usize {
        self.bytes.as_ptr() as usize
    }
    /// Whether this allocation has been published.
    #[must_use]
    pub const fn is_published(&self) -> bool {
        self.published
    }
}
/// Allocate writable, non-moving code storage.
#[must_use]
pub fn alloc_code(bytes: usize) -> Option<CodePtr> {
    (bytes > 0).then(|| CodePtr {
        bytes: vec![0; bytes].into_boxed_slice(),
        published: false,
    })
}
/// Release code storage.
pub fn free_code(_code: CodePtr) {}
/// Publish code after relocations have been installed.
pub const fn publish_code(code: &mut CodePtr) {
    code.published = true;
}

/// One decoded safepoint entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Safepoint {
    pub pc_offset: u32,
    pub frame_words: u16,
    pub slot_words: u16,
    pub word_slot_count: u16,
    pub register_mask: u16,
    pub map_flags: u32,
    pub slot_bitmap: Vec<u8>,
    pub register_ids: Vec<u16>,
}
/// A sorted PC index for precise native frames.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SafepointMap {
    entries: Vec<Safepoint>,
}

/// The fixed four-word native frame header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameHeader {
    pub previous: usize,
    pub return_pc: usize,
    pub function: Word,
    pub flags: u64,
}

/// Read a validated chain of four-word frame headers from a word slice.
#[must_use]
pub fn walk_frame_headers(words: &[Word], first: usize, limit: usize) -> Vec<FrameHeader> {
    let mut result = Vec::new();
    let mut at = first;
    while result.len() < limit && at.checked_add(3).is_some_and(|end| end < words.len()) {
        let header = FrameHeader {
            previous: words[at].address(),
            return_pc: words[at + 1].address(),
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
    /// Decode entries from the specified wire representation.
    ///
    /// # Errors
    ///
    /// Returns an error when a header, bitmap, or register list is truncated.
    pub fn decode(bytes: &[u8], count: usize) -> Result<Self, &'static str> {
        let mut at = 0;
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            if bytes.len().saturating_sub(at) < 16 {
                return Err("truncated safepoint header");
            }
            let u32at = |i| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap_or([0; 4]));
            let u16at = |i| u16::from_le_bytes(bytes[i..i + 2].try_into().unwrap_or([0; 2]));
            let pc_offset = u32at(at);
            let frame_words = u16at(at + 4);
            let slot_words = u16at(at + 6);
            let word_slot_count = u16at(at + 8);
            let register_mask = u16at(at + 10);
            let map_flags = u32at(at + 12);
            at += 16;
            let bitmap_bytes = usize::from(slot_words).div_ceil(8);
            if bytes.len().saturating_sub(at) < bitmap_bytes {
                return Err("truncated slot bitmap");
            }
            let bitmap = bytes[at..at + bitmap_bytes].to_vec();
            at += bitmap_bytes;
            let register_count =
                usize::from(u16::try_from(register_mask.count_ones()).unwrap_or(0));
            if bytes.len().saturating_sub(at) < register_count * 2 {
                return Err("truncated register ids");
            }
            let mut ids = Vec::with_capacity(register_count);
            for _ in 0..register_count {
                ids.push(u16at(at));
                at += 2;
            }
            entries.push(Safepoint {
                pc_offset,
                frame_words,
                slot_words,
                word_slot_count,
                register_mask,
                map_flags,
                slot_bitmap: bitmap,
                register_ids: ids,
            });
        }
        entries.sort_by_key(|entry| entry.pc_offset);
        Ok(Self { entries })
    }
    /// Find the nearest map at or before a return-PC offset.
    #[must_use]
    pub fn lookup(&self, pc_offset: u32) -> Option<&Safepoint> {
        let mut low = 0;
        let mut high = self.entries.len();
        while low < high {
            let middle = low + (high - low) / 2;
            if self.entries[middle].pc_offset <= pc_offset {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        low.checked_sub(1).and_then(|index| self.entries.get(index))
    }
    /// Return all decoded entries.
    #[must_use]
    pub fn entries(&self) -> &[Safepoint] {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn code_lifecycle() {
        let Some(mut c) = alloc_code(4) else { return };
        assert!(c.as_mut_slice().is_some());
        publish_code(&mut c);
        assert!(c.as_mut_slice().is_none());
    }
    #[test]
    fn map_lookup() {
        let mut b = vec![0; 16];
        b[0..4].copy_from_slice(&7u32.to_le_bytes());
        b[4..6].copy_from_slice(&8u16.to_le_bytes());
        b[6..8].copy_from_slice(&4u16.to_le_bytes());
        b.push(0);
        let result = SafepointMap::decode(&b, 1);
        assert!(result.is_ok());
        let Some(map) = result.ok() else {
            return;
        };
        assert_eq!(map.lookup(7).map(|x| x.pc_offset), Some(7));
    }
}
