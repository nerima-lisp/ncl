use super::super::{Heap, State};
use super::{HashMap, PageKind, Word};

pub(super) fn snapshot_root_slots(root_slots: &[*mut Word]) -> Vec<(*mut Word, Word)> {
    root_slots
        .iter()
        .copied()
        .filter(|root| !root.is_null())
        .map(|root| {
            // SAFETY: registered root slots remain valid through collection.
            let value = unsafe { *root };
            (root, value)
        })
        .collect()
}

pub(super) fn record_root_slot_status(
    heap: &Heap,
    state: &State,
    root_slots: &[*mut Word],
    moved: &HashMap<usize, usize>,
    roots_before_collection: &[(*mut Word, Word)],
) {
    let slots_were_forwarded = roots_match_moved_map(moved, roots_before_collection);
    let slots_are_live = all_root_slots_live(state, root_slots);
    heap.registered_root_slots_live_and_forwarded_for_test
        .store(
            slots_were_forwarded && slots_are_live,
            std::sync::atomic::Ordering::Relaxed,
        );
}

fn all_root_slots_live(state: &State, root_slots: &[*mut Word]) -> bool {
    root_slots
        .iter()
        .copied()
        .filter(|root| !root.is_null())
        .all(|root| {
            // SAFETY: the world is stopped and collection just updated these registered slots.
            let value = unsafe { *root };
            root_value_is_live(state, value)
        })
}

fn roots_match_moved_map(moved: &HashMap<usize, usize>, roots: &[(*mut Word, Word)]) -> bool {
    let mut slots_match = true;
    for (root, original) in roots.iter().copied() {
        let expected = expected_root_word_after_move(moved, original);
        // SAFETY: snapshotted registered root slots remain valid through collection.
        let actual = unsafe { *root };
        slots_match &= actual == expected;
    }
    slots_match
}

fn expected_root_word_after_move(moved: &HashMap<usize, usize>, original: Word) -> Word {
    let tag = if original.is_cons() {
        crate::LowTag::List
    } else if is_other_pointer(original) {
        crate::LowTag::OtherPointer
    } else {
        return original;
    };
    moved
        .get(&original.address())
        .copied()
        .map_or(original, |address| Word::pointer(address, tag))
}

fn root_value_is_live(state: &State, value: Word) -> bool {
    if value.is_fixnum()
        || value == Word::NIL
        || value == Word::TRUE
        || value.is_character()
        || value.is_unbound()
    {
        return true;
    }

    let is_list = value.is_list();
    let is_other_pointer = is_other_pointer(value);
    if !is_list && !is_other_pointer {
        return false;
    }
    let Some(index) = state.object_starts.get(&value.address()).copied() else {
        return false;
    };
    let Some(object) = state.objects.get(index) else {
        return false;
    };
    object.alive
        && object.forwarded_to.is_none()
        && if is_list {
            object.kind == PageKind::Cons
        } else {
            object.kind != PageKind::Cons
        }
}

fn is_other_pointer(value: Word) -> bool {
    value != Word::TRUE && Word::pointer(value.address(), crate::LowTag::OtherPointer) == value
}
