use crate::heap::metadata::MetadataBudget;
use crate::heap::metadata_map::MetadataSet;
use crate::{Heap, NativeState, SafepointState, Thread};

#[derive(Debug)]
pub struct StopWorld {
    pub(crate) epoch: u64,
    pub(crate) requested: bool,
    pub(crate) parked: MetadataSet<usize>,
    pub(crate) gate_parked: MetadataSet<usize>,
    pub(crate) failure: Option<crate::StorageCondition>,
}

impl StopWorld {
    pub(crate) fn new(budget: MetadataBudget) -> Self {
        Self {
            epoch: 0,
            requested: false,
            parked: MetadataSet::new(budget.clone()),
            gate_parked: MetadataSet::new(budget),
            failure: None,
        }
    }

    fn sync_parked_gate_waiters(&mut self) -> Result<(), crate::StorageCondition> {
        self.parked.retain(|_| false);
        for pointer in self.gate_parked.iter().copied() {
            let thread = std::ptr::with_exposed_provenance::<Thread>(pointer);
            // SAFETY: callers hold the stop-world lock, keeping registered threads alive during this read.
            let is_mutator = unsafe { (&*thread).native_state() != NativeState::Native };
            if is_mutator {
                self.parked
                    .insert(pointer)
                    .map_err(|_| crate::StorageCondition::HeapMetadataCapacityExceeded)?;
            }
        }
        Ok(())
    }
}

struct CollectionCleanup<'a> {
    heap: &'a Heap,
    thread: *mut Thread,
    stop_world_started: bool,
    active: bool,
}

impl<'a> CollectionCleanup<'a> {
    const fn new(heap: &'a Heap, thread: &mut Thread) -> Self {
        Self {
            heap,
            thread,
            stop_world_started: false,
            active: true,
        }
    }

    const fn mark_stop_world_started(&mut self) {
        self.stop_world_started = true;
    }

    fn finish(mut self) -> Result<(), crate::StorageCondition> {
        let result = self.run();
        self.active = false;
        result
    }

    fn run(&mut self) -> Result<(), crate::StorageCondition> {
        let mut end_result = Ok(());
        if self.stop_world_started {
            end_result = self.heap.end_collection();
            let _stop_world = self
                .heap
                .stop_world
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            // SAFETY: the collection call borrows the Thread for at least this guard's lifetime.
            let thread = unsafe { &mut *self.thread };
            thread.state = if thread.native_state() == NativeState::Native {
                SafepointState::Safe
            } else if thread.safepoint_request == 0 {
                SafepointState::Running
            } else {
                SafepointState::PollRequested
            };
            self.stop_world_started = false;
        }
        self.heap.finish_collection_preparation();
        end_result
    }
}

impl Drop for CollectionCleanup<'_> {
    fn drop(&mut self) {
        if self.active {
            let _ = self.run();
        }
    }
}

impl Heap {
    pub(crate) fn request_epoch(&self) -> Result<(), crate::StorageCondition> {
        let mut stop_world = self
            .stop_world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !stop_world.requested {
            stop_world.epoch = stop_world.epoch.wrapping_add(1).max(1);
            stop_world.requested = true;
            stop_world.failure = None;
            if let Err(error) = stop_world.sync_parked_gate_waiters() {
                stop_world.failure = Some(error);
                stop_world.requested = false;
                drop(stop_world);
                self.stop_world_ready.notify_all();
                return Err(error);
            }
        }
        if let Some(error) = stop_world.failure {
            drop(stop_world);
            self.stop_world_ready.notify_all();
            return Err(error);
        }
        drop(stop_world);
        self.stop_world_ready.notify_all();
        Ok(())
    }

