use super::{CARD_SIZE, Heap};
use crate::Word;

impl Heap {
    pub(crate) fn barrier(&self, object: Word, slot: usize) {
        let mut state = self.lock_state();
        if let Some(index) = self.find_for_mutator(&state, object)
            && let Some(entry) = state.objects.get(index)
            && entry.generation > 0
            && slot < entry.words.len()
        {
            if state.collection_preparing {
                // The scratch snapshot cannot grow after preflight; overflow makes
                // the minor collector scan all old objects and preserves this write.
                state.remembered_set_overflow = true;
                return;
            }
            let object_address = entry.words.as_ptr().addr();
            let address = object_address + slot * 8;
            if state
                .dirty_cards
                .insert((object_address, address / CARD_SIZE))
                .is_err()
            {
                state.remembered_set_overflow = true;
            }
        }
    }
}
