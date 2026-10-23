use crate::heap::metadata::{MetadataBudget, MetadataCharge, MetadataVec};
use crate::heap::metadata_map::{MetadataMap, MetadataSet};
use crate::{CodeRegistry, Finalizer, PageKind, ReferenceLayout, Thread, Weakness, Word};

#[derive(Debug)]
pub struct LayoutEntry {
    pub(crate) layout: ReferenceLayout,
    pub(crate) _charge: MetadataCharge,
}

#[derive(Debug)]
pub struct Object {
    pub(crate) words: Box<[u64]>,
    pub(crate) kind: PageKind,
    pub(crate) generation: u8,
    pub(crate) survived: u8,
    pub(crate) pinned: bool,
    pub(crate) weak: Option<Weakness>,
    pub(crate) finalizer: Option<(Finalizer, bool)>,
    pub(crate) alive: bool,
    pub(crate) forwarded_to: Option<usize>,
}
#[derive(Debug)]
pub struct State {
    pub(crate) used: usize,
    pub(crate) gc_epoch: u64,
    pub(crate) collection_preparing: bool,
    pub(crate) remembered_set_overflow: bool,
    pub(crate) objects: MetadataVec<Object>,
    pub(crate) object_starts: MetadataMap<usize, usize>,
    pub(crate) forwarding: MetadataMap<usize, usize>,
    pub(crate) layouts: MetadataMap<u8, LayoutEntry>,
    pub(crate) threads: MetadataVec<*mut Thread>,
    pub(crate) dirty_cards: MetadataSet<(usize, usize)>,
    pub(crate) roots: MetadataVec<*mut Word>,
    pub(crate) root_generations: MetadataVec<u128>,
    pub(crate) next_root_generation: u128,
    pub(crate) finalizers: MetadataVec<(Word, Finalizer)>,
    pub(crate) after_gc_hooks: MetadataVec<fn()>,
    pub(crate) code_registry: CodeRegistry,
}

impl State {
    pub(crate) fn new(budget: MetadataBudget) -> Self {
        Self {
            used: 0,
            gc_epoch: 0,
            collection_preparing: false,
            remembered_set_overflow: false,
            objects: MetadataVec::new(budget.clone()),
            object_starts: MetadataMap::new(budget.clone()),
            forwarding: MetadataMap::new(budget.clone()),
            layouts: MetadataMap::new(budget.clone()),
            threads: MetadataVec::new(budget.clone()),
            dirty_cards: MetadataSet::new(budget.clone()),
            roots: MetadataVec::new(budget.clone()),
            root_generations: MetadataVec::new(budget.clone()),
            next_root_generation: 1,
            finalizers: MetadataVec::new(budget.clone()),
            after_gc_hooks: MetadataVec::new(budget.clone()),
            code_registry: CodeRegistry::with_budget(budget),
        }
    }
}
