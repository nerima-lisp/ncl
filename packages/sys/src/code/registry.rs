use super::{CodeError, CodeObjectMetadata, CodePtr};
use crate::heap::metadata::{MetadataBudget, MetadataCharge, MetadataVec};

/// PC range index for published code objects.
#[derive(Debug)]
pub struct CodeRegistry {
    objects: MetadataVec<RegistryEntry>,
    budget: MetadataBudget,
}

#[derive(Debug)]
struct RegistryEntry {
    base: usize,
    end: usize,
    metadata: CodeObjectMetadata,
    _charge: MetadataCharge,
}

impl CodeRegistry {
    pub(crate) fn with_budget(budget: MetadataBudget) -> Self {
        Self {
            objects: MetadataVec::new(budget.clone()),
            budget,
        }
    }

    /// Register metadata for a published code allocation.
    ///
    /// # Errors
    ///
    /// Returns an error when the code allocation is unpublished, its address range overflows, or
    /// the metadata capacity cannot be satisfied.
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
        let base = code.address();
        let index = self
            .objects
            .as_slice()
            .binary_search_by_key(&base, |entry| entry.base);
        let nested_bytes = metadata
            .requested_capacity_bytes()
            .map_err(|_| CodeError::MetadataCapacityExceeded)?;
        let charge = self
            .budget
            .reserve(nested_bytes)
            .map_err(|_| CodeError::MetadataCapacityExceeded)?;
        let entry = RegistryEntry {
            base,
            end,
            metadata,
            _charge: charge,
        };
        match index {
            Ok(index) => {
                let Some(existing) = self.objects.get_mut(index) else {
                    return Err(CodeError::RegistryInvariantViolation);
                };
                *existing = entry;
            }
            Err(index) => self
                .objects
                .insert(index, entry)
                .map_err(|_| CodeError::MetadataCapacityExceeded)?,
        }
        Ok(())
    }
    /// Remove metadata before releasing a code allocation.
    pub fn unregister(&mut self, code: &CodePtr) -> Option<CodeObjectMetadata> {
        let index = self
            .objects
            .as_slice()
            .binary_search_by_key(&code.address(), |entry| entry.base)
            .ok()?;
        Some(self.objects.remove(index)?.metadata)
    }
    /// Resolve a PC to its containing code object and relative offset.
    #[must_use]
    pub fn find(&self, pc: usize) -> Option<(&CodeObjectMetadata, u32)> {
        let index = match self
            .objects
            .as_slice()
            .binary_search_by_key(&pc, |entry| entry.base)
        {
            Ok(index) => index,
            Err(insertion_index) => insertion_index.checked_sub(1)?,
        };
        let entry = self.objects.get(index)?;
        if pc >= entry.end {
            return None;
        }
        Some((&entry.metadata, u32::try_from(pc - entry.base).ok()?))
    }
}

impl Default for CodeRegistry {
    fn default() -> Self {
        Self::with_budget(MetadataBudget::default())
    }
}
