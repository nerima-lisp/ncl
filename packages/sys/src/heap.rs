use crate::heap_state::{HeapWords, MIN_OBJECTS_BEFORE_COMPACTION, Object, PageMetadata, State};
pub use crate::heap_types::{
    Finalizer, HeapConfig, LayoutError, PageKind, ReferenceLayout, StorageCondition, TypeTag,
    Weakness,
};
use crate::{CodeError, CodeObjectMetadata, CodePtr, Thread, Word};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};
#[path = "heap/allocation.rs"]
mod allocation;
#[path = "heap/collect.rs"]
mod collect;
#[path = "heap/lookup.rs"]
mod lookup;
#[path = "heap/pages.rs"]
mod pages;
pub use pages::{LOGICAL_PAGE_SIZE, PageError, PageRange};
#[path = "heap/relocation.rs"]
mod relocation;
#[path = "heap/roots.rs"]
mod roots;
#[path = "heap/scan.rs"]
mod scan;
#[cfg(test)]
use lookup::SIMPLE_FUN_WIDETAG;
pub use roots::HeapRootError;
const CARD_SIZE: usize = 512;
const LARGE_OBJECT: usize = 8 * 1024;
const WIDETAG_MASK: u64 = 0xff;
const FORWARDED_FLAG: u64 = 1 << 10;
static NEXT_HEAP_ID: AtomicU64 = AtomicU64::new(1);
/// Heap objects must not occupy the address range used by immediate values.
pub const MIN_HEAP_ADDRESS: usize = 1_usize << 32;

