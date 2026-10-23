use super::{Heap, LARGE_OBJECT, MIN_HEAP_ADDRESS, Object, PageKind, StorageCondition, TypeTag};
use crate::{Thread, Word};
use std::sync::{MutexGuard, TryLockError};

pub struct SlowPathGuard<'a> {
    heap: &'a Heap,
    guard: Option<MutexGuard<'a, ()>>,
}

pub(super) struct CoordinatedAllocation<'a> {
    pub(super) value: Word,
    _gate: Option<SlowPathGuard<'a>>,
}

impl Drop for SlowPathGuard<'_> {
    fn drop(&mut self) {
        drop(self.guard.take());
        let mut generation = self
            .heap
            .slow_path_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *generation = generation.wrapping_add(1);
        drop(generation);
        self.heap.slow_path_ready.notify_all();
    }
}

impl Heap {
    pub(crate) fn alloc(
        &self,
        thread: &mut Thread,
        tag: TypeTag,
        words: usize,
    ) -> Result<Word, StorageCondition> {
        let mut thread = thread.enter_lisp_state();
        self.allocate(
            &mut thread,
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
        let mut thread = thread.enter_lisp_state();
        self.allocate(
            &mut thread,
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
        let mut car = car;
        let mut cdr = cdr;
        let native_roots = if thread.native_state() == crate::NativeState::Native {
            // Keep native arguments visible to stack scanning while transition waits for collection.
            let first_slot = core::ptr::addr_of_mut!(car);
            let tail_slot = core::ptr::addr_of_mut!(cdr);
            // SAFETY: these pointers address the live local root slots until the reads below.
            unsafe {
                first_slot.write_volatile(car);
                tail_slot.write_volatile(cdr);
            }
            Some((first_slot, tail_slot))
        } else {
            None
        };
        let mut thread = thread.enter_lisp_state();
        if let Some((first_slot, tail_slot)) = native_roots {
            // SAFETY: both pointers still address the locals initialized before the transition.
            unsafe {
                car = first_slot.read_volatile();
                cdr = tail_slot.read_volatile();
            }
        }
        self.allocate_cons_with_coordinator(&mut thread, &mut car, &mut cdr)
    }

    pub(super) fn allocate_cons_with_coordinator(
        &self,
        thread: &mut Thread,
        car: &mut Word,
        cdr: &mut Word,
    ) -> Result<Word, StorageCondition> {
        let first_root = thread.push_root(car);
        let second_root = thread.push_root(cdr);
        let result =
            self.allocate_with_coordinator(thread, PageKind::Cons, TypeTag { widetag: 0 }, 2);
        match result {
            Ok(allocation) => {
                self.write_words(allocation.value, &[(0, *car), (1, *cdr)]);
                let value = allocation.value;
                drop(allocation);
                let _ = thread.pop_root(second_root);
                let _ = thread.pop_root(first_root);
                Ok(value)
            }
            Err(error) => {
                let _ = thread.pop_root(second_root);
                let _ = thread.pop_root(first_root);
                Err(error)
            }
        }
    }

    fn allocate(
        &self,
        thread: &mut Thread,
        kind: PageKind,
        tag: TypeTag,
        words: usize,
    ) -> Result<Word, StorageCondition> {
        self.allocate_with_coordinator(thread, kind, tag, words)
            .map(|allocation| allocation.value)
    }

    pub(super) fn allocate_with_coordinator(
        &self,
        thread: &mut Thread,
        kind: PageKind,
        tag: TypeTag,
        words: usize,
    ) -> Result<CoordinatedAllocation<'_>, StorageCondition> {
        let bytes = self.validate_allocation(thread, words)?;
        if !self.options.auto_gc {
            match self.allocate_once(thread, kind, tag, words) {
                Ok(value) => return Ok(CoordinatedAllocation { value, _gate: None }),
                Err(StorageCondition::CollectionInProgress) => {
                    let gate = self.acquire_slow_path_gate(thread)?;
                    let value = self.allocate_once(thread, kind, tag, words)?;
                    return Ok(CoordinatedAllocation {
                        value,
                        _gate: Some(gate),
                    });
                }
                Err(error) => return Err(error),
            }
        }
        if thread.native_state() == crate::NativeState::Lisp
            && thread
                .bytes_cons
                .checked_add(bytes)
                .is_some_and(|debt| debt <= self.config.bytes_considered_between_gcs)
        {
            match self.allocate_once(thread, kind, tag, words) {
                Ok(value) => return Ok(CoordinatedAllocation { value, _gate: None }),
                Err(
                    StorageCondition::CapacityExceeded
                    | StorageCondition::HeapMetadataCapacityExceeded
                    | StorageCondition::CollectionInProgress,
                ) => {}
                Err(error) => return Err(error),
            }
        }

        let gate = self.acquire_slow_path_gate(thread)?;
        let value = self.allocate_after_gate(thread, kind, tag, words, bytes)?;
        Ok(CoordinatedAllocation {
            value,
            _gate: Some(gate),
        })
    }

    fn validate_allocation(
        &self,
        thread: &Thread,
        words: usize,
    ) -> Result<usize, StorageCondition> {
        if words == 0 || words > (1 << 20) {
            return Err(StorageCondition::InvalidSize);
        }
        if thread.heap != Some(self) {
            return Err(StorageCondition::ThreadNotRegistered);
        }
        words.checked_mul(8).ok_or(StorageCondition::InvalidSize)
    }

    fn allocate_once(
        &self,
        thread: &mut Thread,
        kind: PageKind,
        tag: TypeTag,
        words: usize,
    ) -> Result<Word, StorageCondition> {
        let bytes = words.checked_mul(8).ok_or(StorageCondition::InvalidSize)?;
        let mut state = self.lock_state();
        if state.collection_preparing {
            return Err(StorageCondition::CollectionInProgress);
        }
        if state
            .used
            .checked_add(bytes)
            .is_none_or(|size| size > self.config.dynamic_space_size)
        {
            return Err(StorageCondition::CapacityExceeded);
        }
        state
            .objects
            .reserve(1)
            .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)?;
        state
            .object_starts
            .reserve(1)
            .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)?;
        let mut data = vec![0_u64; words].into_boxed_slice();
        if kind != PageKind::Cons {
            let Some(first) = data.first_mut() else {
                return Err(StorageCondition::InvalidSize);
            };
            *first = u64::from(tag.widetag);
        }
        let address = data.as_ptr().addr();
        if address < MIN_HEAP_ADDRESS {
            return Err(StorageCondition::HeapAddressTooLow);
        }
        let index = state.objects.len();
        state
            .object_starts
            .insert(address, index)
            .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)?;
        let object = Object {
            words: data,
            kind,
            generation: if bytes >= LARGE_OBJECT { 2 } else { 0 },
            survived: 0,
            pinned: false,
            weak: None,
            finalizer: None,
            alive: true,
            forwarded_to: None,
        };
        if state.objects.push(object).is_err() {
            state.object_starts.remove(&address);
            return Err(StorageCondition::HeapMetadataCapacityExceeded);
        }
        state.used += bytes;
        state.forwarding.remove(&address);
        drop(state);
        thread.bytes_cons = thread.bytes_cons.saturating_add(bytes);
        Ok(Word::pointer(
            address,
            if kind == PageKind::Cons {
                crate::LowTag::List
            } else {
                crate::LowTag::OtherPointer
            },
        ))
    }

    fn allocate_after_gate(
        &self,
        thread: &mut Thread,
        kind: PageKind,
        tag: TypeTag,
        words: usize,
        bytes: usize,
    ) -> Result<Word, StorageCondition> {
        if !self.options.auto_gc {
            return self.allocate_once(thread, kind, tag, words);
        }
        let mut minor_collected = false;
        let mut full_collected = false;

        if thread
            .bytes_cons
            .checked_add(bytes)
            .is_none_or(|debt| debt > self.config.bytes_considered_between_gcs)
        {
            self.collect_with_gate(thread, false)?;
            minor_collected = true;
        }

        loop {
            match self.allocate_once(thread, kind, tag, words) {
                Ok(value) => return Ok(value),
                Err(
                    StorageCondition::CapacityExceeded
                    | StorageCondition::HeapMetadataCapacityExceeded,
                ) if !minor_collected => {
                    self.collect_with_gate(thread, false)?;
                    minor_collected = true;
                }
                Err(
                    StorageCondition::CapacityExceeded
                    | StorageCondition::HeapMetadataCapacityExceeded,
                ) if !full_collected => {
                    self.collect_with_gate(thread, true)?;
                    full_collected = true;
                }
                Err(error) => return Err(error),
            }
        }
    }

    pub(crate) fn acquire_slow_path_gate(
        &self,
        thread: &mut Thread,
    ) -> Result<SlowPathGuard<'_>, StorageCondition> {
        #[cfg(target_arch = "x86_64")]
        if thread.native_state() != crate::NativeState::Native
            && !thread.has_native_frame_snapshot()
        {
            thread.publish_snapshot();
        }
        let pointer = std::ptr::from_mut(thread).expose_provenance();
        loop {
            let observed_generation = *self
                .slow_path_generation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            match self.slow_path_gate.try_lock() {
                Ok(guard) => {
                    #[cfg(target_arch = "x86_64")]
                    if thread.native_state() == crate::NativeState::Native {
                        thread.publish_snapshot();
                    }
                    self.unpark_gate_waiter(thread, pointer);
                    return Ok(SlowPathGuard {
                        heap: self,
                        guard: Some(guard),
                    });
                }
                Err(TryLockError::Poisoned(error)) => {
                    #[cfg(target_arch = "x86_64")]
                    if thread.native_state() == crate::NativeState::Native {
                        thread.publish_snapshot();
                    }
                    self.unpark_gate_waiter(thread, pointer);
                    return Ok(SlowPathGuard {
                        heap: self,
                        guard: Some(error.into_inner()),
                    });
                }
                Err(TryLockError::WouldBlock) => {
                    self.park_gate_waiter(thread, pointer)?;
                    let mut generation = self
                        .slow_path_generation
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    while *generation == observed_generation {
                        generation = self
                            .slow_path_ready
                            .wait(generation)
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                    }
                }
            }
        }
    }

    fn park_gate_waiter(
        &self,
        thread: &mut Thread,
        pointer: usize,
    ) -> Result<(), StorageCondition> {
        let mut stop_world = self
            .stop_world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        stop_world
            .gate_parked
            .insert(pointer)
            .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)?;
        if thread.native_state() != crate::NativeState::Native {
            let previous_state = thread.state;
            thread.state = crate::SafepointState::Parked;
            if stop_world.parked.insert(pointer).is_err() {
                stop_world.gate_parked.remove(&pointer);
                thread.state = previous_state;
                return Err(StorageCondition::HeapMetadataCapacityExceeded);
            }
        }
        drop(stop_world);
        self.stop_world_ready.notify_all();
        Ok(())
    }

    fn unpark_gate_waiter(&self, thread: &mut Thread, pointer: usize) {
        let mut stop_world = self
            .stop_world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        stop_world.gate_parked.remove(&pointer);
        stop_world.parked.remove(&pointer);
        if thread.state == crate::SafepointState::Parked {
            thread.state = if thread.native_state() == crate::NativeState::Native {
                crate::SafepointState::Safe
            } else if thread.safepoint_request == 0 {
                crate::SafepointState::Running
            } else {
                crate::SafepointState::PollRequested
            };
        }
    }

    pub(crate) fn reset_allocation_debt(&self) {
        let state = self.lock_state();
        for thread in state.threads.iter().copied() {
            // SAFETY: callers hold the stop-the-world phase until every debt is reset.
            unsafe { (*thread).bytes_cons = 0 };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Heap;
    use crate::{HeapConfig, SafepointState, StorageCondition, Thread};

    #[test]
    fn failed_gate_park_restores_thread_state_and_gate_set() {
        const METADATA_LIMIT: usize = 64 * 1024;

        let heap = Heap::with_options(
            HeapConfig::default(),
            crate::HeapOptions {
                max_metadata_bytes: Some(METADATA_LIMIT),
                ..crate::HeapOptions::default()
            },
        );
        let mut thread = Thread::new();
        assert_eq!(heap.register_thread(&mut thread), Ok(()));
        thread.state = SafepointState::PollRequested;
        let pointer = std::ptr::from_mut(&mut thread).expose_provenance();

        {
            let mut stop_world = heap
                .stop_world
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for value in 1..=6 {
                assert!(stop_world.parked.insert(value).is_ok());
            }
            for value in 101..=105 {
                assert!(stop_world.gate_parked.insert(value).is_ok());
            }
            drop(stop_world);
        }

        let remaining = METADATA_LIMIT.saturating_sub(heap.metadata_budget.used_bytes());
        let _budget_fill = heap
            .metadata_budget
            .reserve(remaining)
            .unwrap_or_else(|error| {
                panic!("remaining test budget should be reservable: {error:?}")
            });

        assert_eq!(
            heap.park_gate_waiter(&mut thread, pointer),
            Err(StorageCondition::HeapMetadataCapacityExceeded)
        );
        assert_eq!(thread.state, SafepointState::PollRequested);
        let stop_world = heap
            .stop_world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert!(!stop_world.gate_parked.contains(&pointer));
        assert!(!stop_world.parked.contains(&pointer));
        drop(stop_world);

        heap.unregister_thread(&thread);
    }
}
