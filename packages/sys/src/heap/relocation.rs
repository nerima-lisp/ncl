use super::{HashMap, Heap, State, Word};

impl Heap {
    pub(super) fn relocated_address(
        state: &State,
        moved: &HashMap<usize, usize>,
        value: Word,
    ) -> Option<usize> {
        let index = Self::find_raw(state, value)?;
        let object = state.objects.get(index)?;
        let new_base = moved.get(&object.start)?;
        new_base.checked_add(value.address().checked_sub(object.start)?)
    }
    pub(super) const fn relocated_word(value: Word, address: usize) -> Word {
        Word::pointer(
            address,
            if value.is_list() {
                crate::LowTag::List
            } else {
                crate::LowTag::OtherPointer
            },
        )
    }
}