    pub(crate) fn poll_thread(&self, thread: &mut Thread) -> Result<(), crate::StorageCondition> {
        let pointer = std::ptr::from_mut(thread).expose_provenance();
        let mut stop_world = self
            .stop_world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        loop {
            if !stop_world.requested {
                thread.state = SafepointState::Running;
                drop(stop_world);
                return Ok(());
            }
            let epoch = stop_world.epoch;
            thread.safepoint_epoch = epoch;
            thread.state = SafepointState::Published;
            if stop_world.parked.insert(pointer).is_err() {
                let error = crate::StorageCondition::HeapMetadataCapacityExceeded;
                stop_world.failure = Some(error);
                thread.state = SafepointState::PollRequested;
                self.stop_world_ready.notify_all();
                while stop_world.requested && stop_world.epoch == epoch {
                    stop_world = self
                        .stop_world_ready
                        .wait(stop_world)
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                }
                drop(stop_world);
                return Err(error);
            }
            self.stop_world_ready.notify_all();
            while stop_world.requested && stop_world.epoch == epoch {
                stop_world = self
                    .stop_world_ready
                    .wait(stop_world)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
            if let Some(error) = stop_world.failure {
                thread.state = SafepointState::PollRequested;
                drop(stop_world);
                return Err(error);
            }
            drop(stop_world);
            stop_world = self
                .stop_world
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    pub(crate) fn begin_collection(
        &self,
        thread: &mut Thread,
    ) -> Result<(), crate::StorageCondition> {
        self.request_epoch()?;
        let mut stop_world = self
            .stop_world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if thread.native_state() != crate::NativeState::Native
            && stop_world
                .parked
                .insert(std::ptr::from_mut(thread).expose_provenance())
                .is_err()
        {
            let error = crate::StorageCondition::HeapMetadataCapacityExceeded;
            stop_world.failure = Some(error);
            drop(stop_world);
            self.stop_world_ready.notify_all();
            return Err(error);
        }
        while stop_world.parked.len() < self.active_mutators() && stop_world.failure.is_none() {
            stop_world = self
                .stop_world_ready
                .wait(stop_world)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        if let Some(error) = stop_world.failure {
            return Err(error);
        }
        drop(stop_world);
        Ok(())
    }

    pub(crate) fn end_collection(&self) -> Result<(), crate::StorageCondition> {
        let mut stop_world = self
            .stop_world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        stop_world.requested = false;
        stop_world.failure = None;
        let result = stop_world.sync_parked_gate_waiters();
        if let Err(error) = result {
            stop_world.failure = Some(error);
        }
        drop(stop_world);
        self.stop_world_ready.notify_all();
        result
    }

    pub(crate) fn collect_with_thread(
        &self,
        thread: &mut Thread,
        full: bool,
    ) -> Result<(), crate::StorageCondition> {
        let _gate = self.acquire_slow_path_gate(thread)?;
        self.collect_with_gate(thread, full)
    }

    pub(crate) fn collect_with_gate(
        &self,
        thread: &mut Thread,
        full: bool,
    ) -> Result<(), crate::StorageCondition> {
        let mut scratch = self.prepare_collection()?;
        let mut cleanup = CollectionCleanup::new(self, thread);
        #[cfg(target_arch = "x86_64")]
        if !thread.has_native_frame_snapshot() {
            thread.publish_snapshot();
        }
        {
            let stop_world = self
                .stop_world
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            thread.state = SafepointState::Collector;
            drop(stop_world);
        }
        cleanup.mark_stop_world_started();
        let collection_result = self.begin_collection(thread).and_then(|()| {
            let stop_world = self
                .stop_world
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            thread.state = SafepointState::Collecting;
            drop(stop_world);
            self.collect_prepared(full, &mut scratch)
        });
        if collection_result.is_ok() {
            self.reset_allocation_debt();
        }
        let cleanup_result = cleanup.finish();
        drop(scratch);
        let hook_count = match (collection_result, cleanup_result) {
            (Ok(hook_count), Ok(())) => hook_count,
            (Err(error), _) | (_, Err(error)) => return Err(error),
        };
        self.run_after_gc_hooks(hook_count);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Heap;
    use crate::{HeapConfig, Thread};
    use std::sync::{Arc, Barrier, mpsc};
    use std::thread;

    #[test]
    fn poll_thread_rejoins_when_a_new_epoch_starts_before_wakeup() {
        let heap = Arc::new(Heap::new(HeapConfig::default()));
        let (pointer_sender, pointer_receiver) = mpsc::channel();
        let ready = Arc::new(Barrier::new(2));
        let worker_heap = Arc::clone(&heap);
        let worker_ready = Arc::clone(&ready);
        let worker = thread::spawn(move || {
            let mut thread = Thread::new();
            assert_eq!(worker_heap.register_thread(&mut thread), Ok(()));
            pointer_sender
                .send(std::ptr::from_mut(&mut thread) as usize)
                .ok();
            worker_ready.wait();
            assert_eq!(worker_heap.poll_thread(&mut thread), Ok(()));
            worker_heap.unregister_thread(&thread);
        });
        let pointer = pointer_receiver.recv().unwrap_or(0);
        {
            let mut stop_world = heap
                .stop_world
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            stop_world.requested = true;
            stop_world.epoch = 1;
        }
        ready.wait();
        heap.stop_world_ready.notify_all();

        let mut stop_world = heap
            .stop_world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while !stop_world.parked.contains(&pointer) {
            stop_world = heap
                .stop_world_ready
                .wait(stop_world)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        stop_world.requested = false;
        stop_world.parked.clear();
        stop_world.epoch = 2;
        stop_world.requested = true;
        drop(stop_world);
        heap.stop_world_ready.notify_all();

        let mut stop_world = heap
            .stop_world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while !stop_world.parked.contains(&pointer) {
            stop_world = heap
                .stop_world_ready
                .wait(stop_world)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        stop_world.requested = false;
        drop(stop_world);
        heap.stop_world_ready.notify_all();
        assert!(worker.join().is_ok());
    }

    #[test]
    fn native_collector_waits_for_running_mutator_to_poll() {
        let heap = Arc::new(Heap::new(HeapConfig::default()));
        let (mutator_ready_sender, mutator_ready_receiver) = mpsc::channel();
        let (poll_sender, poll_receiver) = mpsc::channel();
        let worker_heap = Arc::clone(&heap);
        let mutator = thread::spawn(move || {
            let mut worker = Thread::new();
            assert_eq!(worker_heap.register_thread(&mut worker), Ok(()));
            assert!(mutator_ready_sender.send(()).is_ok());
            assert!(poll_receiver.recv().is_ok());
            worker.poll_safepoint();
            worker_heap.unregister_thread(&worker);
        });
        assert!(mutator_ready_receiver.recv().is_ok());

        let collector_heap = Arc::clone(&heap);
        let (collector_ready_sender, collector_ready_receiver) = mpsc::channel();
        let (collection_done_sender, collection_done_receiver) = mpsc::channel();
        let collector = thread::spawn(move || {
            let mut collector = Thread::new();
            assert_eq!(collector_heap.register_thread(&mut collector), Ok(()));
            collector.enter_native();
            assert!(collector_ready_sender.send(()).is_ok());
            collector_heap
                .collect_with_thread(&mut collector, true)
                .unwrap_or_else(|error| panic!("test heap has metadata for collection: {error:?}"));
            assert!(collection_done_sender.send(()).is_ok());
            collector_heap.unregister_thread(&collector);
        });
        assert!(collector_ready_receiver.recv().is_ok());

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut stop_world = heap
            .stop_world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while !stop_world.requested {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            assert!(
                !remaining.is_zero(),
                "collector did not request a safepoint"
            );
            let (next, wait) = heap
                .stop_world_ready
                .wait_timeout(stop_world, remaining)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            stop_world = next;
            assert!(!wait.timed_out() || stop_world.requested);
        }
        drop(stop_world);

        assert!(matches!(
            collection_done_receiver.recv_timeout(std::time::Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        assert!(poll_sender.send(()).is_ok());
        assert!(
            collection_done_receiver
                .recv_timeout(std::time::Duration::from_secs(5))
                .is_ok()
        );
        assert!(mutator.join().is_ok());
        assert!(collector.join().is_ok());
    }
}
