use super::super::{Heap, StorageCondition};
use crate::heap::Weakness;
use crate::heap::metadata::{MetadataBudget, MetadataVec};
use crate::heap::metadata_map::{MetadataMap, MetadataSet};

pub struct CollectionScratch {
    pub(super) live: MetadataSet<usize>,
    pub(super) stack: MetadataVec<usize>,
    pub(super) weak_kv: MetadataMap<usize, Weakness>,
    pub(super) pending_weak_tables: MetadataVec<usize>,
    pub(super) pending_weak_next: MetadataVec<usize>,
    pub(super) pending_weak_set: MetadataSet<usize>,
    pub(super) moved: MetadataMap<usize, usize>,
    pub(super) dirty_objects: MetadataVec<usize>,
}

impl CollectionScratch {
    fn new(
        budget: MetadataBudget,
        object_count: usize,
        dirty_count: usize,
    ) -> Result<Self, StorageCondition> {
        let maximum_objects = object_count
            .checked_mul(2)
            .ok_or(StorageCondition::HeapMetadataCapacityExceeded)?;
        let mut live = MetadataSet::new(budget.clone());
        live.reserve(maximum_objects)
            .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)?;
        let mut stack = MetadataVec::new(budget.clone());
        stack
            .reserve(maximum_objects)
            .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)?;
        let mut weak_kv = MetadataMap::new(budget.clone());
        weak_kv
            .reserve(object_count)
            .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)?;
        let mut pending_weak_tables = MetadataVec::new(budget.clone());
        pending_weak_tables
            .reserve(object_count)
            .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)?;
        let mut pending_weak_next = MetadataVec::new(budget.clone());
        pending_weak_next
            .reserve(object_count)
            .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)?;
        let mut pending_weak_set = MetadataSet::new(budget.clone());
        pending_weak_set
            .reserve(object_count)
            .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)?;
        let mut moved = MetadataMap::new(budget.clone());
        moved
            .reserve(object_count)
            .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)?;
        let mut dirty_objects = MetadataVec::new(budget);
        dirty_objects
            .reserve(dirty_count)
            .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)?;
        Ok(Self {
            live,
            stack,
            weak_kv,
            pending_weak_tables,
            pending_weak_next,
            pending_weak_set,
            moved,
            dirty_objects,
        })
    }
}

impl Heap {
    pub(crate) fn prepare_collection(&self) -> Result<CollectionScratch, StorageCondition> {
        let mut state = self.lock_state();
        if state.collection_preparing {
            return Err(StorageCondition::CollectionInProgress);
        }
        state.collection_preparing = true;
        let prepared = (|| {
            let object_count = state.objects.len();
            let maximum_objects = object_count
                .checked_mul(2)
                .ok_or(StorageCondition::HeapMetadataCapacityExceeded)?;
            state
                .objects
                .reserve(object_count)
                .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)?;
            state
                .object_starts
                .reserve(object_count)
                .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)?;
            state
                .forwarding
                .reserve(object_count)
                .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)?;
            state
                .dirty_cards
                .reserve(maximum_objects)
                .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)?;
            // Any existing object can be armed after preflight, so reserve one slot per object.
            state
                .finalizers
                .reserve(object_count)
                .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)?;
            Self::validate_hash_table_layouts(&state)?;
            let dirty_count = state.dirty_cards.len();
            CollectionScratch::new(self.metadata_budget.clone(), object_count, dirty_count)
        })();
        if prepared.is_err() {
            state.collection_preparing = false;
        }
        prepared
    }
}