#[derive(Debug)]
/// Moving heap and its stop-the-world coordination state.
pub struct Heap {
    root_namespace: u64,
    config: HeapConfig,
    state: Mutex<State>,
    available_bytes: AtomicUsize,
    pending_page_error: AtomicBool,
    strict_forwarding: AtomicBool,
    #[cfg(feature = "test-support")]
    registered_root_slots_live_and_forwarded_for_test: AtomicBool,
    slow_path_gate: Mutex<()>,
    slow_path_generation: Mutex<u64>,
    slow_path_ready: Condvar,
    pub(crate) stop_world: Mutex<crate::stw::StopWorld>,
    pub(crate) stop_world_ready: Condvar,
}
impl Default for HeapConfig {
    fn default() -> Self {
        Self {
            dynamic_space_size: 512 * 1024 * 1024,
            bytes_considered_between_gcs: 4 * 1024 * 1024,
        }
    }
}
impl Heap {
    #[must_use]
    /// Construct an empty heap with the supplied capacity policy.
    pub fn new(config: HeapConfig) -> Self {
        let reserve_size = config
            .dynamic_space_size
            .checked_add(pages::LOGICAL_PAGE_SIZE - 1)
            .map_or(0, |size| {
                size / pages::LOGICAL_PAGE_SIZE * pages::LOGICAL_PAGE_SIZE
            })
            .max(if config.dynamic_space_size < pages::LOGICAL_PAGE_SIZE {
                2 * pages::LOGICAL_PAGE_SIZE
            } else {
                pages::LOGICAL_PAGE_SIZE
            });
        let (page_range, page_error, mut pages) = match pages::PageRange::reserve(reserve_size) {
            Ok(range) => {
                let mut metadata = Vec::new();
                if metadata.try_reserve_exact(range.page_count()).is_err() {
                    (
                        None,
                        Some(pages::PageError::MetadataAllocationFailed),
                        Vec::new(),
                    )
                } else {
                    metadata.resize_with(range.page_count(), PageMetadata::default);
                    (Some(range), None, metadata)
                }
            }
            Err(error) => (None, Some(error), Vec::new()),
        };
        if page_range.is_none() {
            pages.clear();
        }
        Self {
            root_namespace: NEXT_HEAP_ID.fetch_add(1, Ordering::Relaxed),
            config,
            state: Mutex::new(State {
                used: 0,
                gc_epoch: 0,
                objects: Vec::new(),
                compact_at_object_count: MIN_OBJECTS_BEFORE_COMPACTION,
                page_range,
                page_error,
                pending_page_error: None,
                pages,
                layouts: HashMap::new(),
                threads: Vec::new(),
                dirty_cards: HashSet::new(),
                roots: Vec::new(),
                root_generations: Vec::new(),
                next_root_generation: 1,
                finalizers: Vec::new(),
                after_gc_hooks: Vec::new(),
                code_registry: crate::CodeRegistry::default(),
            }),
            available_bytes: AtomicUsize::new(config.dynamic_space_size),
            pending_page_error: AtomicBool::new(false),
            strict_forwarding: AtomicBool::new(false),
            #[cfg(feature = "test-support")]
            registered_root_slots_live_and_forwarded_for_test: AtomicBool::new(false),
            slow_path_gate: Mutex::new(()),
            slow_path_generation: Mutex::new(0),
            slow_path_ready: Condvar::new(),
            stop_world: Mutex::new(crate::stw::StopWorld::default()),
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
    /// Return the configured dynamic-space capacity in bytes.
    pub const fn dynamic_space_size(&self) -> usize {
        self.config.dynamic_space_size
    }
    /// Return the allocation debt threshold that triggers collection.
    pub const fn bytes_considered_between_gcs(&self) -> usize {
        self.config.bytes_considered_between_gcs
    }
    /// Return the monotonically increasing collection epoch.
    #[must_use]
    pub fn gc_epoch(&self) -> u64 {
        self.lock_state().gc_epoch
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn forwarded_object_count_for_test(&self) -> usize {
        self.lock_state()
            .objects
            .iter()
            .filter(|object| object.forwarded_to.is_some())
            .count()
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn registered_root_slots_are_live_and_forwarded_for_test(&self) -> bool {
        self.registered_root_slots_live_and_forwarded_for_test
            .load(Ordering::Relaxed)
    }
    /// Reject forwarded from-space words passed through mutator accessors.
    pub fn set_strict_forwarding(&self, on: bool) {
        self.strict_forwarding.store(on, Ordering::Relaxed);
    }
    pub(crate) fn is_forwarding_violation(&self, object: Word) -> bool {
        if !self.strict_forwarding.load(Ordering::Relaxed) {
            return false;
        }
        let state = self.lock_state();
        Self::find_raw(&state, object)
            .and_then(|index| state.objects.get(index))
            .is_some_and(|entry| entry.forwarded_to.is_some())
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
        if !thread.initialize_allocation_log() {
            return Err(StorageCondition::CapacityExceeded);
        }
        let mut state = self.lock_state();
        let pointer = std::ptr::from_mut(thread);
        if state.threads.contains(&pointer) {
            return Err(StorageCondition::ThreadNotRegistered);
        }
        if thread.native_state() == crate::NativeState::Native {
            // Native threads are outside the mutator quorum from registration onward.
            thread.publish_snapshot();
        }
        state.threads.push(pointer);
        thread.heap = Some(self);
        drop(state);
        drop(stop_world);
        Ok(())
    }
    /// Register code metadata for precise native frame scanning.
    ///
    /// Metadata for unpublished code remains prepared and hidden from PC lookup
    /// until the registry is activated after code publication. Metadata for
    /// already-published code becomes active during this call.
    ///
    /// # Errors
    ///
    /// Returns `CodeError::OutOfBounds` when metadata does not fit the allocation,
    /// or `CodeError::AlreadyRegistered` when metadata is already registered.
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
        let _gate = self.acquire_slow_path_gate(thread);
        self.begin_collection(thread);
        let registered = {
            let state = self.lock_state();
            state.code_registry.find(code.address()).is_some()
        };
        if !registered {
            self.end_collection();
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
            self.end_collection();
            return Err(CodeError::CodeInUse);
        }
        let code = owned.take().ok_or(CodeError::NotRegistered)?;
        let removed = self.lock_state().code_registry.unregister(&code);
        self.end_collection();
        if removed.is_none() {
            *owned = Some(code);
            return Err(CodeError::NotRegistered);
        }
        drop(code);
        Ok(())
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
        let owner = pointer.addr();
        for page in &mut state.pages {
            if page.owner == Some(owner) {
                page.owner = None;
            }
        }
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
            && let Some(entry) = state.objects.get(index)
            && entry.generation > 0
            && slot < entry.words.len()
            && let Some(address) = slot
                .checked_mul(8)
                .and_then(|offset| entry.start.checked_add(offset))
        {
            state.dirty_cards.insert((index, address / CARD_SIZE));
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
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Err(error) = state.flush_pending_allocations() {
            if state.pending_page_error.is_none() {
                state.pending_page_error = Some(error);
            }
            self.pending_page_error.store(true, Ordering::Release);
        }
        state
    }
    pub(crate) fn take_pending_page_error(&self) -> Option<pages::PageError> {
        let mut state = self.lock_state();
        let error = state.pending_page_error.take();
        self.pending_page_error.store(false, Ordering::Release);
        drop(state);
        error
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
