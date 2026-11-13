use super::{HashMap, HashSet, Word};

pub(super) fn update_weak_slot(
    state: &mut super::super::State,
    moved: &HashMap<usize, usize>,
    live: &HashSet<usize>,
    index: usize,
    full: bool,
) {
    let Some(object) = state.objects.get(index) else {
        return;
    };
    if !object.alive || object.weak.is_none() {
        return;
    }
    let value = object
        .words
        .get(1)
        .copied()
        .map_or(Word::NIL, Word::from_bits);
    let replacement = if super::weak_mark::referent_is_live(state, live, full, value) {
        super::super::Heap::relocated_address(state, moved, value)
            .map(|address| super::super::Heap::relocated_word(value, address))
    } else {
        Some(Word::NIL)
    };
    if let Some(replacement) = replacement
        && let Some(slot) = state
            .objects
            .get_mut(index)
            .and_then(|object| object.words.get_mut(1))
    {
        *slot = replacement.bits();
    }
}
