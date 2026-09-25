use crate::{Word, os::declarations};
use core::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
#[path = "code/metadata.rs"]
mod metadata;
pub use metadata::{CodeObjectMetadata, SourceLocation};
#[path = "code/registry.rs"]
mod registry;
pub use registry::CodeRegistry;

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
    /// The requested mapping has zero bytes.
    EmptyAllocation,
    /// A range or address arithmetic operation exceeded the allocation.
    OutOfBounds,
    /// Publication was requested after executable publication.
    AlreadyPublished,
    /// A registry operation requires published code.
    NotPublished,
    /// The operating system rejected the mapping request.
    MappingFailed,
    /// The operating system rejected executable page permissions.
    ProtectionFailed,
    /// A live registered frame still returns into the code allocation.
    CodeInUse,
    /// The code allocation is not present in the heap registry.
    NotRegistered,
}

/// A page-backed, non-moving code allocation.
#[derive(Debug)]
pub struct CodePtr {
    ptr: NonNull<u8>,
    len: usize,
    mapping_len: usize,
    published: AtomicBool,
    entry: AtomicUsize,
}

impl CodePtr {
    /// Return the initialized code bytes.
    #[must_use]
    pub const fn as_slice(&self) -> &[u8] {
        // SAFETY: ptr is a live mmap allocation owned by self.
        unsafe { core::slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }
    /// Copy bytes into the allocation at a checked offset.
    ///
    /// # Errors
    ///
    /// Returns an error for a published allocation or an out-of-bounds range.
    pub fn write_code(&mut self, offset: usize, bytes: &[u8]) -> Result<(), CodeError> {
        if self.is_published() {
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
        // SAFETY: the allocation is writable until publication and the checked range is in-bounds.
        unsafe {
            core::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len)[offset..end]
                .copy_from_slice(bytes);
        }
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
    pub fn is_published(&self) -> bool {
        self.published.load(Ordering::Acquire)
    }
    /// Load the published entry address with acquire ordering.
    #[must_use]
    pub fn entry(&self) -> usize {
        self.entry.load(Ordering::Acquire)
    }
    fn publish(&mut self) -> Result<(), CodeError> {
        if self.is_published() {
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
        self.entry.store(self.address(), Ordering::Release);
        self.published.store(true, Ordering::Release);
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
        published: AtomicBool::new(false),
        entry: AtomicUsize::new(0),
    })
}

/// Release code storage that was never registered with [`crate::Heap`].
///
/// Registered code must be released through [`crate::Heap::release_code`], which
/// performs the stop-the-world quiescence check before dropping the mapping.
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
#[cfg(test)]
mod tests;
