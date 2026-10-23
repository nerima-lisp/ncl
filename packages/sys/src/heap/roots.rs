use super::Heap;
use crate::heap_state::State;
use crate::{RootToken, Word};
use core::cell::Cell;

/// Failure while registering or removing a heap root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeapRootError {
    /// The token does not identify an active root in this heap.
    InvalidToken,
    /// The metadata limit cannot reserve root registry capacity.
    MetadataCapacityExceeded,
}

fn trim_root_tombstones(state: &mut State) {
    let State {
        roots,
        root_generations,
        ..
    } = state;
    while roots.last().is_some_and(|root| root.is_null()) {
        let _ = roots.pop();
        let _ = root_generations.pop();
    }
}

fn root_token_or_abort(result: Result<RootToken, HeapRootError>) -> RootToken {
    // These signatures cannot report failure; returning a token would leave the value unregistered.
    let Ok(token) = result else {
        std::process::abort()
    };
    token
}

impl Heap {
    /// Register a caller-owned precise root slot with the heap.
    ///
    /// The referenced slot must outlive the returned token and remain at the
    /// same address until [`Heap::pop_root`] or [`Heap::unregister_root`] is
    /// called. The process aborts if root metadata cannot be reserved.
    #[must_use]
    #[track_caller]
    pub fn push_root(&self, value: &mut Word) -> RootToken {
        root_token_or_abort(self.try_push_root(value))
    }

    /// Register a caller-owned precise root slot with the heap.
    ///
    /// The referenced slot must outlive the returned token and remain at the
    /// same address until [`Heap::pop_root`] or [`Heap::unregister_root`] is
    /// called.
    ///
    /// # Errors
    /// Returns [`HeapRootError::MetadataCapacityExceeded`] when root registry storage cannot grow.
    pub fn try_push_root(&self, value: &mut Word) -> Result<RootToken, HeapRootError> {
        self.register_root(std::ptr::from_mut(value))
    }

    /// Register a heap root backed by interior-mutable storage.
    ///
    /// The cell must outlive the returned token and remain at the same address
    /// until [`Heap::pop_root`] or [`Heap::unregister_root`] is called. The
    /// process aborts if root metadata cannot be reserved.
    #[must_use]
    #[track_caller]
    pub fn push_root_cell(&self, value: &Cell<Word>) -> RootToken {
        root_token_or_abort(self.try_push_root_cell(value))
    }

    /// Register a heap root backed by interior-mutable storage.
    ///
    /// The cell must outlive the returned token and remain at the same address
    /// until [`Heap::pop_root`] or [`Heap::unregister_root`] is called.
    ///
    /// # Errors
    /// Returns [`HeapRootError::MetadataCapacityExceeded`] when root registry storage cannot grow.
    pub fn try_push_root_cell(&self, value: &Cell<Word>) -> Result<RootToken, HeapRootError> {
        self.register_root(value.as_ptr())
    }

    fn register_root(&self, value: *mut Word) -> Result<RootToken, HeapRootError> {
        let mut state = self.lock_state();
        state
            .roots
            .reserve(1)
            .map_err(|_| HeapRootError::MetadataCapacityExceeded)?;
        state
            .root_generations
            .reserve(1)
            .map_err(|_| HeapRootError::MetadataCapacityExceeded)?;
        let generation = state.next_root_generation;
        let token = RootToken {
            index: state.roots.len(),
            count: 1,
            generation,
            heap_id: self.root_namespace,
        };
        state
            .roots
            .push(value)
            .map_err(|_| HeapRootError::MetadataCapacityExceeded)?;
        if state.root_generations.push(generation).is_err() {
            let _ = state.roots.pop();
            return Err(HeapRootError::MetadataCapacityExceeded);
        }
        state.next_root_generation = generation.wrapping_add(1);
        if state.next_root_generation == 0 {
            state.next_root_generation = 1;
        }
        drop(state);
        Ok(token)
    }

    /// Remove the most recently registered heap-owned root.
    pub fn pop_root(&self, token: RootToken) -> bool {
        let mut state = self.lock_state();
        let Some(end) = token.index.checked_add(token.count) else {
            return false;
        };
        if end != state.roots.len() {
            return false;
        }
        if token.count != 1 || token.generation == 0 || token.heap_id != self.root_namespace {
            return false;
        }
        let Some(root_span) = state.roots.get(token.index..end) else {
            return false;
        };
        if root_span.iter().any(|root| root.is_null())
            || state.root_generations.get(token.index) != Some(&token.generation)
        {
            return false;
        }
        state.roots.truncate(token.index);
        state.root_generations.truncate(token.index);
        trim_root_tombstones(&mut state);
        drop(state);
        true
    }

    /// Remove a heap root by its token, regardless of insertion order.
    ///
    /// This is for owners that cannot wait for newer roots to be removed.
    ///
    /// # Errors
    /// Returns [`HeapRootError::InvalidToken`] when the token is not active in
    /// this heap.
    pub fn unregister_root(&self, token: RootToken) -> Result<(), HeapRootError> {
        if token.count != 1 || token.generation == 0 || token.heap_id != self.root_namespace {
            return Err(HeapRootError::InvalidToken);
        }
        let mut state = self.lock_state();
        {
            let State {
                roots,
                root_generations,
                ..
            } = &mut *state;
            let Some(generation) = root_generations.get_mut(token.index) else {
                return Err(HeapRootError::InvalidToken);
            };
            if *generation != token.generation {
                return Err(HeapRootError::InvalidToken);
            }
            let Some(root) = roots.get_mut(token.index) else {
                return Err(HeapRootError::InvalidToken);
            };
            if root.is_null() {
                return Err(HeapRootError::InvalidToken);
            }
            *root = std::ptr::null_mut();
            *generation = 0;
        }
        trim_root_tombstones(&mut state);
        drop(state);
        Ok(())
    }
}
