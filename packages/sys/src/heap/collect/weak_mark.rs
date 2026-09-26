use super::{
    HASH_TABLE_HIGH_WATER, HASH_TABLE_KV, HASH_TABLE_MARKER, HashMap, HashSet, VECTOR_DATA,
    Weakness, Word, scan,
};
pub(super) struct WeakMarkContext<'a> {
    pub(super) state: &'a super::super::State,
    pub(super) full: bool,
    pub(super) live: &'a mut HashSet<usize>,
    pub(super) stack: &'a mut Vec<usize>,
    pub(super) weak_kv: &'a mut HashMap<usize, Weakness>,
    pub(super) pending: &'a mut Vec<usize>,
}

impl WeakMarkContext<'_> {
    pub(super) fn drain(&mut self) {
        loop {
            while let Some(index) = self.stack.pop() {
                if !self.live.insert(index) {
                    continue;
                }
                if super::super::Heap::is_hash_table(self.state, index)
                    && let Some(weakness) =
                        super::super::Heap::hash_table_weakness(self.state, index)
                {
                    self.mark_table(index, weakness);
                    continue;
                }
                if self.weak_kv.contains_key(&index) {
                    continue;
                }
                for slot in scan::layout(self.state, index) {
                    if self.state.objects[index].weak.is_some() && slot == 1 {
                        continue;
                    }
                    if let Some(value) = self.state.objects[index]
                        .words
                        .get(slot)
                        .copied()
                        .map(Word::from_bits)
                        && let Some(next) = super::super::Heap::find(self.state, value)
                    {
                        self.stack.push(next);
                    }
                }
            }
            let pending = std::mem::take(self.pending);
            let old_live_count = self.live.len();
            for index in pending {
                if self.live.contains(&index)
                    && let Some(weakness) =
                        super::super::Heap::hash_table_weakness(self.state, index)
                {
                    self.mark_table(index, weakness);
                }
            }
            if self.stack.is_empty() && self.live.len() == old_live_count {
                break;
            }
        }
    }

    fn mark_table(&mut self, table: usize, weakness: Weakness) {
        let words = &self.state.objects[table].words;
        for slot in scan::layout(self.state, table) {
            if slot != HASH_TABLE_KV
                && let Some(next) = words
                    .get(slot)
                    .copied()
                    .map(Word::from_bits)
                    .and_then(|value| super::super::Heap::find(self.state, value))
            {
                self.stack.push(next);
            }
        }
        let Some(kv) = words
            .get(HASH_TABLE_KV)
            .copied()
            .map(Word::from_bits)
            .and_then(|value| super::super::Heap::find(self.state, value))
        else {
            return;
        };
        self.weak_kv.insert(kv, weakness);
        self.stack.push(kv);
        let marker = words.get(HASH_TABLE_MARKER).copied().map(Word::from_bits);
        let high_water = words
            .get(HASH_TABLE_HIGH_WATER)
            .copied()
            .map(Word::from_bits)
            .and_then(Word::as_fixnum)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(0);
        for position in 0..high_water {
            let Some(key) = self.state.objects[kv]
                .words
                .get(VECTOR_DATA + position * 2)
                .copied()
                .map(Word::from_bits)
            else {
                continue;
            };
            if Some(key) == marker {
                continue;
            }
            let Some(value) = self.state.objects[kv]
                .words
                .get(VECTOR_DATA + position * 2 + 1)
                .copied()
                .map(Word::from_bits)
            else {
                continue;
            };
            match weakness {
                Weakness::Key => self.mark_value(value),
                Weakness::Value => self.mark_value(key),
                Weakness::KeyAndValue => {}
                Weakness::KeyOrValue => {
                    let key_live = self.referent_is_live(key);
                    let value_live = self.referent_is_live(value);
                    if key_live || value_live {
                        self.mark_value(key);
                        self.mark_value(value);
                    } else {
                        self.pending.push(table);
                    }
                }
            }
        }
    }

    fn mark_value(&mut self, value: Word) {
        if let Some(index) = super::super::Heap::find(self.state, value) {
            self.stack.push(index);
        }
    }

    pub(super) fn referent_is_live(&self, value: Word) -> bool {
        referent_is_live(self.state, self.live, self.full, value)
    }
}

pub(super) fn referent_is_live(
    state: &super::super::State,
    live: &HashSet<usize>,
    full: bool,
    value: Word,
) -> bool {
    if is_immediate(value) {
        return true;
    }
    super::super::Heap::find(state, value).is_some_and(|index| {
        live.contains(&index) || (!full && state.objects[index].generation >= 2)
    })
}

fn is_immediate(value: Word) -> bool {
    value.is_fixnum()
        || value == Word::NIL
        || value == Word::TRUE
        || value.is_character()
        || value.is_unbound()
}
