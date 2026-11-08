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
        let function_pointer_tag = Word::pointer(0, crate::LowTag::Function).lowtag();
        Word::pointer(
            address,
            if value.is_list() {
                crate::LowTag::List
            } else if value.lowtag() == function_pointer_tag {
                crate::LowTag::Function
            } else {
                crate::LowTag::OtherPointer
            },
        )
    }
}
