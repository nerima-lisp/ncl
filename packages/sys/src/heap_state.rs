use crate::{CodeRegistry, Finalizer, PageKind, ReferenceLayout, Thread, Weakness, Word};
use std::collections::{HashMap, HashSet};

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
    pub(crate) objects: Vec<Object>,
    pub(crate) layouts: HashMap<u8, ReferenceLayout>,
    pub(crate) threads: Vec<*mut Thread>,
    pub(crate) dirty_cards: HashSet<(usize, usize)>,
    pub(crate) roots: Vec<*mut Word>,
    pub(crate) finalizers: Vec<(Word, Finalizer)>,
    pub(crate) after_gc_hooks: Vec<fn()>,
    pub(crate) code_registry: CodeRegistry,
}
