use crate::{HeapRootedWord, ObjectError, ThreadContext};
use ncl_sys::Word;

/// An owned sequence of words registered in the heap-level root registry.
///
/// The sequence exposes only [`HeapRootedWord`] handles, so callers must read
/// the current value from a handle after collection instead of retaining a
/// copied `Word` across an allocation.
#[derive(Debug, Default)]
pub struct HeapRootedWords {
    roots: Vec<HeapRootedWord>,
}

impl HeapRootedWords {
    /// Register each word in the heap-level root registry.
    ///
    /// # Errors
    /// Returns an object-layer error if the context cannot register a root.
    pub fn new(ctx: &ThreadContext, values: &[Word]) -> Result<Self, ObjectError> {
        let roots = values
            .iter()
            .copied()
            .map(|value| HeapRootedWord::new(ctx, value))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { roots })
    }

    /// Register one additional word and append its handle to the sequence.
    ///
    /// # Errors
    /// Returns an object-layer error if the context cannot register a root.
    pub fn push(&mut self, ctx: &ThreadContext, value: Word) -> Result<(), ObjectError> {
        self.roots.push(HeapRootedWord::new(ctx, value)?);
        Ok(())
    }

    #[must_use]
    pub fn get(&self, index: usize) -> Option<&HeapRootedWord> {
        self.roots.get(index)
    }

    #[must_use]
    pub fn first(&self) -> Option<&HeapRootedWord> {
        self.roots.first()
    }

    #[must_use]
    pub fn as_slice(&self) -> &[HeapRootedWord] {
        &self.roots
    }

    #[must_use]
    pub fn split_first(&self) -> Option<(&HeapRootedWord, &[HeapRootedWord])> {
        self.roots.split_first()
    }

    pub fn iter(&self) -> impl Iterator<Item = &HeapRootedWord> {
        self.roots.iter()
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.roots.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::HeapRootedWords;
    use crate::{Runtime, ThreadContext, make_string, pop_root, push_root, string_length};
    use ncl_sys::Word;

    #[test]
    fn heap_rooted_words_reads_moved_values_after_collection() {
        let runtime =
            Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("register failed: {error:?}"));
        let first = make_string(&mut ctx, &runtime, &['a'; 64])
            .unwrap_or_else(|error| panic!("first string allocation failed: {error:?}"));
        let second = make_string(&mut ctx, &runtime, &['b'; 96])
            .unwrap_or_else(|error| panic!("second string allocation failed: {error:?}"));
        let rooted = HeapRootedWords::new(&ctx, &[first, second])
            .unwrap_or_else(|error| panic!("heap roots failed: {error:?}"));
        ctx.set_strict_forwarding(true);
        ctx.set_gc_stress(true);
        let _garbage = make_string(&mut ctx, &runtime, &['g'; 64])
            .unwrap_or_else(|error| panic!("stress string allocation failed: {error:?}"));

        ctx.collect(true)
            .unwrap_or_else(|error| panic!("collection failed: {error:?}"));

        assert_eq!(rooted.len(), 2);
        let first = rooted
            .get(0)
            .unwrap_or_else(|| panic!("first heap root missing"));
        let second = rooted
            .get(1)
            .unwrap_or_else(|| panic!("second heap root missing"));
        assert_eq!(string_length(&ctx, first.get()), Ok(64));
        assert_eq!(string_length(&ctx, second.get()), Ok(96));
        assert!(rooted.get(2).is_none());
    }

    #[test]
    fn heap_rooted_words_survives_thread_root_growth_and_collection() {
        let runtime =
            Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("register failed: {error:?}"));
        let value = make_string(&mut ctx, &runtime, &['x'; 64])
            .unwrap_or_else(|error| panic!("string allocation failed: {error:?}"));
        let rooted = HeapRootedWords::new(&ctx, &[value])
            .unwrap_or_else(|error| panic!("heap roots failed: {error:?}"));
        let mut stack_value = Word::fixnum(1);
        let stack_token = push_root(&mut ctx, &mut stack_value);
        ctx.set_strict_forwarding(true);
        ctx.set_gc_stress(true);
        let _garbage = make_string(&mut ctx, &runtime, &['g'; 64])
            .unwrap_or_else(|error| panic!("stress string allocation failed: {error:?}"));

        ctx.collect(true)
            .unwrap_or_else(|error| panic!("collection failed: {error:?}"));

        let value = rooted
            .first()
            .unwrap_or_else(|| panic!("heap root missing"));
        assert_eq!(string_length(&ctx, value.get()), Ok(64));
        assert!(pop_root(&mut ctx, stack_token));
    }

    #[test]
    fn heap_rooted_words_push_appends_a_rooted_handle() {
        let runtime =
            Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("register failed: {error:?}"));
        let first = make_string(&mut ctx, &runtime, &['a'; 64])
            .unwrap_or_else(|error| panic!("first string allocation failed: {error:?}"));
        let second = make_string(&mut ctx, &runtime, &['b'; 64])
            .unwrap_or_else(|error| panic!("second string allocation failed: {error:?}"));
        let mut rooted = HeapRootedWords::new(&ctx, &[first])
            .unwrap_or_else(|error| panic!("heap roots failed: {error:?}"));
        rooted
            .push(&ctx, second)
            .unwrap_or_else(|error| panic!("appending heap root failed: {error:?}"));
        ctx.set_strict_forwarding(true);
        ctx.set_gc_stress(true);
        let _garbage = make_string(&mut ctx, &runtime, &['g'; 64])
            .unwrap_or_else(|error| panic!("stress string allocation failed: {error:?}"));

        ctx.collect(true)
            .unwrap_or_else(|error| panic!("collection failed: {error:?}"));

        assert_eq!(rooted.len(), 2);
        let (first, rest) = rooted
            .split_first()
            .unwrap_or_else(|| panic!("first heap root missing"));
        assert_eq!(string_length(&ctx, first.get()), Ok(64));
        let second = rest
            .first()
            .unwrap_or_else(|| panic!("second heap root missing"));
        assert_eq!(string_length(&ctx, second.get()), Ok(64));
    }
}
