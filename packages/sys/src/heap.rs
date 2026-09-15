use crate::heap_state::{Object, State};
pub use crate::heap_types::{
    Finalizer, HeapConfig, LayoutError, PageKind, ReferenceLayout, StorageCondition, TypeTag,
    Weakness,
};
use crate::{CodeError, CodeObjectMetadata, CodePtr, Thread, Word};
use std::collections::{HashMap, HashSet};
use std::sync::{Condvar, Mutex};
#[path = "heap/collect.rs"]
mod collect;
#[path = "heap/scan.rs"]
mod scan;
const CARD_SIZE: usize = 512;
const LARGE_OBJECT: usize = 8 * 1024;
const WIDETAG_MASK: u64 = 0xff;
const FORWARDED_FLAG: u64 = 1 << 10;
#[derive(Debug)]
/// Moving heap and its stop-the-world coordination state.
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
    /// Construct an empty heap with the supplied capacity policy.
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
                code_registry: crate::CodeRegistry::default(),
            }),
            stop_world: Mutex::new(crate::stw::StopWorld::default()),
            stop_world_ready: Condvar::new(),
        }
    }
    /// Return the configured dynamic-space capacity in bytes.
    pub const fn dynamic_space_size(&self) -> usize {
        self.config.dynamic_space_size
    }
    /// Return the allocation debt threshold that triggers collection.
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
    /// Register published code metadata for precise native frame scanning.
    ///
    /// # Errors
    ///
    /// Returns `CodeError::NotPublished` when the code is not executable yet.
    pub fn register_code(
        &self,
        code: &CodePtr,
        metadata: CodeObjectMetadata,
    ) -> Result<(), CodeError> {
        self.lock_state().code_registry.register(code, metadata)
    }
    /// Remove code metadata before releasing its non-moving allocation.
    pub fn unregister_code(&self, code: &CodePtr) -> Option<CodeObjectMetadata> {
        self.lock_state().code_registry.unregister(code)
    }
    /// Quiesce all registered mutators, then unregister and release executable code.
    ///
    /// `code` remains in `owned` when a published return PC is still present, so
    /// the caller can retry after the owning frames have unwound.
    ///
    /// # Errors
    ///
    /// Returns [`CodeError::CodeInUse`] while a stopped frame still points into
    /// the mapping, or [`CodeError::NotRegistered`] when the registry has no entry.
    pub fn release_code(
        &self,
        thread: &mut Thread,
        owned: &mut Option<CodePtr>,
    ) -> Result<(), CodeError> {
        if thread.heap != Some(self) {
            return Err(CodeError::NotRegistered);
        }
        let code = owned.as_ref().ok_or(CodeError::NotRegistered)?;
        self.begin_collection(thread);
        let live = {
            let state = self.lock_state();
            if state.code_registry.find(code.address()).is_none() {
                false
            } else {
                state.threads.iter().copied().any(|candidate| {
                    // SAFETY: collection has stopped registered mutators.
                    unsafe {
                        (*candidate)
                            .frame_chain
                            .iter()
                            .skip(1)
                            .step_by(5)
                            .any(|return_pc| {
                                let pc = return_pc.address();
                                pc >= code.address() && pc < code.address() + code.len()
                            })
                    }
                })
            }
        };
        if live {
            self.end_collection();
            return Err(CodeError::CodeInUse);
        }
        let code = owned.take().ok_or(CodeError::NotRegistered)?;
        let removed = self.lock_state().code_registry.unregister(&code);
        self.end_collection();
        if removed.is_none() {
            drop(code);
            return Err(CodeError::NotRegistered);
        }
        drop(code);
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
    fn find_conservative(state: &State, value: Word) -> Option<usize> {
        let expected = if value.is_list() {
            PageKind::Cons
        } else if value.lowtag() == crate::LowTag::OtherPointer as u8 {
            PageKind::HeaderObject
        } else {
            return None;
        };
        let index = Self::find_raw(state, value)?;
        let object = &state.objects[index];
        (object.kind == expected && value.address() == object.words.as_ptr() as usize)
            .then_some(index)
    }
    pub(crate) fn read_word(&self, object: Word, slot: usize) -> Option<Word> {
        let state = self.lock_state();
        state.objects[Self::find(&state, object)?]
            .words
            .get(slot)
            .map(|value| Word::from_bits(*value))
    }
    pub(crate) fn write_word(&self, object: Word, slot: usize, value: Word) -> bool {
        self.write_word_at(object, slot + 1, value)
    }
    pub(crate) fn write_word_at(&self, object: Word, slot: usize, value: Word) -> bool {
        let mut state = self.lock_state();
        Self::find(&state, object)
            .and_then(|index| state.objects[index].words.get_mut(slot))
            .map(|target| *target = value.bits())
            .is_some()
    }
    pub(crate) fn widetag(&self, object: Word) -> Option<u8> {
        let state = self.lock_state();
        let index = Self::find(&state, object)?;
        Some(u8::try_from(state.objects[index].words[0] & WIDETAG_MASK).unwrap_or(0))
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
    pub(crate) fn register_after_gc_hook(&self, hook: fn()) {
        self.lock_state().after_gc_hooks.push(hook);
    }
    fn lock_state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
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
    const fn relocated_word(value: Word, address: usize) -> Word {
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
// SAFETY: State is accessed only while holding Heap::state; registered pointers are valid until unregistering.
unsafe impl Send for State {}
// SAFETY: Heap::state serializes access and collection runs while mutators are stopped.
unsafe impl Sync for State {}
#[cfg(test)]
mod tests;
