use crate::heap_state::{Object, State};
pub use crate::heap_types::{
    Finalizer, HeapConfig, LayoutError, PageKind, ReferenceLayout, StorageCondition, TypeTag,
    Weakness,
};
use crate::{Thread, Word};
use std::collections::{HashMap, HashSet};
use std::sync::{Condvar, Mutex};
const CARD_SIZE: usize = 512;
const LARGE_OBJECT: usize = 8 * 1024;
const WIDETAG_MASK: u64 = 0xff;
const FORWARDED_FLAG: u64 = 1 << 10;
#[derive(Debug)]
pub struct Heap {
    config: HeapConfig,
    state: Mutex<State>,
    pub(crate) stop_world: Mutex<crate::stw::StopWorld>,
    pub(crate) stop_world_ready: Condvar,
}
impl Default for HeapConfig {
    fn default() -> Self {
        Self {
            dynamic_space_size: 64 * 1024 * 1024,
            bytes_considered_between_gcs: 4 * 1024 * 1024,
        }
    }
}
impl Heap {
    #[must_use]
    pub fn new(config: HeapConfig) -> Self {
        Self {
            config,
            state: Mutex::new(State {
                used: 0,
                objects: Vec::new(),
                layouts: HashMap::new(),
                threads: Vec::new(),
                dirty_cards: HashSet::new(),
                roots: Vec::new(),
                finalizers: Vec::new(),
                after_gc_hooks: Vec::new(),
            }),
            stop_world: Mutex::new(crate::stw::StopWorld::default()),
            stop_world_ready: Condvar::new(),
        }
    }
    pub const fn dynamic_space_size(&self) -> usize {
        self.config.dynamic_space_size
    }
    pub const fn bytes_considered_between_gcs(&self) -> usize {
        self.config.bytes_considered_between_gcs
    }
    /// # Errors
    /// Returns `LayoutError` when the widetag is already registered.
    pub fn register_layout(&self, widetag: u8, layout: ReferenceLayout) -> Result<(), LayoutError> {
        let mut state = self.lock_state();
        if state.layouts.insert(widetag, layout).is_some() {
            Err(LayoutError)
        } else {
            Ok(())
        }
    }
    pub(crate) fn register_thread(&self, thread: &mut Thread) -> Result<(), StorageCondition> {
        let mut state = self.lock_state();
        let pointer = std::ptr::from_mut(thread);
        if state.threads.contains(&pointer) {
            return Err(StorageCondition::ThreadNotRegistered);
        }
        state.threads.push(pointer);
        drop(state);
        thread.heap = Some(self);
        Ok(())
    }
    pub(crate) fn unregister_thread(&self, thread: &Thread) {
        let mut state = self.lock_state();
        let pointer = std::ptr::from_ref(thread).cast_mut();
        state.threads.retain(|item| *item != pointer);
        drop(state);
        let mut stop_world = self
            .stop_world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        stop_world.parked.remove(&(pointer as usize));
        drop(stop_world);
        self.stop_world_ready.notify_all();
    }
    pub(crate) fn active_mutators(&self) -> usize {
        let state = self.lock_state();
        state
            .threads
            .iter()
            .filter(|candidate| {
                // SAFETY: registered thread pointers are valid until unregister_thread.
                unsafe {
                    let pointer = **candidate;
                    (&*pointer).native_state() != crate::NativeState::Native
                }
            })
            .count()
    }
    pub(crate) fn alloc(
        &self,
        thread: &mut Thread,
        tag: TypeTag,
        words: usize,
    ) -> Result<Word, StorageCondition> {
        self.allocate(
            thread,
            PageKind::HeaderObject,
            tag,
            words.checked_add(1).ok_or(StorageCondition::InvalidSize)?,
        )
    }
    pub(crate) fn alloc_large(
        &self,
        thread: &mut Thread,
        tag: TypeTag,
        words: usize,
    ) -> Result<Word, StorageCondition> {
        self.allocate(
            thread,
            PageKind::Large,
            tag,
            words.checked_add(1).ok_or(StorageCondition::InvalidSize)?,
        )
    }
    pub(crate) fn alloc_cons(
        &self,
        thread: &mut Thread,
        car: Word,
        cdr: Word,
    ) -> Result<Word, StorageCondition> {
        let value = self.allocate(thread, PageKind::Cons, TypeTag { widetag: 0 }, 2)?;
        self.write_words(value, &[(0, car), (1, cdr)]);
        Ok(value)
    }
    pub(crate) fn read_word(&self, object: Word, slot: usize) -> Option<Word> {
        let state = self.lock_state();
        let index = Self::find(&state, object)?;
        state.objects[index]
            .words
            .get(slot)
            .copied()
            .map(Word::from_bits)
    }
    pub(crate) fn read_cons_word(&self, object: Word, slot: usize) -> Option<Word> {
        self.read_word(object, slot)
    }
    pub(crate) fn write_cons_word(&self, object: Word, slot: usize, value: Word) -> bool {
        self.write_word_at(object, slot, value)
    }
    pub(crate) fn write_word(&self, object: Word, slot: usize, value: Word) -> bool {
        self.write_word_at(object, slot, value)
    }
    fn write_word_at(&self, object: Word, slot: usize, value: Word) -> bool {
        let mut state = self.lock_state();
        Self::find(&state, object).is_some_and(|index| {
            state.objects[index]
                .words
                .get_mut(slot)
                .is_some_and(|target| {
                    *target = value.bits();
                    true
                })
        })
    }
    pub(crate) fn widetag(&self, object: Word) -> Option<u8> {
        let state = self.lock_state();
        let index = Self::find(&state, object)?;
        Some(Self::object_widetag(&state.objects[index]))
    }
    fn allocate(
        &self,
        thread: &mut Thread,
        kind: PageKind,
        tag: TypeTag,
        words: usize,
    ) -> Result<Word, StorageCondition> {
        if words == 0 || words > (1 << 20) {
            return Err(StorageCondition::InvalidSize);
        }
        if thread.heap != Some(self) {
            return Err(StorageCondition::ThreadNotRegistered);
        }
        let bytes = words.checked_mul(8).ok_or(StorageCondition::InvalidSize)?;
        let mut state = self.lock_state();
        if state
            .used
            .checked_add(bytes)
            .is_none_or(|size| size > self.config.dynamic_space_size)
        {
            return Err(StorageCondition::CapacityExceeded);
        }
        let mut data = vec![0_u64; words].into_boxed_slice();
        if kind != PageKind::Cons {
            data[0] = u64::from(tag.widetag);
        }
        let address = data.as_ptr() as usize;
        state.used += bytes;
        state.objects.push(Object {
            words: data,
            kind,
            generation: if bytes >= LARGE_OBJECT { 2 } else { 0 },
            survived: 0,
            pinned: false,
            weak: None,
            finalizer: None,
            alive: true,
            forwarded_to: None,
        });
        drop(state);
        thread.bytes_cons += bytes;
        Ok(Word::pointer(
            address,
            if kind == PageKind::Cons {
                crate::LowTag::List
            } else {
                crate::LowTag::OtherPointer
            },
        ))
    }
    fn find_raw(state: &State, value: Word) -> Option<usize> {
        let address = value.address();
        state.objects.iter().position(|object| {
            (object.alive || object.forwarded_to.is_some()) && {
                let start = object.words.as_ptr() as usize;
                address >= start
                    && address < start + object.words.len() * 8
                    && (address - start).is_multiple_of(8)
            }
        })
    }
    fn find(state: &State, value: Word) -> Option<usize> {
        let mut index = Self::find_raw(state, value)?;
        while let Some(next) = state.objects[index].forwarded_to {
            index = next;
        }
        state.objects[index].alive.then_some(index)
    }
    fn layout(state: &State, index: usize) -> Vec<usize> {
        if state.objects[index].kind == PageKind::Cons {
            return vec![0, 1];
        }
        state
            .layouts
            .get(&Self::object_widetag(&state.objects[index]))
            .map_or_else(Vec::new, |layout| layout.reference_words.clone())
    }
    fn write_words(&self, object: Word, values: &[(usize, Word)]) {
        let mut state = self.lock_state();
        if let Some(index) = Self::find(&state, object) {
            for (slot, value) in values {
                if *slot < state.objects[index].words.len() {
                    state.objects[index].words[*slot] = value.bits();
                }
            }
        }
    }
    pub(crate) fn barrier(&self, object: Word, slot: usize) {
        let mut state = self.lock_state();
        if let Some(index) = Self::find(&state, object)
            && state.objects[index].generation > 0
            && slot < state.objects[index].words.len()
        {
            let address = state.objects[index].words.as_ptr() as usize + slot * 8;
            state.dirty_cards.insert((index, address / CARD_SIZE));
        }
    }
    pub(crate) fn make_weak(&self, value: Word, weakness: Weakness) -> Word {
        let mut state = self.lock_state();
        if let Some(index) = Self::find(&state, value) {
            state.objects[index].weak = Some(weakness);
        }
        value
    }
    pub(crate) fn weak_value(&self, value: Word) -> Word {
        let state = self.lock_state();
        Self::find(&state, value)
            .and_then(|i| state.objects[i].words.get(1).copied())
            .map_or(Word::NIL, Word::from_bits)
    }
    pub(crate) fn register_finalizer(&self, object: Word, callback: Finalizer) {
        let mut state = self.lock_state();
        if let Some(index) = Self::find(&state, object) {
            state.objects[index].finalizer = Some((callback, false));
        }
    }
    pub(crate) fn run_pending_finalizers(&self) {
        let mut state = self.lock_state();
        let pending = std::mem::take(&mut state.finalizers);
        drop(state);
        for (_, callback) in pending {
            callback(Word::NIL);
        }
    }
    #[allow(
        clippy::too_many_lines,
        reason = "collection phases share one lock to preserve forwarding invariants"
    )]
    pub(crate) fn collect(&self, full: bool) {
        let mut state = self.lock_state();
        let mut live = HashSet::new();
        let mut stack = Vec::new();
        let mut root_slots = state.roots.clone();
        let mut conservative_values = Vec::new();
        for thread in state.threads.iter().copied() {
            // SAFETY: registered thread pointers remain valid until unregister_thread.
            unsafe {
                root_slots.extend((*thread).roots.iter().copied());
                conservative_values.extend((*thread).conservative_roots.iter().copied());
            }
        }
        let mut conservative_indices = Vec::new();
        for value in conservative_values {
            if let Some(index) = Self::find(&state, value) {
                state.objects[index].pinned = true;
                conservative_indices.push(index);
            }
        }
        if !full {
            for (index, _) in state.dirty_cards.clone() {
                if state.objects.get(index).is_some_and(|object| object.alive) {
                    stack.push(index);
                }
            }
        }
        for root in root_slots.iter().copied() {
            if !root.is_null() {
                // SAFETY: registered root slots outlive registration.
                let value = unsafe { *root };
                if let Some(index) = Self::find(&state, value) {
                    stack.push(index);
                }
            }
        }
        stack.extend(conservative_indices);
        while let Some(index) = stack.pop() {
            if !live.insert(index) {
                continue;
            }
            for slot in Self::layout(&state, index) {
                if state.objects[index].weak.is_some() && slot == 1 {
                    continue;
                }
                if let Some(value) = state.objects[index]
                    .words
                    .get(slot)
                    .copied()
                    .map(Word::from_bits)
                    && let Some(next) = Self::find(&state, value)
                {
                    stack.push(next);
                }
            }
        }
        let mut moved = HashMap::new();
        let original_len = state.objects.len();
        for index in 0..original_len {
            if !live.contains(&index)
                || state.objects[index].generation >= 2
                || state.objects[index].pinned
            {
                continue;
            }
            let source = &state.objects[index];
            let mut copy = Object {
                words: source.words.clone(),
                kind: source.kind,
                generation: source.generation,
                survived: source.survived.saturating_add(1),
                pinned: source.pinned,
                weak: source.weak,
                finalizer: source.finalizer,
                alive: true,
                forwarded_to: None,
            };
            copy.generation = copy.survived.min(2);
            let old_address = source.words.as_ptr() as usize;
            let new_address = copy.words.as_ptr() as usize;
            let new_index = state.objects.len();
            state.objects[index].forwarded_to = Some(new_index);
            state.objects[index].alive = false;
            if state.objects[index].kind != PageKind::Cons {
                state.objects[index].words[0] |= FORWARDED_FLAG;
                if state.objects[index].words.len() > 1 {
                    state.objects[index].words[1] = 0;
                }
            }
            moved.insert(old_address, new_address);
            state.objects.push(copy);
            live.insert(new_index);
        }
        for root in root_slots.iter().copied() {
            if root.is_null() {
                continue;
            }
            // SAFETY: registered root slots remain valid and uniquely mutable by their owner.
            let value = unsafe { *root };
            if let Some(address) = Self::relocated_address(&state, &moved, value) {
                // SAFETY: the root slot is registered and points to a valid Word.
                unsafe {
                    *root = Word::pointer(
                        address,
                        if value.is_list() {
                            crate::LowTag::List
                        } else {
                            crate::LowTag::OtherPointer
                        },
                    );
                }
            }
        }
        for index in 0..state.objects.len() {
            if !state.objects[index].alive {
                continue;
            }
            for slot in Self::layout(&state, index) {
                if state.objects[index].weak.is_some() && slot == 1 {
                    continue;
                }
                if let Some(value) = state.objects[index]
                    .words
                    .get(slot)
                    .copied()
                    .map(Word::from_bits)
                    && let Some(address) = Self::relocated_address(&state, &moved, value)
                {
                    state.objects[index].words[slot] = Word::pointer(
                        address,
                        if value.is_list() {
                            crate::LowTag::List
                        } else {
                            crate::LowTag::OtherPointer
                        },
                    )
                    .bits();
                }
            }
        }
        for index in 0..state.objects.len() {
            if !state.objects[index].alive || state.objects[index].weak.is_none() {
                continue;
            }
            let value = state.objects[index]
                .words
                .get(1)
                .copied()
                .map_or(Word::NIL, Word::from_bits);
            let retained = Self::find(&state, value).is_some_and(|target| {
                full || live.contains(&target) || state.objects[target].generation >= 2
            });
            if !retained {
                state.objects[index].words[1] = Word::NIL.bits();
            }
        }
        for index in 0..state.objects.len() {
            if !state.objects[index].alive {
                continue;
            }
            let collect = !live.contains(&index) && (full || state.objects[index].generation < 2);
            if !collect {
                continue;
            }
            if let Some((callback, false)) = state.objects[index].finalizer {
                state.finalizers.push((Word::NIL, callback));
                state.objects[index].finalizer = Some((callback, true));
            }
            state.used = state
                .used
                .saturating_sub(state.objects[index].words.len() * 8);
            state.objects[index].alive = false;
        }
        let dirty_cards = state
            .dirty_cards
            .iter()
            .copied()
            .filter(|(index, _)| {
                state
                    .objects
                    .get(*index)
                    .is_some_and(|object| object.alive && object.generation > 0)
            })
            .collect();
        state.dirty_cards = dirty_cards;
        let hooks = state.after_gc_hooks.clone();
        for thread in state.threads.iter().copied() {
            // SAFETY: collection owns the stop-the-world phase, so mutator snapshots are not changing.
            unsafe { (*thread).conservative_roots.clear() };
        }
        drop(state);
        for hook in hooks {
            hook();
        }
    }
    pub(crate) fn register_after_gc_hook(&self, hook: fn()) {
        self.lock_state().after_gc_hooks.push(hook);
    }
    fn lock_state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    fn object_widetag(object: &Object) -> u8 {
        u8::try_from(object.words[0] & WIDETAG_MASK).unwrap_or(0)
    }
    fn relocated_address(
        state: &State,
        moved: &HashMap<usize, usize>,
        value: Word,
    ) -> Option<usize> {
        let index = Self::find_raw(state, value)?;
        let object = &state.objects[index];
        let new_base = moved.get(&(object.words.as_ptr() as usize))?;
        Some(new_base + (value.address() - object.words.as_ptr() as usize))
    }
}
// SAFETY: State is accessed only while holding Heap::state; registered pointers are valid until unregistering.
unsafe impl Send for State {}
// SAFETY: Heap::state serializes access and collection runs while mutators are stopped.
unsafe impl Sync for State {}
#[cfg(test)]
mod tests;
