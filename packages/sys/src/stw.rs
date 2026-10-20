use crate::{Heap, SafepointState, Thread};
use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct StopWorld {
    pub(crate) epoch: u64,
    pub(crate) requested: bool,
    pub(crate) parked: HashSet<usize>,
    pub(crate) gate_parked: HashSet<usize>,
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
            let gate_parked = stop_world.gate_parked.clone();
            stop_world.parked = gate_parked;
        }
        drop(stop_world);
        self.stop_world_ready.notify_all();
    }

    pub(crate) fn poll_thread(&self, thread: &mut Thread) {
        let pointer = std::ptr::from_mut(thread) as usize;
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
        let pointer = std::ptr::from_mut(thread) as usize;
        self.request_epoch();
        let mut stop_world = self
            .stop_world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        stop_world.parked.insert(pointer);
        let active = self.active_mutators();
        while stop_world.parked.len() < active {
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
        let gate_parked = stop_world.gate_parked.clone();
        stop_world.parked = gate_parked;
        drop(stop_world);
        self.stop_world_ready.notify_all();
    }

    pub(crate) fn collect_with_thread(&self, thread: &mut Thread, full: bool) {
        let _gate = self.acquire_slow_path_gate(thread);
        self.collect_with_gate(thread, full);
    }

    pub(crate) fn collect_with_gate(&self, thread: &mut Thread, full: bool) {
        if !thread.has_native_frame_snapshot() {
            thread.publish_snapshot();
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
        self.collect(full);
        self.reset_allocation_debt();
        self.end_collection();
        let stop_world = self
            .stop_world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        thread.state = if thread.safepoint_request == 0 {
            SafepointState::Running
        } else {
            SafepointState::PollRequested
        };
        drop(stop_world);
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
        ready.wait();

        {
            let mut stop_world = heap
                .stop_world
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            stop_world.requested = true;
            stop_world.epoch = 1;
        }
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
}
