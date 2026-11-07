use std::marker::PhantomData;

use crate::ids::{ArenaIndex, IdError};

/// A typed, append-only arena tied to one form or compilation-unit identity.
#[derive(Debug)]
pub struct Arena<T, I: ArenaIndex> {
    owner: I::Owner,
    values: Vec<T>,
    index_type: PhantomData<fn() -> I>,
}

impl<T, I: ArenaIndex> Arena<T, I> {
    /// Creates an empty arena for the specified owner.
    #[must_use]
    pub fn new(owner: I::Owner) -> Self {
        Self {
            owner,
            values: Vec::new(),
            index_type: PhantomData,
        }
    }

    /// Returns the identity that owns this arena.
    #[must_use]
    pub const fn owner(&self) -> I::Owner {
        self.owner
    }

    /// Appends a value and returns its typed index.
    ///
    /// # Errors
    ///
    /// Returns `IdError::IndexOverflow` if the next index does not fit in `u32`.
    pub fn allocate(&mut self, value: T) -> Result<I, IdError> {
        let id = I::try_from_index(self.owner, self.values.len())?;
        self.values.push(value);
        Ok(id)
    }

    /// Looks up a value when `id` belongs to this arena and is in bounds.
    #[must_use]
    pub fn get(&self, id: I) -> Option<&T> {
        if id.owner() != self.owner {
            return None;
        }

        let index = usize::try_from(id.index()).ok()?;
        self.values.get(index)
    }

    /// Looks up a mutable value when `id` belongs to this arena and is in bounds.
    #[must_use]
    pub fn get_mut(&mut self, id: I) -> Option<&mut T> {
        if id.owner() != self.owner {
            return None;
        }

        let index = usize::try_from(id.index()).ok()?;
        self.values.get_mut(index)
    }

    /// Returns the number of allocated entries for checked range construction.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns whether this arena has allocated any identities.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Iterates over entries with their zero-based arena indices.
    #[must_use]
    pub fn iter(&self) -> impl Iterator<Item = (usize, &T)> {
        self.values.iter().enumerate()
    }

    /// Consumes the arena and returns its values in allocation order.
    #[must_use]
    pub fn into_values(self) -> Vec<T> {
        self.values
    }
}
