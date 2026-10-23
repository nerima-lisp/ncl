use crate::heap::metadata::{MetadataBudget, MetadataVec};
use crate::heap::metadata_map::MetadataMap;
use crate::heap_state::{LayoutEntry, Object, State};
pub use crate::heap_types::{
    Finalizer, HeapConfig, HeapOptions, LayoutError, PageKind, ReferenceLayout, StorageCondition,
    TypeTag, Weakness,
};
use crate::{CodeError, CodeObjectMetadata, CodePtr, Thread, Word};
use std::alloc::Layout;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};
#[path = "heap/allocation.rs"]
mod allocation;
#[path = "heap/collect.rs"]
mod collect;
#[path = "heap/config.rs"]
mod config;
#[path = "heap/config_accessors.rs"]
mod config_accessors;
pub mod metadata;
pub mod metadata_map;
#[path = "heap/roots.rs"]
mod roots;
#[path = "heap/scan.rs"]
mod scan;
pub use roots::HeapRootError;
const CARD_SIZE: usize = 512;
const LARGE_OBJECT: usize = 8 * 1024;
const WIDETAG_MASK: u64 = 0xff;
const FORWARDED_FLAG: u64 = 1 << 10;
static NEXT_HEAP_ID: AtomicU64 = AtomicU64::new(1);
/// Heap objects must not occupy the address range used by immediate values.
pub const MIN_HEAP_ADDRESS: usize = 1_usize << 32;

const fn map_storage_code_error(error: StorageCondition) -> CodeError {
    match error {
        StorageCondition::ThreadNotRegistered => CodeError::NotRegistered,
        StorageCondition::HeapMetadataCapacityExceeded => CodeError::MetadataCapacityExceeded,
        condition => CodeError::CollectionFailed(condition),
    }
}

#[derive(Debug)]
/// Moving heap and its stop-the-world coordination state.
pub struct Heap {
    root_namespace: u64,
    config: HeapConfig,
    options: HeapOptions,
    metadata_budget: MetadataBudget,
    state: Mutex<State>,
    strict_forwarding: AtomicBool,
    slow_path_gate: Mutex<()>,
    slow_path_generation: Mutex<u64>,
    slow_path_ready: Condvar,
    pub(crate) stop_world: Mutex<crate::stw::StopWorld>,
    pub(crate) stop_world_ready: Condvar,
}
impl Heap {
    #[must_use]
    /// Construct an empty heap with the supplied capacity policy.
    ///
    /// This uses [`HeapOptions::default()`], which disables automatic collection
    /// and leaves collector metadata unbounded.
    pub fn new(config: HeapConfig) -> Self {
        Self::with_options(config, HeapOptions::default())
    }

