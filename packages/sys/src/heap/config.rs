use super::{HashMap, HashSet, Heap, HeapConfig, State};
use std::sync::atomic::AtomicBool;
use std::sync::{Condvar, Mutex};

impl Heap {
    /// Construct a heap with an explicit number of collector workers.
    #[must_use]
    pub fn new_with_workers(config: HeapConfig, worker_count: usize) -> Self {
        Self {
            config,
            worker_count: worker_count.max(1),
            state: Mutex::new(State {
                used: 0,
                gc_epoch: 0,
                objects: Vec::new(),
                object_starts: HashMap::new(),
                layouts: HashMap::new(),
                threads: Vec::new(),
                dirty_cards: HashSet::new(),
                roots: Vec::new(),
                finalizers: Vec::new(),
                after_gc_hooks: Vec::new(),
                code_registry: crate::CodeRegistry::default(),
            }),
            strict_forwarding: AtomicBool::new(false),
            stop_world: Mutex::new(crate::stw::StopWorld::default()),
            stop_world_ready: Condvar::new(),
        }
    }

    /// Return the number of workers used by parallel collection phases.
    pub const fn worker_count(&self) -> usize {
        self.worker_count
    }
}
