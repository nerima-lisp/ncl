use super::{Heap, PageKind, State, WIDETAG_MASK, Word};
use std::sync::atomic::Ordering;

const CLOSURE_WIDETAG: u8 = 9;
pub(super) const SIMPLE_FUN_WIDETAG: u8 = 8;

impl Heap {
    /// Resolve a heap word through forwarding for registry lookups.
    #[must_use]
    pub fn forwarded_word(&self, value: Word) -> Option<Word> {
        let state = self.lock_state();
        let index = Self::find(&state, value)?;
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
        let other_pointer_tag = Word::pointer(0, crate::LowTag::OtherPointer).lowtag();
        let function_pointer_tag = Word::pointer(0, crate::LowTag::Function).lowtag();
        let is_function_pointer = value.lowtag() == function_pointer_tag;
        let is_heap_pointer =
            value.is_cons() || value.lowtag() == other_pointer_tag || is_function_pointer;
        if !is_heap_pointer {
            return None;
        }
        let address = value.address();
        let index = state.object_index_at(address)?;
        let object = state.objects.get(index)?;
        if is_function_pointer
            && (object.kind != PageKind::HeaderObject
                || !object.words.first().is_some_and(|header| {
                    let widetag = *header & WIDETAG_MASK;
                    widetag == u64::from(SIMPLE_FUN_WIDETAG)
                        || widetag == u64::from(CLOSURE_WIDETAG)
                }))
        {
            return None;
        }
        (object.alive || object.forwarded_to.is_some()).then_some(index)
    }

    pub(super) fn find(state: &State, value: Word) -> Option<usize> {
        Self::follow_forwarding(state, Self::find_raw(state, value)?)
    }

    pub(super) fn find_for_mutator(&self, state: &State, value: Word) -> Option<usize> {
        let index = Self::find_raw(state, value)?;
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
