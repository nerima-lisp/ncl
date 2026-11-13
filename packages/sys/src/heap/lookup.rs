use super::{Heap, PageKind, State, Word};
use std::sync::atomic::Ordering;

const CLOSURE_WIDETAG: u8 = 9;
pub(super) const SIMPLE_FUN_WIDETAG: u8 = 8;

impl Heap {
    /// Resolve a heap word through forwarding for registry lookups.
    #[must_use]
    pub fn forwarded_word(&self, value: Word) -> Option<Word> {
        let mut state = self.lock_state();
        let index = Self::find_raw_with_pending(self, &mut state, value)?;
        let index = Self::follow_forwarding(&state, index)?;
        let object = state.objects.get(index)?;
        let lowtag = value.lowtag();
        let tag = if lowtag == Word::pointer(0, crate::LowTag::List).lowtag() {
            crate::LowTag::List
        } else if lowtag == Word::pointer(0, crate::LowTag::Function).lowtag() {
            crate::LowTag::Function
        } else if lowtag == Word::pointer(0, crate::LowTag::Instance).lowtag() {
            crate::LowTag::Instance
        } else if lowtag == Word::pointer(0, crate::LowTag::OtherPointer).lowtag() {
            crate::LowTag::OtherPointer
        } else {
            return None;
        };
        let address = object.start;
        drop(state);
        Some(Word::pointer(address, tag))
    }

    pub(super) fn find_raw(state: &State, value: Word) -> Option<usize> {
        let function_pointer_tag = Word::pointer(0, crate::LowTag::Function).lowtag();
        let is_function_pointer = value.lowtag() == function_pointer_tag;
        if !Self::is_heap_pointer(value) {
            return None;
        }
        let address = value.address();
        let index = state.object_index_at(address)?;
        let object = state.objects.get(index)?;
        if is_function_pointer
            && (object.kind != PageKind::HeaderObject
                || !matches!(object.widetag, SIMPLE_FUN_WIDETAG | CLOSURE_WIDETAG))
        {
            return None;
        }
        (object.alive || object.forwarded_to.is_some()).then_some(index)
    }

    pub(super) fn find(state: &State, value: Word) -> Option<usize> {
        Self::follow_forwarding(state, Self::find_raw(state, value)?)
    }

    const fn is_heap_pointer(value: Word) -> bool {
        let other_pointer_tag = Word::pointer(0, crate::LowTag::OtherPointer).lowtag();
        let function_pointer_tag = Word::pointer(0, crate::LowTag::Function).lowtag();
        value.is_cons()
            || value.lowtag() == other_pointer_tag
            || value.lowtag() == function_pointer_tag
    }

    pub(super) fn find_raw_with_pending(
        heap: &Self,
        state: &mut State,
        value: Word,
    ) -> Option<usize> {
        if let Some(index) = Self::find_raw(state, value) {
            return Some(index);
        }
        if Self::is_heap_pointer(value) && heap.flush_pending_allocation_for(state, value.address())
        {
            Self::find_raw(state, value)
        } else {
            None
        }
    }

    pub(super) fn find_for_mutator(&self, state: &mut State, value: Word) -> Option<usize> {
        let index = Self::find_raw_with_pending(self, state, value)?;
        let object = state.objects.get(index)?;
        if self.strict_forwarding.load(Ordering::Relaxed) && object.forwarded_to.is_some() {
            return None;
        }
        Self::follow_forwarding(state, index)
    }

    fn follow_forwarding(state: &State, mut index: usize) -> Option<usize> {
        for _ in 0..state.objects.len() {
            let object = state.objects.get(index)?;
            if let Some(next) = object.forwarded_to {
                index = next;
            } else {
                return object.alive.then_some(index);
            }
        }
        None
    }

    pub(super) fn find_conservative(state: &State, value: Word) -> Option<usize> {
        let index = Self::find_raw(state, value)?;
        let object = state.objects.get(index)?;
        if !object.alive || object.words.is_empty() {
            return None;
        }
        let kind_matches = if value.is_list() {
            object.kind == PageKind::Cons
        } else if value.lowtag() == Word::pointer(0, crate::LowTag::OtherPointer).lowtag() {
            matches!(object.kind, PageKind::HeaderObject | PageKind::Large)
        } else {
            false
        };
        (kind_matches && value.address() == object.start).then_some(index)
    }
}