    #[must_use]
    /// Construct an empty heap with explicit collection and metadata settings.
    pub fn with_options(config: HeapConfig, options: HeapOptions) -> Self {
        let metadata_budget = MetadataBudget::new(options.max_metadata_bytes);
        Self {
            root_namespace: NEXT_HEAP_ID.fetch_add(1, Ordering::Relaxed),
            config,
            options,
            metadata_budget: metadata_budget.clone(),
            state: Mutex::new(State::new(metadata_budget.clone())),
            strict_forwarding: AtomicBool::new(false),
            slow_path_gate: Mutex::new(()),
            slow_path_generation: Mutex::new(0),
            slow_path_ready: Condvar::new(),
            stop_world: Mutex::new(crate::stw::StopWorld::new(metadata_budget)),
            stop_world_ready: Condvar::new(),
        }
    }
    /// Verify that the host allocator can place heap storage outside the
    /// 32-bit immediate-character address range.
    ///
    /// # Errors
    ///
    /// Returns [`StorageCondition::HeapAddressTooLow`] when the allocator
    /// returns an address in the immediate-value range.
    pub fn validate_address_space() -> Result<(), StorageCondition> {
        let probe = Box::new([0_u64; 1]);
        (probe.as_ptr() as usize >= MIN_HEAP_ADDRESS)
            .then_some(())
            .ok_or(StorageCondition::HeapAddressTooLow)
    }
    /// Return the monotonically increasing collection epoch.
    #[must_use]
    pub fn gc_epoch(&self) -> u64 {
        self.lock_state().gc_epoch
    }
    /// Reject forwarded from-space words passed through mutator accessors.
    pub fn set_strict_forwarding(&self, on: bool) {
        self.strict_forwarding.store(on, Ordering::Relaxed);
    }
    /// Resolve a heap word through forwarding for registry lookups.
    #[must_use]
    pub fn forwarded_word(&self, value: Word) -> Option<Word> {
        let state = self.lock_state();
        let index = Self::find(&state, value)?;
        let object = state.objects.get(index)?;
        let tag = match value.lowtag() {
            x if x == crate::LowTag::List as u8 => crate::LowTag::List,
            x if x == crate::LowTag::Function as u8 => crate::LowTag::Function,
            x if x == crate::LowTag::Instance as u8 => crate::LowTag::Instance,
            x if x == crate::LowTag::OtherPointer as u8 => crate::LowTag::OtherPointer,
            _ => return None,
        };
        let address = object.words.as_ptr() as usize;
        drop(state);
        Some(Word::pointer(address, tag))
    }
    /// # Errors
    /// Returns `LayoutError::AlreadyRegistered` for an existing widetag or
    /// `LayoutError::MetadataCapacityExceeded` when the configured limit cannot reserve it.
    pub fn register_layout(&self, widetag: u8, layout: ReferenceLayout) -> Result<(), LayoutError> {
        let mut state = self.lock_state();
        if state.layouts.contains_key(&widetag) {
            return Err(LayoutError::AlreadyRegistered);
        }
        let nested_bytes = Layout::array::<usize>(layout.reference_words.capacity())
            .map_err(|_| LayoutError::MetadataCapacityExceeded)?
            .size();
        let charge = self
            .metadata_budget
            .reserve(nested_bytes)
            .map_err(|_| LayoutError::MetadataCapacityExceeded)?;
        state
            .layouts
            .insert(
                widetag,
                LayoutEntry {
                    layout,
                    _charge: charge,
                },
            )
            .map(|_| ())
            .map_err(|_| LayoutError::MetadataCapacityExceeded)
    }
    pub(crate) fn register_thread(&self, thread: &mut Thread) -> Result<(), StorageCondition> {
        let mut stop_world = self
            .stop_world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while stop_world.requested {
            stop_world = self
                .stop_world_ready
                .wait(stop_world)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        let mut state = self.lock_state();
        let pointer = std::ptr::from_mut(thread);
        if state.threads.contains(&pointer) {
            return Err(StorageCondition::ThreadNotRegistered);
        }
        let max_waiters = state
            .threads
            .len()
            .checked_add(1)
            .ok_or(StorageCondition::HeapMetadataCapacityExceeded)?;
        state
            .threads
            .reserve(1)
            .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)?;
        let parked_additional = max_waiters.saturating_sub(stop_world.parked.len());
        let gate_parked_additional = max_waiters.saturating_sub(stop_world.gate_parked.len());
        stop_world
            .parked
            .reserve(parked_additional)
            .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)?;
        stop_world
            .gate_parked
            .reserve(gate_parked_additional)
            .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)?;
        if thread.native_state() == crate::NativeState::Native {
            // Native threads are outside the mutator quorum from registration onward.
            thread.publish_snapshot();
        }
        state
            .threads
            .push(pointer)
            .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)?;
        thread.heap = Some(self);
        drop(state);
        drop(stop_world);
        Ok(())
    }
    /// Register published code metadata for precise native frame scanning.
    ///
    /// # Errors
    ///
    /// Returns `CodeError::NotPublished` when the code is not executable yet, or
    /// `CodeError::MetadataCapacityExceeded` when the metadata limit is reached.
    pub fn register_code(
        &self,
        code: &CodePtr,
        metadata: CodeObjectMetadata,
    ) -> Result<(), CodeError> {
        self.lock_state().code_registry.register(code, metadata)
    }
    pub(crate) fn safepoint_map_for_pc(&self, pc: usize) -> Option<crate::Safepoint> {
        {
            let state = self.lock_state();
            let (metadata, offset) = state.code_registry.find(pc)?;
            let map = metadata.safepoint_map.find_map(offset).cloned();
            drop(state);
            map
        }
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
        let _gate = self
            .acquire_slow_path_gate(thread)
            .map_err(map_storage_code_error)?;
        if let Err(error) = self.begin_collection(thread) {
            let _ = self.end_collection();
            return Err(map_storage_code_error(error));
        }
        let registered = {
            let state = self.lock_state();
            state.code_registry.find(code.address()).is_some()
        };
        if !registered {
            let _ = self.end_collection();
            return Err(CodeError::NotRegistered);
        }
        let live = {
            let state = self.lock_state();
            state.threads.iter().copied().any(|candidate| {
                // SAFETY: collection has stopped registered mutators.
                unsafe {
                    crate::walk_frame_headers(&(*candidate).frame_chain, 0, usize::MAX)
                        .iter()
                        .any(|header| {
                            let pc = header.return_pc;
                            pc >= code.address() && pc < code.address() + code.len()
                        })
                }
            })
        };
        if live {
            let _ = self.end_collection();
            return Err(CodeError::CodeInUse);
        }
        let code = owned.take().ok_or(CodeError::NotRegistered)?;
        let removed = self.lock_state().code_registry.unregister(&code);
        let end_result = self.end_collection();
        if removed.is_none() {
            *owned = Some(code);
            return Err(CodeError::NotRegistered);
        }
        drop(code);
        end_result.map_err(map_storage_code_error)
    }
    pub(crate) fn unregister_thread(&self, thread: &Thread) {
        let pointer = std::ptr::from_ref(thread).cast_mut();
        let mut stop_world = self
            .stop_world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        stop_world.parked.remove(&pointer.expose_provenance());
        stop_world.gate_parked.remove(&pointer.expose_provenance());
        drop(stop_world);

        let mut state = self.lock_state();
        state.threads.retain(|item| *item != pointer);
        drop(state);
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
    fn find_raw(state: &State, value: Word) -> Option<usize> {
        let address = value.address();
        let index = state.object_starts.get(&address).copied().or_else(|| {
            let forwarded_address = state.forwarding.get(&address)?;
            state.object_starts.get(forwarded_address).copied()
        })?;
        let object = state.objects.get(index)?;
        (object.alive || object.forwarded_to.is_some()).then_some(index)
    }
    fn find(state: &State, value: Word) -> Option<usize> {
        let mut index = Self::find_raw(state, value)?;
        while let Some(next) = state.objects[index].forwarded_to {
            index = next;
        }
        state.objects[index].alive.then_some(index)
    }
    fn find_for_mutator(&self, state: &State, value: Word) -> Option<usize> {
        let address = value.address();
        let points_to_forwarded_object = state.forwarding.contains_key(&address)
            || state
                .object_starts
                .get(&address)
                .and_then(|index| state.objects.get(*index))
                .is_some_and(|object| object.forwarded_to.is_some());
        if self.strict_forwarding.load(Ordering::Relaxed) && points_to_forwarded_object {
            return None;
        }
        let index = Self::find_raw(state, value)?;
        let mut index = index;
        while let Some(next) = state.objects[index].forwarded_to {
            index = next;
        }
        state.objects[index].alive.then_some(index)
    }
    fn find_conservative(state: &State, value: Word) -> Option<usize> {
        let index = Self::find_raw(state, value)?;
        let object = state.objects.get(index)?;
        let kind_matches = if value.is_list() {
            object.kind == PageKind::Cons
        } else if value.lowtag() == crate::LowTag::OtherPointer as u8 {
            matches!(object.kind, PageKind::HeaderObject | PageKind::Large)
        } else {
            false
        };
        (kind_matches && value.address() == object.words.as_ptr().addr()).then_some(index)
    }
    pub(crate) fn read_word(&self, object: Word, slot: usize) -> Option<Word> {
        let state = self.lock_state();
        state.objects[self.find_for_mutator(&state, object)?]
            .words
            .get(slot)
            .map(|value| Word::from_bits(*value))
    }
    pub(crate) fn write_word(&self, object: Word, slot: usize, value: Word) -> bool {
        self.write_word_at(object, slot + 1, value)
    }
    pub(crate) fn write_word_at(&self, object: Word, slot: usize, value: Word) -> bool {
        let mut state = self.lock_state();
        self.find_for_mutator(&state, object)
            .and_then(|index| state.objects[index].words.get_mut(slot))
            .map(|target| *target = value.bits())
            .is_some()
    }
    pub(crate) fn widetag(&self, object: Word) -> Option<u8> {
        let state = self.lock_state();
        let index = self.find_for_mutator(&state, object)?;
        let entry = state.objects.get(index)?;
        let header = (entry.kind != PageKind::Cons).then(|| entry.words.first().copied())??;
        drop(state);
        Some(u8::try_from(header & WIDETAG_MASK).unwrap_or(0))
    }
    fn write_words(&self, object: Word, values: &[(usize, Word)]) {
        let mut state = self.lock_state();
        if let Some(index) = self.find_for_mutator(&state, object) {
            for (slot, value) in values {
                if *slot < state.objects[index].words.len() {
                    state.objects[index].words[*slot] = value.bits();
                }
            }
        }
    }
    pub(crate) fn barrier(&self, object: Word, slot: usize) {
        let mut state = self.lock_state();
        if let Some(index) = self.find_for_mutator(&state, object)
            && state.objects[index].generation > 0
            && slot < state.objects[index].words.len()
        {
            if state.collection_preparing {
                // The scratch snapshot cannot grow after preflight; overflow makes
                // the minor collector scan all old objects and preserves this write.
                state.remembered_set_overflow = true;
                return;
            }
            let address = state.objects[index].words.as_ptr() as usize + slot * 8;
            if state
                .dirty_cards
                .insert((index, address / CARD_SIZE))
                .is_err()
            {
                state.remembered_set_overflow = true;
            }
        }
    }
    pub(crate) fn make_weak(&self, value: Word, weakness: Weakness) -> Word {
        let mut state = self.lock_state();
        if let Some(index) = self.find_for_mutator(&state, value) {
            state.objects[index].weak = Some(weakness);
        }
        value
    }
    pub(crate) fn weak_value(&self, value: Word) -> Word {
        let state = self.lock_state();
        self.find_for_mutator(&state, value)
            .and_then(|i| state.objects[i].words.get(1).copied())
            .map_or(Word::NIL, Word::from_bits)
    }
    pub(crate) fn register_finalizer(&self, object: Word, callback: Finalizer) {
        let mut state = self.lock_state();
        if let Some(index) = self.find_for_mutator(&state, object) {
            state.objects[index].finalizer = Some((callback, false));
        }
    }
    pub(crate) fn run_pending_finalizers(&self) {
        let mut state = self.lock_state();
        let pending = std::mem::replace(
            &mut state.finalizers,
            MetadataVec::new(self.metadata_budget.clone()),
        );
        drop(state);
        for (_, callback) in pending.iter().copied() {
            callback(Word::NIL);
        }
    }
    pub(crate) fn register_after_gc_hook(&self, hook: fn()) -> Result<(), StorageCondition> {
        self.lock_state()
            .after_gc_hooks
            .push(hook)
            .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)
    }
    fn lock_state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    fn relocated_address(
        state: &State,
        moved: &MetadataMap<usize, usize>,
        value: Word,
    ) -> Option<usize> {
        let fallback_address = value.address();
        let current_address = state
            .forwarding
            .get(&fallback_address)
            .copied()
            .unwrap_or(fallback_address);
        let index = Self::find_raw(state, value)?;
        let object = &state.objects[index];
        let object_address = object.words.as_ptr().addr();
        let new_base = moved.get(&object_address)?;
        Some(new_base + (current_address - object_address))
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
#[path = "heap/registry_tests.rs"]
mod registry_tests;
#[cfg(test)]
#[path = "heap/tests/slow_path.rs"]
mod slow_path_tests;
#[cfg(test)]
mod tests;
