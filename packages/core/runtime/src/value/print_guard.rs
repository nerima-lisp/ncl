use std::cell::RefCell;
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrintKind {
    Cons,
    Vector,
    Structure,
}

thread_local! {
    static ACTIVE: RefCell<HashSet<(PrintKind, usize)>> = RefCell::new(HashSet::new());
}

// This is an internal termination guard, not Lisp PRINT-CIRCLE serialization.
pub struct PrintGuard((PrintKind, usize));

impl PrintGuard {
    // The caller keeps the allocation alive until this guard is dropped.
    pub(crate) fn enter(kind: PrintKind, identity: usize) -> Option<Self> {
        let key = (kind, identity);
        ACTIVE
            .with(|active| active.borrow_mut().insert(key))
            .then(|| Self(key))
    }
}

impl Drop for PrintGuard {
    fn drop(&mut self) {
        ACTIVE.with(|active| active.borrow_mut().remove(&self.0));
    }
}
