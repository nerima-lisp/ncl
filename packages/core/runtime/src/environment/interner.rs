use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

thread_local! {
    /// Case-insensitive frame keys, deduplicated so repeated lookups of the
    /// same name reuse one allocation instead of normalizing a fresh
    /// `String` on every `Environment` access -- the hottest path in the
    /// evaluator (variable/function lookup runs on every form).
    static INTERNED_NAMES: RefCell<HashSet<Rc<str>>> = RefCell::new(HashSet::new());
}

/// Normalizes `name` to upper case and returns a deduplicated, reference-
/// counted handle to it, reusing a prior interning of the same name instead
/// of allocating.
///
/// This is intentionally distinct from the crate-wide
/// [`normalize_name`](super::normalize_name), which returns an owned
/// `String` for the many call sites that only need a normalized value to
/// compare or move, not a frame key: caching every one of those would grow
/// the intern table with names that are never looked up twice.
pub fn intern_name(name: &str) -> Rc<str> {
    if name.bytes().all(|byte| !byte.is_ascii_lowercase()) {
        return intern_normalized(name);
    }
    intern_normalized(&name.to_ascii_uppercase())
}

fn intern_normalized(normalized: &str) -> Rc<str> {
    INTERNED_NAMES.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(existing) = cache.get(normalized) {
            return Rc::clone(existing);
        }
        let interned: Rc<str> = Rc::from(normalized);
        cache.insert(Rc::clone(&interned));
        interned
    })
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::intern_name;

    #[test]
    fn repeated_interning_reuses_the_same_allocation() {
        let first = intern_name("mixed-Case");
        let second = intern_name("MIXED-CASE");
        assert_eq!(&*first, "MIXED-CASE");
        assert!(Rc::ptr_eq(&first, &second));
    }

    #[test]
    fn already_uppercase_names_intern_without_a_case_conversion_pass() {
        let first = intern_name("ALREADY-UPPER");
        let second = intern_name("ALREADY-UPPER");
        assert!(Rc::ptr_eq(&first, &second));
    }

    #[test]
    fn distinct_names_intern_to_distinct_allocations() {
        let first = intern_name("one");
        let second = intern_name("two");
        assert!(!Rc::ptr_eq(&first, &second));
    }
}
