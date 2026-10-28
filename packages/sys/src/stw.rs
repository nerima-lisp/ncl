use crate::{Heap, NativeState, SafepointState, Thread};
use std::collections::HashSet;

#[cfg(target_arch = "x86_64")]
fn log_stack_diagnostic(test: &str, phase: &str, thread: &Thread, rsp: usize) {
    let (bounds, scan_start) = thread.diagnostic_stack_state();
    eprintln!(
        "NCL_GC_STACK_DIAG phase={phase} test={test} thread={:#x} rsp={rsp:#x} bounds={bounds:?} scan_start={scan_start:?}",
        std::ptr::from_ref(thread).expose_provenance(),
    );
}

#[derive(Debug, Default)]
pub struct StopWorld {
    pub(crate) epoch: u64,
    pub(crate) requested: bool,
    pub(crate) parked: HashSet<usize>,
    pub(crate) gate_parked: HashSet<usize>,
}

impl StopWorld {
    fn sync_parked_gate_waiters(&mut self) {
        self.parked.clone_from(&self.gate_parked);
        self.parked.retain(|pointer| {
            let thread = std::ptr::with_exposed_provenance::<Thread>(*pointer);
            // SAFETY: callers hold the stop-world lock, keeping registered threads alive during this read.
            unsafe { (&*thread).native_state() != NativeState::Native }
        });
    }
}

impl Heap {
    pub(crate) fn request_epoch(&self) {
        let mut stop_world = self
            .stop_world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !stop_world.requested {
            stop_world.epoch = stop_world.epoch.wrapping_add(1).max(1);
            stop_world.requested = true;
            stop_world.sync_parked_gate_waiters();
        }
        drop(stop_world);
        self.stop_world_ready.notify_all();
    }

    pub(crate) fn poll_thread(&self, thread: &mut Thread) {
        let pointer = std::ptr::from_mut(thread).expose_provenance();
        let mut stop_world = self
            .stop_world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        loop {
            if !stop_world.requested {
                thread.state = SafepointState::Running;
                drop(stop_world);
                return;
            }
            let epoch = stop_world.epoch;
            thread.safepoint_epoch = epoch;
            thread.state = SafepointState::Published;
            stop_world.parked.insert(pointer);
            self.stop_world_ready.notify_all();
            while stop_world.requested && stop_world.epoch == epoch {
                stop_world = self
                    .stop_world_ready
                    .wait(stop_world)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
            drop(stop_world);
            stop_world = self
                .stop_world
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    pub(crate) fn begin_collection(&self, thread: &mut Thread) {
        self.request_epoch();
        let mut stop_world = self
            .stop_world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if thread.native_state() != crate::NativeState::Native {
            stop_world
                .parked
                .insert(std::ptr::from_mut(thread).expose_provenance());
        }
        while stop_world.parked.len() < self.active_mutators() {
            stop_world = self
                .stop_world_ready
                .wait(stop_world)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        drop(stop_world);
    }

    pub(crate) fn end_collection(&self) {
        let mut stop_world = self
            .stop_world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        stop_world.requested = false;
        stop_world.sync_parked_gate_waiters();
        drop(stop_world);
        self.stop_world_ready.notify_all();
    }

    pub(crate) fn collect_with_thread(&self, thread: &mut Thread, full: bool) {
        let _gate = self.acquire_slow_path_gate(thread);
        self.collect_with_gate(thread, full);
    }

    pub(crate) fn collect_with_gate(&self, thread: &mut Thread, full: bool) {
        #[cfg(target_arch = "x86_64")]
        let diagnostic_test = {
            let current = std::thread::current();
            current
                .name()
                .filter(|name| name.starts_with("weak_"))
                .map(str::to_owned)
        };
        #[cfg(target_arch = "x86_64")]
        if !thread.has_native_frame_snapshot() {
            thread.publish_snapshot();
        }
        #[cfg(target_arch = "x86_64")]
        if let Some(test) = diagnostic_test.as_deref() {
            let rsp: usize;
            unsafe {
                core::arch::asm!(
                    "mov {}, rsp",
                    out(reg) rsp,
                    options(nomem, nostack, preserves_flags)
                );
            }
            log_stack_diagnostic(test, "after_publish_snapshot", thread, rsp);
        }
        {
            let _stop_world = self
                .stop_world
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            thread.state = SafepointState::Collector;
        }
        self.begin_collection(thread);
        {
            let _stop_world = self
                .stop_world
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            thread.state = SafepointState::Collecting;
        }
        #[cfg(target_arch = "x86_64")]
        if let Some(test) = diagnostic_test.as_deref() {
            let rsp: usize;
            unsafe {
                core::arch::asm!(
                    "mov {}, rsp",
                    out(reg) rsp,
                    options(nomem, nostack, preserves_flags)
                );
            }
            log_stack_diagnostic(test, "before_collection", thread, rsp);
        }
        self.collect(full);
        self.reset_allocation_debt();
        self.end_collection();
        let _stop_world = self
            .stop_world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        thread.state = if thread.native_state() == crate::NativeState::Native {
            SafepointState::Safe
        } else if thread.safepoint_request == 0 {
            SafepointState::Running
        } else {
            SafepointState::PollRequested
        };
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
            worker_heap.poll_thread(&mut thread);
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
            collector_heap.collect_with_thread(&mut collector, true);
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
