use crate::{Heap, SafepointState, Thread};
use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct StopWorld {
    pub(crate) epoch: u64,
    pub(crate) requested: bool,
    pub(crate) parked: HashSet<usize>,
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
            stop_world.parked.clear();
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
        if !stop_world.requested {
            thread.state = SafepointState::Running;
            drop(stop_world);
            return;
        }
        thread.safepoint_epoch = stop_world.epoch;
        thread.state = SafepointState::Published;
        stop_world.parked.insert(pointer);
        self.stop_world_ready.notify_all();
        while stop_world.requested {
            stop_world = self
                .stop_world_ready
                .wait(stop_world)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        drop(stop_world);
        thread.state = SafepointState::Running;
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
        stop_world.parked.clear();
        drop(stop_world);
        self.stop_world_ready.notify_all();
    }

    pub(crate) fn collect_with_thread(&self, thread: &mut Thread, full: bool) {
        self.begin_collection(thread);
        thread.state = SafepointState::Collecting;
        self.collect(full);
        thread.state = SafepointState::Running;
        self.end_collection();
    }
}
