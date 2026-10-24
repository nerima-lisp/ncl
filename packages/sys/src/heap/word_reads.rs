use super::{Heap, diagnostics::log_word_rejection};
use crate::Word;
use std::sync::atomic::Ordering;

impl Heap {
    #[track_caller]
    pub(crate) fn read_word(&self, object: Word, slot: usize) -> Option<Word> {
        self.read_word_with_operation("read_word", object, slot)
    }

    #[track_caller]
    pub(crate) fn read_cons_word(&self, object: Word, slot: usize) -> Option<Word> {
        self.read_word_with_operation("read_cons_word", object, slot)
    }

    #[track_caller]
    fn read_word_with_operation(
        &self,
        operation: &'static str,
        object: Word,
        slot: usize,
    ) -> Option<Word> {
        let state = self.lock_state();
        let raw_index = Self::find_raw(&state, object);
        let forwarded_index = raw_index.and_then(|index| state.objects[index].forwarded_to);
        let strict_forwarding = self.strict_forwarding.load(Ordering::Relaxed);
        let index = self.find_for_mutator(&state, object);
        let Some(index) = index else {
            let reason = if strict_forwarding && forwarded_index.is_some() {
                "strict_forwarding_rejected_forwarded_object"
            } else if raw_index.is_none() {
                "object_not_found"
            } else {
                "object_unavailable"
            };
            let forwarding_target =
                forwarded_index.map(|index| state.objects[index].words.as_ptr().addr());
            let gc_epoch = state.gc_epoch;
            drop(state);
            log_word_rejection(
                operation,
                reason,
                object,
                slot,
                None,
                forwarding_target,
                raw_index.is_some(),
                gc_epoch,
            );
            return None;
        };
        let Some(value) = state.objects[index].words.get(slot) else {
            let forwarding_target =
                forwarded_index.map(|index| state.objects[index].words.as_ptr().addr());
            let gc_epoch = state.gc_epoch;
            drop(state);
            log_word_rejection(
                operation,
                "slot_out_of_bounds",
                object,
                slot,
                None,
                forwarding_target,
                raw_index.is_some(),
                gc_epoch,
            );
            return None;
        };
        Some(Word::from_bits(*value))
    }
}
