use super::Heap;
use crate::heap_state::State;
use crate::{RootToken, Word};
use core::cell::Cell;

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

impl Heap {
    /// Register a heap-owned precise root slot.
    ///
    /// The referenced slot must outlive the returned token and remain at the
    /// same address until [`Heap::pop_root`] or [`Heap::unregister_root`] is
    /// called.
    pub fn push_root(&self, value: &mut Word) -> RootToken {
        self.register_root(std::ptr::from_mut(value))
    }

    /// Register a heap root backed by interior-mutable storage.
    ///
    /// The cell must outlive the returned token and remain at the same address
    /// until [`Heap::pop_root`] or [`Heap::unregister_root`] is called.
    pub fn push_root_cell(&self, value: &Cell<Word>) -> RootToken {
        self.register_root(value.as_ptr())
    }

    fn register_root(&self, value: *mut Word) -> RootToken {
        let mut state = self.lock_state();
        let generation = state.next_root_generation;
        state.next_root_generation = generation.wrapping_add(1);
        if state.next_root_generation == 0 {
            state.next_root_generation = 1;
        }
        let token = RootToken {
            index: state.roots.len(),
            count: 1,
            generation,
            heap_id: self.root_namespace,
        };
        state.roots.push(value);
        state.root_generations.push(generation);
        token
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
    pub fn unregister_root(&self, token: RootToken) -> bool {
        if token.count != 1 || token.generation == 0 || token.heap_id != self.root_namespace {
            return false;
        }
        let mut state = self.lock_state();
        {
            let State {
                roots,
                root_generations,
                ..
            } = &mut *state;
            let Some(generation) = root_generations.get_mut(token.index) else {
                return false;
            };
            if *generation != token.generation {
                return false;
            }
            let Some(root) = roots.get_mut(token.index) else {
                return false;
            };
            if root.is_null() {
                return false;
            }
            *root = std::ptr::null_mut();
            *generation = 0;
        }
        trim_root_tombstones(&mut state);
        drop(state);
        true
    }
}
