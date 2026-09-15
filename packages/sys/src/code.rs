use crate::{Word, os::declarations};
use core::ptr::NonNull;
use std::collections::BTreeMap;

const PAGE_SIZE: usize = 4096;
const PROT_READ: i32 = 1;
const PROT_WRITE: i32 = 2;
const PROT_EXEC: i32 = 4;
const MAP_PRIVATE: i32 = 2;
#[cfg(target_os = "macos")]
const MAP_ANON: i32 = 0x1000;
#[cfg(target_os = "macos")]
const MAP_JIT: i32 = 0x800;
#[cfg(target_os = "linux")]
const MAP_ANON: i32 = 0x20;

/// Errors from executable code-space operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodeError {
    EmptyAllocation,
    OutOfBounds,
    AlreadyPublished,
    NotPublished,
    MappingFailed,
    ProtectionFailed,
}

/// A page-backed, non-moving code allocation.
#[derive(Debug, Eq, PartialEq)]
pub struct CodePtr {
    ptr: NonNull<u8>,
    len: usize,
    mapping_len: usize,
    published: bool,
}

impl CodePtr {
    /// Return the initialized code bytes.
    #[must_use]
    pub const fn as_slice(&self) -> &[u8] {
        // SAFETY: ptr is a live mmap allocation owned by self.
        unsafe { core::slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }
    /// Return writable code bytes for relocation.
    pub fn as_mut_slice(&mut self) -> Option<&mut [u8]> {
        (!self.published).then(|| {
            // SAFETY: this allocation is writable until publication.
            unsafe { core::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
        })
    }
    /// Copy bytes into the allocation at a checked offset.
    ///
    /// # Errors
    ///
    /// Returns an error for a published allocation or an out-of-bounds range.
    pub fn write_code(&mut self, offset: usize, bytes: &[u8]) -> Result<(), CodeError> {
        if self.published {
            return Err(CodeError::AlreadyPublished);
        }
        let end = offset
            .checked_add(bytes.len())
            .ok_or(CodeError::OutOfBounds)?;
        if end > self.len {
            return Err(CodeError::OutOfBounds);
        }
        #[cfg(target_os = "macos")]
        // SAFETY: this thread owns the JIT mapping and brackets its write.
        unsafe {
            declarations::pthread_jit_write_protect_np(0);
        };
        self.as_mut_slice().ok_or(CodeError::AlreadyPublished)?[offset..end].copy_from_slice(bytes);
        #[cfg(target_os = "macos")]
        // SAFETY: restore the JIT write-protection state after the copy.
        unsafe {
            declarations::pthread_jit_write_protect_np(1);
        };
        Ok(())
    }
    /// Return the address of the code allocation.
    #[must_use]
    pub fn address(&self) -> usize {
        self.ptr.as_ptr() as usize
    }
    /// Return the requested byte length.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }
    /// Whether the allocation has no requested bytes.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
    /// Whether this allocation has been published.
    #[must_use]
    pub const fn is_published(&self) -> bool {
        self.published
    }
    fn publish(&mut self) -> Result<(), CodeError> {
        if self.published {
            return Err(CodeError::AlreadyPublished);
        }
        #[cfg(not(target_os = "macos"))]
        {
            // SAFETY: the mapping is page-aligned and mapping_len is its exact length.
            let result = unsafe {
                declarations::mprotect(
                    self.ptr.as_ptr().cast(),
                    self.mapping_len,
                    PROT_READ | PROT_EXEC,
                )
            };
            if result != 0 {
                return Err(CodeError::ProtectionFailed);
            }
        }
        #[cfg(target_os = "macos")]
        // SAFETY: the range is a live instruction mapping owned by self.
        unsafe {
            declarations::pthread_jit_write_protect_np(1);
            declarations::sys_icache_invalidate(self.ptr.as_ptr().cast(), self.len);
        }
        self.published = true;
        Ok(())
    }
}

impl Drop for CodePtr {
    fn drop(&mut self) {
        // SAFETY: ptr and mapping_len came from the successful mmap owned by self.
        let _ = unsafe { declarations::munmap(self.ptr.as_ptr().cast(), self.mapping_len) };
    }
}

/// Allocate writable, non-moving code storage.
///
/// # Errors
///
/// Returns an error for an empty allocation or when the OS cannot map pages.
pub fn alloc_code(bytes: usize) -> Result<CodePtr, CodeError> {
    if bytes == 0 {
        return Err(CodeError::EmptyAllocation);
    }
    let mapping_len = bytes
        .checked_next_multiple_of(PAGE_SIZE)
        .ok_or(CodeError::MappingFailed)?;
    #[cfg(target_os = "macos")]
    let flags = MAP_PRIVATE | MAP_ANON | MAP_JIT;
    #[cfg(not(target_os = "macos"))]
    let flags = MAP_PRIVATE | MAP_ANON;
    #[cfg(target_os = "macos")]
    let initial_prot = PROT_READ | PROT_WRITE | PROT_EXEC;
    #[cfg(not(target_os = "macos"))]
    let initial_prot = PROT_READ | PROT_WRITE;
    // SAFETY: null requests an OS-selected page-aligned anonymous mapping.
    let raw = unsafe {
        declarations::mmap(
            core::ptr::null_mut(),
            mapping_len,
            initial_prot,
            flags,
            -1,
            0,
        )
    };
    let ptr = NonNull::new(raw.cast()).ok_or(CodeError::MappingFailed)?;
    Ok(CodePtr {
        ptr,
        len: bytes,
        mapping_len,
        published: false,
    })
}

/// Release code storage.
pub fn free_code(code: CodePtr) {
    drop(code);
}

/// Publish code after relocations and metadata have been installed.
///
/// # Errors
///
/// Returns an error when the allocation was already published or its permissions cannot be changed.
pub fn publish_code(code: &mut CodePtr) -> Result<(), CodeError> {
    code.publish()
}

/// Copy bytes into a writable code allocation.
///
/// # Errors
///
/// Returns an error for a published allocation or an out-of-bounds range.
pub fn write_code(code: &mut CodePtr, offset: usize, bytes: &[u8]) -> Result<(), CodeError> {
    code.write_code(offset, bytes)
}

/// Metadata registered for one immutable code range.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeObjectMetadata {
    pub entry_offset: usize,
    pub size: usize,
    pub constant_slots: Vec<Word>,
    pub safepoint_map: SafepointMap,
    pub debug_table: Vec<u8>,
}

/// PC range index for published code objects.
#[derive(Clone, Debug, Default)]
pub struct CodeRegistry {
    objects: BTreeMap<usize, (usize, CodeObjectMetadata)>,
}
impl CodeRegistry {
    /// Register metadata for a published code allocation.
    ///
    /// # Errors
    ///
    /// Returns an error when the code allocation has not been published.
    pub fn register(
        &mut self,
        code: &CodePtr,
        metadata: CodeObjectMetadata,
    ) -> Result<(), CodeError> {
        if !code.is_published() {
            return Err(CodeError::NotPublished);
        }
        let end = code
            .address()
            .checked_add(code.len())
            .ok_or(CodeError::OutOfBounds)?;
        self.objects.insert(code.address(), (end, metadata));
        Ok(())
    }
    /// Remove metadata before releasing a code allocation.
    pub fn unregister(&mut self, code: &CodePtr) -> Option<CodeObjectMetadata> {
        self.objects
            .remove(&code.address())
            .map(|(_, metadata)| metadata)
    }
    /// Resolve a PC to its containing code object and relative offset.
    #[must_use]
    pub fn find(&self, pc: usize) -> Option<(&CodeObjectMetadata, u32)> {
        let (base, (end, metadata)) = self.objects.range(..=pc).next_back()?;
        if pc >= *end {
            return None;
        }
        Some((metadata, u32::try_from(pc - base).ok()?))
    }
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

/// Read a bounded chain of four-word frame headers from a word slice.
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
    pub fn lookup(&self, pc_offset: u32) -> Option<&Safepoint> {
        self.entries[..]
            .binary_search_by_key(&pc_offset, |entry| entry.pc_offset)
            .map_or_else(
                |index| index.checked_sub(1).and_then(|i| self.entries.get(i)),
                |index| self.entries.get(index),
            )
    }
    #[must_use]
    pub fn entries(&self) -> &[Safepoint] {
        &self.entries
    }
    #[must_use]
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
        let return_pc = words[at + 1].address();
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

#[cfg(test)]
mod tests;
