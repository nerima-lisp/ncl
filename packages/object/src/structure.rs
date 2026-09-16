use crate::object_access::{fix, get, put};
use crate::{ObjectError, Runtime, ThreadContext, allocate, structure_offset, widetag, with_roots};
use ncl_sys::Word;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructureLayout(u32);
impl From<u32> for StructureLayout {
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl From<StructureLayout> for u32 {
    fn from(value: StructureLayout) -> Self {
        value.0
    }
}
impl StructureLayout {
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

impl Runtime {
    /// Allocate a monotonically increasing structure layout identifier.
    ///
    /// # Errors
    /// Returns an error when the identifier space is exhausted.
    pub fn register_structure_layout(
        &self,
        slot_count: usize,
    ) -> Result<StructureLayout, ObjectError> {
        let mut next = self
            .next_layout
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let id = *next;
        *next = next.checked_add(1).ok_or(ObjectError::Layout)?;
        drop(next);
        self.layouts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(id, slot_count);
        Ok(StructureLayout(id))
    }

    /// Return the slot count for a registered structure layout.
    #[must_use]
    pub fn structure_layout_size(&self, id: StructureLayout) -> Option<usize> {
        self.layouts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&id.0)
            .copied()
    }
}

/// Allocate a structure object.
///
/// # Errors
/// Returns an error when the layout is unknown or allocation fails.
pub fn make_structure(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    layout: StructureLayout,
    slots: &[Word],
) -> Result<Word, ObjectError> {
    if runtime.structure_layout_size(layout) != Some(slots.len()) {
        return Err(ObjectError::Layout);
    }
    with_roots(ctx, slots, |ctx, slots| {
        let object = allocate(ctx, runtime, widetag::STRUCTURE, 1 + slots.len())?;
        put(
            ctx,
            object,
            structure_offset::LAYOUT,
            fix(layout.0 as usize)?,
        )?;
        for (index, value) in slots.iter().copied().enumerate() {
            put(ctx, object, structure_offset::SLOTS + index, value)?;
        }
        Ok(object)
    })
}

/// Read a structure layout identifier.
///
/// # Errors
/// Returns an error when the object is not a valid structure.
pub fn structure_layout(ctx: &ThreadContext, object: Word) -> Result<StructureLayout, ObjectError> {
    let value = get(ctx, object, widetag::STRUCTURE, structure_offset::LAYOUT)?
        .as_fixnum()
        .ok_or(ObjectError::Layout)?;
    Ok(StructureLayout(
        u32::try_from(value).map_err(|_| ObjectError::Layout)?,
    ))
}

/// Read a structure slot.
///
/// # Errors
/// Returns an error when the object or slot is invalid.
pub fn structure_ref(ctx: &ThreadContext, object: Word, index: usize) -> Result<Word, ObjectError> {
    get(
        ctx,
        object,
        widetag::STRUCTURE,
        structure_offset::SLOTS + index,
    )
}

/// Write a structure slot.
///
/// # Errors
/// Returns an error when the object or slot is invalid.
pub fn structure_set(
    ctx: &mut ThreadContext,
    object: Word,
    index: usize,
    value: Word,
) -> Result<(), ObjectError> {
    if ncl_sys::object_widetag(&ctx.thread, object) != Some(widetag::STRUCTURE) {
        return Err(ObjectError::TypeError);
    }
    put(ctx, object, structure_offset::SLOTS + index, value)?;
    ncl_sys::write_barrier(&mut ctx.thread, object, structure_offset::SLOTS + index);
    Ok(())
}
