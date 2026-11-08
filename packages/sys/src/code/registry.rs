use super::{CodeError, CodeObjectMetadata, CodePtr, REGISTRY_ACTIVE, REGISTRY_UNREGISTERED};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

/// PC range index for prepared and active code objects.
#[derive(Clone, Debug, Default)]
pub struct CodeRegistry {
    pub(super) objects: BTreeMap<usize, (usize, CodeObjectMetadata, Arc<AtomicU8>)>,
}

impl CodeRegistry {
    /// Register code metadata for PC lookup.
    ///
    /// Metadata remains prepared while code is unpublished. If code is already
    /// published, the entry becomes active during this call. Publish-first
    /// callers must keep this entry private until the call returns.
    ///
    /// # Errors
    ///
    /// Returns an error when metadata does not fit the allocation, the range is already
    /// registered, or the registry state cannot be activated.
    pub fn register(
        &mut self,
        code: &CodePtr,
        metadata: CodeObjectMetadata,
    ) -> Result<(), CodeError> {
        if metadata.size == 0
            || metadata.size > code.len()
            || metadata.entry_offset >= metadata.size
        {
            return Err(CodeError::OutOfBounds);
        }
        let end = code
            .address()
            .checked_add(metadata.size)
            .ok_or(CodeError::OutOfBounds)?;
        if self.objects.contains_key(&code.address()) {
            return Err(CodeError::AlreadyRegistered);
        }
        let state = code.prepare_registry()?;
        if code.is_published() {
            code.activate_registry()?;
        }
        self.objects.insert(code.address(), (end, metadata, state));
        Ok(())
    }
    /// Remove metadata before releasing a code allocation.
    pub fn unregister(&mut self, code: &CodePtr) -> Option<CodeObjectMetadata> {
        self.objects
            .remove(&code.address())
            .map(|(_, metadata, active)| {
                active.store(REGISTRY_UNREGISTERED, Ordering::Release);
                metadata
            })
    }
    /// Resolve a PC to its containing code object and relative offset.
    #[must_use]
    pub fn find(&self, pc: usize) -> Option<(&CodeObjectMetadata, u32)> {
        let (base, (end, metadata, active)) = self.objects.range(..=pc).next_back()?;
        if pc >= *end || active.load(Ordering::Acquire) != REGISTRY_ACTIVE {
            return None;
        }
        Some((metadata, u32::try_from(pc - base).ok()?))
    }
}
