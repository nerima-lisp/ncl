use super::super::{PageKind, Word};
use crate::heap_state::State;

pub(super) fn record_weak_table(state: &mut State, index: usize, slot: usize) {
    let is_weak_table = state.objects.get(index).is_some_and(|object| {
        object.kind != PageKind::Cons
            && object.words.first().copied() == Some(5)
            && object
                .words
                .get(slot)
                .copied()
                .map(Word::from_bits)
                .and_then(Word::as_fixnum)
                .is_some_and(|value| (1..=4).contains(&value))
    });
    if slot == 2 && is_weak_table {
        state.weak_tables.insert(index);
    }
}

impl super::Heap {
    pub(crate) fn write_word_at(&self, object: Word, slot: usize, value: Word) -> bool {
        let mut state = self.lock_state();
        let Some(index) = self.find_for_mutator(&state, object) else {
            return false;
        };
        let Some(target) = state
            .objects
            .get_mut(index)
            .and_then(|object| object.words.get_mut(slot))
        else {
            return false;
        };
        *target = value.bits();
        record_weak_table(&mut state, index, slot);
        true
    }

    pub(super) fn write_words(&self, object: Word, values: &[(usize, Word)]) {
        let mut state = self.lock_state();
        if let Some(index) = self.find_for_mutator(&state, object) {
            for (slot, value) in values {
                if let Some(target) = state
                    .objects
                    .get_mut(index)
                    .and_then(|object| object.words.get_mut(*slot))
                {
                    *target = value.bits();
                    record_weak_table(&mut state, index, *slot);
                }
            }
        }
    }
}
