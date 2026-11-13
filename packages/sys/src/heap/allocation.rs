use super::{Heap, LARGE_OBJECT, PageKind, StorageCondition, TypeTag};
use crate::{Thread, Word};
use std::sync::atomic::Ordering;
use std::sync::{MutexGuard, TryLockError};
#[path = "allocation/large.rs"]
mod large;
#[path = "allocation/tlab.rs"]
mod tlab;
use tlab::TlabAttempt;

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
        // Pair waiter removal with generation changes so no new collection can
        // count a woken gate waiter before it republishes its snapshot.
        let mut generation = self
            .heap
            .slow_path_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if self.heap.slow_path_gate_waiters.load(Ordering::Relaxed) != 0 {
            let mut stop_world = self
                .heap
                .stop_world
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let crate::stw::StopWorld {
                parked,
                gate_parked,
                ..
            } = &mut *stop_world;
            for pointer in gate_parked.iter() {
                parked.remove(pointer);
            }
            gate_parked.clear();
            self.heap.slow_path_gate_waiters.store(0, Ordering::Relaxed);
        }
        *generation = generation.wrapping_add(1);
        drop(self.guard.take());
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
        let mut thread = thread.enter_lisp_state();
        self.allocate_cons_with_coordinator(&mut thread, &mut car, &mut cdr)
    }

    pub(super) fn allocate_cons_with_coordinator(
        &self,
        thread: &mut Thread,
        car: &mut Word,
        cdr: &mut Word,
    ) -> Result<Word, StorageCondition> {
        self.allocate_cons_with_coordinator_inner(thread, car, cdr)
    }

    fn allocate_cons_with_coordinator_inner(
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
        if !self.pending_page_error.load(Ordering::Acquire)
            && thread.native_state() == crate::NativeState::Lisp
            && thread
                .bytes_cons
                .checked_add(bytes)
                .is_some_and(|debt| debt <= self.config.bytes_considered_between_gcs)
        {
            match self.allocate_fast(thread, kind, tag, words, false) {
                Ok(TlabAttempt::Allocated(value)) => {
                    return Ok(CoordinatedAllocation { value, _gate: None });
                }
                Ok(TlabAttempt::NeedsRefill | TlabAttempt::LogFull)
                | Err(StorageCondition::CapacityExceeded) => {}
                Err(error) => return Err(error),
            }
        }

        let gate = self.acquire_slow_path_gate(thread);
        if self.pending_page_error.load(Ordering::Acquire)
            && self.take_pending_page_error().is_some()
        {
            return Err(StorageCondition::PageOperationFailed);
        }
        let allocation = self.allocate_after_gate(thread, kind, tag, words, bytes);
        if self.pending_page_error.load(Ordering::Acquire)
            && self.take_pending_page_error().is_some()
        {
            return Err(StorageCondition::PageOperationFailed);
        }
        let value = allocation?;
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
            self.collect_for_allocation(thread, false)?;
            minor_collected = true;
        }

        loop {
            if kind == PageKind::Large || bytes >= LARGE_OBJECT {
                match self.allocate_large_once(thread, words, bytes, tag) {
                    Ok(value) => return Ok(value),
                    Err(StorageCondition::CapacityExceeded) if !minor_collected => {
                        self.collect_for_allocation(thread, false)?;
                        minor_collected = true;
                        continue;
                    }
                    Err(StorageCondition::CapacityExceeded) if !full_collected => {
                        self.collect_for_allocation(thread, true)?;
                        full_collected = true;
                        continue;
                    }
                    Err(error) => return Err(error),
                }
            }
            // The gate prevents another allocation-triggered collection while a native
            // thread with a stable Safe snapshot consumes its TLAB.
            match self.allocate_fast(thread, kind, tag, words, true) {
                Ok(TlabAttempt::Allocated(value)) => return Ok(value),
                Ok(TlabAttempt::LogFull) => {
                    let log_recycled = {
                        let _state = self.lock_state();
                        if self.pending_page_error.load(Ordering::Acquire)
                            || thread.reconciled_allocation_count()
                                != thread.pending_allocation_count()
                        {
                            false
                        } else {
                            // Keep the state lock until reset so another flush cannot mark the
                            // old published count reconciled after this log wraps to zero.
                            thread.reset_allocation_log();
                            true
                        }
                    };
                    if log_recycled {
                        continue;
                    }
                    if !minor_collected {
                        self.collect_for_allocation(thread, false)?;
                        minor_collected = true;
                        continue;
                    }
                    if !full_collected {
                        self.collect_for_allocation(thread, true)?;
                        full_collected = true;
                        continue;
                    }
                    return Err(StorageCondition::CapacityExceeded);
                }
                Ok(TlabAttempt::NeedsRefill) => match self.refill_tlab(thread, kind) {
                    Ok(()) => (),
                    Err(StorageCondition::CapacityExceeded) if !minor_collected => {
                        self.collect_for_allocation(thread, false)?;
                        minor_collected = true;
                    }
                    Err(StorageCondition::CapacityExceeded) if !full_collected => {
                        self.collect_for_allocation(thread, true)?;
                        full_collected = true;
                    }
                    Err(error) => return Err(error),
                },
                Err(StorageCondition::CapacityExceeded) if !minor_collected => {
                    self.collect_for_allocation(thread, false)?;
                    minor_collected = true;
                }
                Err(StorageCondition::CapacityExceeded) if !full_collected => {
                    self.collect_for_allocation(thread, true)?;
                    full_collected = true;
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn collect_for_allocation(
        &self,
        thread: &mut Thread,
        full: bool,
    ) -> Result<(), StorageCondition> {
        if self.pending_page_error.load(Ordering::Acquire)
            && self.take_pending_page_error().is_some()
        {
            return Err(StorageCondition::PageOperationFailed);
        }
        match self.collect_with_gate(thread, full) {
            Ok(()) => Ok(()),
            Err(_page_error) => {
                let _ = self.take_pending_page_error();
                Err(StorageCondition::PageOperationFailed)
            }
        }
    }

    pub(crate) fn acquire_slow_path_gate(&self, thread: &mut Thread) -> SlowPathGuard<'_> {
        let pointer = std::ptr::from_mut(thread).expose_provenance();
        loop {
            let mut generation = self
                .slow_path_generation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let observed_generation = *generation;
            match self.slow_path_gate.try_lock() {
                Ok(guard) => {
                    drop(generation);
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
                    // Serialize waiter publication with gate release so a waiter cannot
                    // join the parked set after the previous generation clears it.
                    self.park_gate_waiter(thread, pointer);
                    while *generation == observed_generation {
                        generation = self
                            .slow_path_ready
                            .wait(generation)
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                    }
                    self.unpark_gate_waiter(thread, pointer);
                }
            }
        }
    }

    fn park_gate_waiter(&self, thread: &mut Thread, pointer: usize) {
        let mut stop_world = self
            .stop_world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if stop_world.gate_parked.insert(pointer) {
            self.slow_path_gate_waiters.fetch_add(1, Ordering::Relaxed);
        }
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
        if stop_world.gate_parked.remove(&pointer) {
            self.slow_path_gate_waiters.fetch_sub(1, Ordering::Relaxed);
        }
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
