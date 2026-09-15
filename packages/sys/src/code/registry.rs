use super::{CodeError, CodeObjectMetadata, CodePtr};
use std::collections::BTreeMap;

/// PC range index for published code objects.
#[derive(Clone, Debug, Default)]
pub struct CodeRegistry {
    pub(super) objects: BTreeMap<usize, (usize, CodeObjectMetadata)>,
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
