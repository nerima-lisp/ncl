use std::cmp::Ordering;

/// A non-moving executable allocation.
#[derive(Debug, Eq, PartialEq)]
pub struct CodePtr {
    bytes: Box<[u8]>,
    published: bool,
}
impl CodePtr {
    /// Return the writable code bytes.
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }
    /// Return the writable code bytes for relocation.
    pub fn as_mut_slice(&mut self) -> Option<&mut [u8]> {
        (!self.published).then_some(&mut self.bytes)
    }
    /// Return the address of the code allocation.
    pub fn address(&self) -> usize {
        self.bytes.as_ptr() as usize
    }
    /// Whether this allocation has been published.
    pub const fn is_published(&self) -> bool {
        self.published
    }
}
/// Allocate writable, non-moving code storage.
pub fn alloc_code(bytes: usize) -> Option<CodePtr> {
    (bytes > 0).then(|| CodePtr {
        bytes: vec![0; bytes].into_boxed_slice(),
        published: false,
    })
}
/// Release code storage.
pub fn free_code(_code: CodePtr) {}
/// Publish code after relocations have been installed.
pub fn publish_code(code: &mut CodePtr) {
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
impl SafepointMap {
    /// Decode entries from the specified wire representation.
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
            let register_count = usize::from(register_mask.count_ones() as u16);
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
    pub fn lookup(&self, pc_offset: u32) -> Option<&Safepoint> {
        self.entries
            .binary_search_by(|entry| {
                if entry.pc_offset <= pc_offset {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            })
            .ok()
            .and_then(|i| self.entries.get(i))
            .or_else(|| {
                self.entries
                    .iter()
                    .rev()
                    .find(|entry| entry.pc_offset <= pc_offset)
            })
    }
    /// Return all decoded entries.
    pub fn entries(&self) -> &[Safepoint] {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn code_lifecycle() {
        let mut c = alloc_code(4).expect("allocation");
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
