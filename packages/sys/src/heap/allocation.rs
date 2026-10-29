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
        if thread.native_state() == crate::NativeState::Native {
            return Err(StorageCondition::NativeAllocationRequiresRoot);
        }
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
        if thread.native_state() == crate::NativeState::Native {
            return Err(StorageCondition::NativeAllocationRequiresRoot);
        }
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
        if thread.native_state() == crate::NativeState::Native {
            return Err(StorageCondition::NativeAllocationRequiresRoot);
        }
        let mut car = car;
        let mut cdr = cdr;
        let mut thread = thread.enter_lisp_state();
        self.allocate_cons_with_coordinator(&mut thread, &mut car, &mut cdr)
    }

    /// Allocate a header object into a caller-owned heap root slot.
    ///
    /// The slot is registered before entering Lisp state and remains rooted on
    /// success. Keep it alive at the same address until its token is
    /// unregistered from this heap.
    ///
    /// # Safety
    /// `output` must remain valid at the same address until the returned token
    /// is removed. The caller must not access it while its thread is in Native
    /// state or while a collection may rewrite it.
    pub(crate) unsafe fn alloc_rooted(
        &self,
        thread: &mut Thread,
        tag: TypeTag,
        words: usize,
        output: &mut Word,
    ) -> Result<crate::RootToken, StorageCondition> {
        let words = words.checked_add(1).ok_or(StorageCondition::InvalidSize)?;
        self.allocate_into_root(thread, output, PageKind::HeaderObject, tag, words)
    }

    /// Allocate a large object into a caller-owned heap root slot.
    ///
    /// The slot is registered before entering Lisp state and remains rooted on
    /// success. Keep it alive at the same address until its token is
    /// unregistered from this heap.
    ///
    /// # Safety
    /// `output` must remain valid at the same address until the returned token
    /// is removed. The caller must not access it while its thread is in Native
    /// state or while a collection may rewrite it.
    pub(crate) unsafe fn alloc_large_rooted(
        &self,
        thread: &mut Thread,
        tag: TypeTag,
        words: usize,
        output: &mut Word,
    ) -> Result<crate::RootToken, StorageCondition> {
        let words = words.checked_add(1).ok_or(StorageCondition::InvalidSize)?;
        self.allocate_into_root(thread, output, PageKind::Large, tag, words)
    }

    /// Allocate a cons cell from caller-owned root slots into a caller-owned
    /// heap root slot.
    ///
    /// The output slot remains rooted on success. Keep it alive at the same
    /// address until its token is unregistered from this heap.
    ///
    /// # Safety
    /// `car` and `cdr` must be distinct, stable slots containing values already
    /// kept alive by collector-visible roots until this method registers them.
    /// In Native state, they must not refer only to unregistered Native locals.
    /// `output` must be distinct from both inputs, remain valid at the same
    /// address until its token is removed, and not be accessed by the caller
    /// while its thread is in Native state or while a collection may rewrite
    /// any of these slots.
    pub(crate) unsafe fn alloc_cons_rooted(
        &self,
        thread: &mut Thread,
        car: &mut Word,
        cdr: &mut Word,
        output: &mut Word,
    ) -> Result<crate::RootToken, StorageCondition> {
        self.alloc_cons_rooted_before_registration(thread, car, cdr, output, || {})
    }

    #[cfg(test)]
    /// Runs `before_first_root` while the caller is still Native and before
    /// this method registers any of its slots.
    ///
    /// # Safety
    /// The caller must uphold the same slot and root requirements as
    /// [`Heap::alloc_cons_rooted`].
    pub(crate) unsafe fn alloc_cons_rooted_with_test_hook(
        &self,
        thread: &mut Thread,
        car: &mut Word,
        cdr: &mut Word,
        output: &mut Word,
        before_first_root: impl FnOnce(),
    ) -> Result<crate::RootToken, StorageCondition> {
        self.alloc_cons_rooted_before_registration(thread, car, cdr, output, before_first_root)
    }

    fn alloc_cons_rooted_before_registration(
        &self,
        thread: &mut Thread,
        car: &mut Word,
        cdr: &mut Word,
        output: &mut Word,
        before_first_root: impl FnOnce(),
    ) -> Result<crate::RootToken, StorageCondition> {
        before_first_root();
        let output_root = self.push_root(output);
        let first_input_root = self.push_root(car);
        let second_input_root = self.push_root(cdr);
        let result = {
            let mut thread = thread.enter_lisp_state();
            self.allocate_cons_with_coordinator_into_root(&mut thread, car, cdr, output)
        };
        let _ = self.unregister_root(second_input_root);
        let _ = self.unregister_root(first_input_root);
        match result {
            Ok(_) => Ok(output_root),
            Err(error) => {
                let _ = self.unregister_root(output_root);
                Err(error)
            }
        }
    }

    fn allocate_into_root(
        &self,
        thread: &mut Thread,
        output: &mut Word,
        kind: PageKind,
        tag: TypeTag,
        words: usize,
    ) -> Result<crate::RootToken, StorageCondition> {
        let output_root = self.push_root(output);
        let result = {
            let mut thread = thread.enter_lisp_state();
            match self.allocate_with_coordinator(&mut thread, kind, tag, words) {
                Ok(allocation) => {
                    let value = allocation.value;
                    *output = value;
                    drop(allocation);
                    Ok(())
                }
                Err(error) => Err(error),
            }
        };
        match result {
            Ok(()) => Ok(output_root),
            Err(error) => {
                let _ = self.unregister_root(output_root);
                Err(error)
            }
        }
    }

    pub(super) fn allocate_cons_with_coordinator(
        &self,
        thread: &mut Thread,
        car: &mut Word,
        cdr: &mut Word,
    ) -> Result<Word, StorageCondition> {
        self.allocate_cons_with_coordinator_inner(thread, car, cdr, None)
    }

    fn allocate_cons_with_coordinator_into_root(
        &self,
        thread: &mut Thread,
        car: &mut Word,
        cdr: &mut Word,
        output: &mut Word,
    ) -> Result<Word, StorageCondition> {
        self.allocate_cons_with_coordinator_inner(thread, car, cdr, Some(output))
    }

    fn allocate_cons_with_coordinator_inner(
        &self,
        thread: &mut Thread,
        car: &mut Word,
        cdr: &mut Word,
        output: Option<&mut Word>,
    ) -> Result<Word, StorageCondition> {
        let first_root = thread.push_root(car);
        let second_root = thread.push_root(cdr);
        let result =
            self.allocate_with_coordinator(thread, PageKind::Cons, TypeTag { widetag: 0 }, 2);
        match result {
            Ok(allocation) => {
                self.write_words(allocation.value, &[(0, *car), (1, *cdr)]);
                let value = allocation.value;
                if let Some(output) = output {
                    *output = value;
                }
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
        if thread.native_state() == crate::NativeState::Lisp
            && thread
                .bytes_cons
                .checked_add(bytes)
                .is_some_and(|debt| debt <= self.config.bytes_considered_between_gcs)
        {
            match self.allocate_once(thread, kind, tag, words) {
                Ok(value) => return Ok(CoordinatedAllocation { value, _gate: None }),
                Err(StorageCondition::CapacityExceeded) => {}
                Err(error) => return Err(error),
            }
        }

        let gate = self.acquire_slow_path_gate(thread);
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
        if state
            .used
            .checked_add(bytes)
            .is_none_or(|size| size > self.config.dynamic_space_size)
        {
            return Err(StorageCondition::CapacityExceeded);
        }
        let mut data = vec![0_u64; words].into_boxed_slice();
        if kind != PageKind::Cons
            && let Some(first) = data.first_mut()
        {
            *first = u64::from(tag.widetag);
        }
        let address = data.as_ptr().addr();
        if address < MIN_HEAP_ADDRESS {
            return Err(StorageCondition::HeapAddressTooLow);
        }
        state.used += bytes;
        let index = state.objects.len();
        state.object_starts.insert(address, index);
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
        let mut minor_collected = false;
        let mut full_collected = false;

        if thread
            .bytes_cons
            .checked_add(bytes)
            .is_none_or(|debt| debt > self.config.bytes_considered_between_gcs)
        {
            self.collect_with_gate(thread, false);
            minor_collected = true;
        }

        loop {
            match self.allocate_once(thread, kind, tag, words) {
                Ok(value) => return Ok(value),
                Err(StorageCondition::CapacityExceeded) if !minor_collected => {
                    self.collect_with_gate(thread, false);
                    minor_collected = true;
                }
                Err(StorageCondition::CapacityExceeded) if !full_collected => {
                    self.collect_with_gate(thread, true);
                    full_collected = true;
                }
                Err(error) => return Err(error),
            }
        }
    }

    pub(crate) fn acquire_slow_path_gate(&self, thread: &mut Thread) -> SlowPathGuard<'_> {
        let pointer = std::ptr::from_mut(thread).expose_provenance();
        loop {
            let observed_generation = *self
                .slow_path_generation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            match self.slow_path_gate.try_lock() {
                Ok(guard) => {
                    self.unpark_gate_waiter(thread, pointer);
                    return SlowPathGuard {
                        heap: self,
                        guard: Some(guard),
                    };
                }
                Err(TryLockError::Poisoned(error)) => {
                    self.unpark_gate_waiter(thread, pointer);
                    return SlowPathGuard {
                        heap: self,
                        guard: Some(error.into_inner()),
                    };
                }
                Err(TryLockError::WouldBlock) => {
                    self.park_gate_waiter(thread, pointer);
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

    fn park_gate_waiter(&self, thread: &mut Thread, pointer: usize) {
        let mut stop_world = self
            .stop_world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        stop_world.gate_parked.insert(pointer);
        if thread.native_state() != crate::NativeState::Native {
            #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
            thread.publish_snapshot();
            thread.state = crate::SafepointState::Parked;
            stop_world.parked.insert(pointer);
        }
        drop(stop_world);
        self.stop_world_ready.notify_all();
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
