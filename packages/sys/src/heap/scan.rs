use super::{PageKind, State, WIDETAG_MASK};

pub(super) fn layout(state: &State, index: usize) -> Vec<usize> {
    if state.objects[index].kind == PageKind::Cons {
        return vec![0, 1];
    }
    let Some(reference_layout) = state.layouts.get(&widetag(state, index)) else {
        return Vec::new();
    };
    let mut slots = reference_layout.reference_words.clone();
    if let Some(start) = reference_layout.boxed_from {
        let end = state.objects[index].words.len();
        slots.extend(start..end);
    }
    slots.sort_unstable();
    slots.dedup();
    slots
}

fn widetag(state: &State, index: usize) -> u8 {
    u8::try_from(state.objects[index].words[0] & WIDETAG_MASK).unwrap_or(0)
}
