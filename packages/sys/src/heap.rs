use crate::{Thread, Word};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

const LARGE_OBJECT: usize = 8 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageCondition {
    CapacityExceeded,
    InvalidSize,
    ThreadNotRegistered,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageKind {
    Cons,
    HeaderObject,
    Large,
    Code,
    Static,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypeTag {
    pub widetag: u8,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceLayout {
    pub reference_words: Vec<usize>,
}
#[derive(Debug)]
pub struct LayoutError;
#[derive(Clone, Copy, Debug)]
pub struct HeapConfig {
    pub dynamic_space_size: usize,
    pub bytes_considered_between_gcs: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Weakness {
    Key,
    Value,
    KeyAndValue,
    KeyOrValue,
}
pub type Finalizer = fn(Word);

#[derive(Debug)]
struct Object {
    words: Box<[u64]>,
    kind: PageKind,
    generation: u8,
    survived: u8,
    pinned: bool,
    weak: Option<Weakness>,
    finalizer: Option<(Finalizer, bool)>,
    alive: bool,
}
#[derive(Debug)]
struct State {
    used: usize,
    objects: Vec<Object>,
    layouts: HashMap<u8, ReferenceLayout>,
    threads: Vec<*mut Thread>,
    remembered: HashSet<usize>,
    roots: Vec<*mut Word>,
    weak: Vec<usize>,
    finalizers: Vec<(Word, Finalizer)>,
    after_gc_hooks: Vec<fn()>,
}
#[derive(Debug)]
pub struct Heap {
    config: HeapConfig,
    state: Mutex<State>,
}

impl Default for HeapConfig {
    fn default() -> Self {
        Self {
            dynamic_space_size: 64 * 1024 * 1024,
            bytes_considered_between_gcs: 4 * 1024 * 1024,
        }
    }
}
impl Heap {
    pub fn new(config: HeapConfig) -> Self {
        Self {
            config,
            state: Mutex::new(State {
                used: 0,
                objects: Vec::new(),
                layouts: HashMap::new(),
                threads: Vec::new(),
                remembered: HashSet::new(),
                roots: Vec::new(),
                weak: Vec::new(),
                finalizers: Vec::new(),
                after_gc_hooks: Vec::new(),
            }),
        }
    }
    pub const fn dynamic_space_size(&self) -> usize {
        self.config.dynamic_space_size
    }
    pub const fn bytes_considered_between_gcs(&self) -> usize {
        self.config.bytes_considered_between_gcs
    }
    pub fn register_layout(&self, widetag: u8, layout: ReferenceLayout) -> Result<(), LayoutError> {
        let mut s = self.lock_state();
        if s.layouts.insert(widetag, layout).is_some() {
            Err(LayoutError)
        } else {
            Ok(())
        }
    }
    pub(crate) fn register_thread(&self, thread: &mut Thread) -> Result<(), StorageCondition> {
        let mut s = self.lock_state();
        let ptr = std::ptr::from_mut(thread);
        if s.threads.contains(&ptr) {
            return Err(StorageCondition::ThreadNotRegistered);
        }
        s.threads.push(ptr);
        thread.heap = Some(self);
        Ok(())
    }
    pub(crate) fn unregister_thread(&self, thread: &Thread) {
        let mut s = self.lock_state();
        let p = std::ptr::from_ref(thread).cast_mut();
        s.threads.retain(|item| *item != p);
    }
    pub(crate) fn alloc(
        &self,
        t: &mut Thread,
        tag: TypeTag,
        words: usize,
    ) -> Result<Word, StorageCondition> {
        self.allocate(t, PageKind::HeaderObject, tag, words + 1)
    }
    pub(crate) fn alloc_large(
        &self,
        t: &mut Thread,
        tag: TypeTag,
        words: usize,
    ) -> Result<Word, StorageCondition> {
        self.allocate(t, PageKind::Large, tag, words + 1)
    }
    pub(crate) fn alloc_cons(
        &self,
        t: &mut Thread,
        car: Word,
        cdr: Word,
    ) -> Result<Word, StorageCondition> {
        let w = self.allocate(t, PageKind::Cons, TypeTag { widetag: 0 }, 2)?;
        self.write_words(w, &[(0, car), (1, cdr)]);
        Ok(w)
    }
    fn allocate(
        &self,
        t: &mut Thread,
        kind: PageKind,
        tag: TypeTag,
        words: usize,
    ) -> Result<Word, StorageCondition> {
        if words == 0 || words > (1 << 20) {
            return Err(StorageCondition::InvalidSize);
        }
        if t.heap != Some(self) {
            return Err(StorageCondition::ThreadNotRegistered);
        }
        let bytes = words.checked_mul(8).ok_or(StorageCondition::InvalidSize)?;
        let mut s = self.lock_state();
        if s.used
            .checked_add(bytes)
            .is_none_or(|n| n > self.config.dynamic_space_size)
        {
            return Err(StorageCondition::CapacityExceeded);
        }
        let mut data = vec![0_u64; words].into_boxed_slice();
        if kind != PageKind::Cons {
            data[0] = u64::from(tag.widetag);
        }
        let object = Object {
            words: data,
            kind,
            generation: if bytes >= LARGE_OBJECT { 2 } else { 0 },
            survived: 0,
            pinned: false,
            weak: None,
            finalizer: None,
            alive: true,
        };
        let address = object.words.as_ptr() as usize;
        s.used += bytes;
        s.objects.push(object);
        t.bytes_cons += bytes;
        Ok(Word::pointer(
            address,
            if kind == PageKind::Cons {
                crate::LowTag::List
            } else {
                crate::LowTag::OtherPointer
            },
        ))
    }
    fn find(s: &State, word: Word) -> Option<usize> {
        let addr = word.address();
        s.objects.iter().position(|o| {
            o.alive && {
                let start = o.words.as_ptr() as usize;
                addr >= start && addr < start + o.words.len() * 8 && (addr - start) % 8 == 0
            }
        })
    }
    fn write_words(&self, object: Word, values: &[(usize, Word)]) {
        let mut s = self.lock_state();
        if let Some(i) = Self::find(&s, object) {
            for (slot, value) in values {
                if *slot < s.objects[i].words.len() {
                    s.objects[i].words[*slot] = value.bits();
                }
            }
        }
    }
    pub(crate) fn barrier(&self, object: Word, _slot: usize) {
        let mut s = self.lock_state();
        if let Some(i) = Self::find(&s, object) {
            if s.objects[i].generation > 0 {
                s.remembered.insert(i);
            }
        }
    }
    pub(crate) fn make_weak(&self, value: Word, weakness: Weakness) -> Word {
        let mut s = self.lock_state();
        if let Some(i) = Self::find(&s, value) {
            s.objects[i].weak = Some(weakness);
            s.weak.push(i);
        }
        value
    }
    pub(crate) fn weak_value(&self, value: Word) -> Word {
        let s = self.lock_state();
        Self::find(&s, value)
            .and_then(|i| s.objects[i].words.get(1).copied())
            .map(Word::from_bits)
            .unwrap_or(Word::NIL)
    }
    pub(crate) fn register_finalizer(&self, object: Word, callback: Finalizer) {
        let mut s = self.lock_state();
        if let Some(i) = Self::find(&s, object) {
            s.objects[i].finalizer = Some((callback, false));
        }
    }
    pub(crate) fn collect(&self, full: bool) {
        let mut s = self.lock_state();
        let mut live = HashSet::new();
        let mut stack = Vec::new();
        let mut root_slots = s.roots.clone();
        for thread in s.threads.iter().copied() {
            // SAFETY: registered thread pointers remain valid until unregister_thread.
            root_slots.extend(unsafe { (*thread).roots.iter().copied() });
        }
        for root in root_slots.iter().copied() {
            if !root.is_null() {
                let value = unsafe {
                    /* SAFETY: registered root slots outlive registration. */
                    *root
                };
                if let Some(i) = Self::find(&s, value) {
                    stack.push(i);
                }
            }
        }
        while let Some(i) = stack.pop() {
            if !live.insert(i) {
                continue;
            }
            let layout = if s.objects[i].kind == PageKind::Cons {
                vec![0, 1]
            } else {
                s.layouts
                    .get(&(s.objects[i].words[0] as u8))
                    .map_or_else(Vec::new, |l| l.reference_words.clone())
            };
            for slot in layout {
                if s.objects[i].weak.is_some() && slot == 1 {
                    continue;
                }
                if slot < s.objects[i].words.len() {
                    let value = Word::from_bits(s.objects[i].words[slot]);
                    if let Some(next) = Self::find(&s, value) {
                        stack.push(next);
                    }
                }
            }
        }
        let mut moved = HashMap::new();
        for i in 0..s.objects.len() {
            if live.contains(&i)
                && s.objects[i].generation < 2
                && !s.objects[i].pinned
                && (full || s.objects[i].generation == 0)
            {
                let old = s.objects[i].words.as_ptr() as usize;
                let copy = s.objects[i].words.to_vec().into_boxed_slice();
                let new_addr = copy.as_ptr() as usize;
                s.objects[i].words = copy;
                moved.insert(old, new_addr);
                s.objects[i].survived = s.objects[i].survived.saturating_add(1);
                s.objects[i].generation = s.objects[i].survived.min(2);
            }
        }
        for root in root_slots.iter().copied() {
            if !root.is_null() {
                let value = unsafe {
                    /* SAFETY: registered root slot remains valid. */
                    *root
                };
                if let Some(addr) = moved.get(&value.address()) {
                    unsafe {
                        /* SAFETY: root slot is registered and uniquely mutable by the owner. */
                        *root = Word::pointer(
                            *addr,
                            if value.is_list() {
                                crate::LowTag::List
                            } else {
                                crate::LowTag::OtherPointer
                            },
                        );
                    }
                }
            }
        }
        for i in 0..s.objects.len() {
            let layout = if s.objects[i].kind == PageKind::Cons {
                vec![0, 1]
            } else {
                s.layouts
                    .get(&(s.objects[i].words[0] as u8))
                    .map_or_else(Vec::new, |l| l.reference_words.clone())
            };
            for slot in layout {
                if s.objects[i].weak.is_some() && slot == 1 {
                    continue;
                }
                if slot < s.objects[i].words.len() {
                    let value = Word::from_bits(s.objects[i].words[slot]);
                    if let Some(addr) = moved.get(&value.address()) {
                        s.objects[i].words[slot] = Word::pointer(
                            *addr,
                            if value.is_list() {
                                crate::LowTag::List
                            } else {
                                crate::LowTag::OtherPointer
                            },
                        )
                        .bits();
                    }
                }
            }
        }
        for i in (0..s.objects.len()).rev() {
            if !live.contains(&i) && s.objects[i].generation == 0 {
                if let Some((cb, false)) = s.objects[i].finalizer {
                    let value = Word::pointer(
                        s.objects[i].words.as_ptr() as usize,
                        crate::LowTag::OtherPointer,
                    );
                    s.finalizers.push((value, cb));
                    s.objects[i].finalizer = Some((cb, true));
                }
                s.used = s.used.saturating_sub(s.objects[i].words.len() * 8);
                s.objects[i].alive = false;
            }
        }
        for i in 0..s.objects.len() {
            if s.objects[i].weak.is_some() && s.objects[i].alive {
                let value = Word::from_bits(s.objects[i].words.get(1).copied().unwrap_or(0));
                if Self::find(&s, value).is_none() {
                    s.objects[i].words[1] = Word::NIL.bits();
                }
            }
        }
        let hooks = s.after_gc_hooks.clone();
        let finalizers = s.finalizers.drain(..).collect::<Vec<_>>();
        drop(s);
        for (_, callback) in finalizers {
            callback(Word::NIL);
        }
        for hook in hooks {
            hook();
        }
    }
    pub(crate) fn register_after_gc_hook(&self, hook: fn()) {
        self.lock_state().after_gc_hooks.push(hook);
    }
    fn lock_state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }
}
unsafe impl Send for State {}
unsafe impl Sync for State {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tags_round_trip() {
        let v = Word::fixnum(-42);
        assert_eq!(v.as_fixnum(), Some(-42));
        assert!(Word::NIL.is_list());
    }
    #[test]
    fn allocation_and_limit() {
        let h = Heap::new(HeapConfig {
            dynamic_space_size: 8,
            ..HeapConfig::default()
        });
        let mut t = Thread::new();
        assert_eq!(h.register_thread(&mut t), Ok(()));
        assert!(h.alloc_cons(&mut t, Word::NIL, Word::NIL).is_err());
    }
    #[test]
    fn duplicate_layout_rejected() {
        let h = Heap::new(HeapConfig::default());
        let l = ReferenceLayout {
            reference_words: vec![0],
        };
        assert!(h.register_layout(9, l.clone()).is_ok());
        assert!(h.register_layout(9, l).is_err());
    }
    #[test]
    fn roots_are_lifo() {
        let mut t = Thread::new();
        let mut v = Word::NIL;
        let token = t.push_root(&mut v);
        assert!(t.pop_root(token));
    }
}
