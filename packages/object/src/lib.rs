//! Typed, safe values and runtime state for NCL.

use ncl_sys::{
    Heap, HeapConfig, LowTag, ReferenceLayout, RootToken, StorageCondition, Thread, TypeTag, Word,
};

/// Object widetags used by the object layer.
pub mod widetag {
    pub const SYMBOL: u8 = 1;
    pub const STRING: u8 = 2;
    pub const SIMPLE_VECTOR: u8 = 3;
    pub const ARRAY: u8 = 4;
    pub const HASH_TABLE: u8 = 5;
    pub const STRUCTURE: u8 = 6;
    pub const INSTANCE: u8 = 7;
    pub const SIMPLE_FUN: u8 = 8;
    pub const CLOSURE: u8 = 9;
    pub const BIGNUM: u8 = 10;
    pub const RATIO: u8 = 11;
    pub const DOUBLE_FLOAT: u8 = 12;
    pub const COMPLEX: u8 = 13;
    pub const PACKAGE: u8 = 14;
    pub const READTABLE: u8 = 15;
    pub const STREAM: u8 = 16;
    pub const CODE: u8 = 17;
}

/// Payload offsets for a symbol object.
pub mod symbol_offset {
    pub const VALUE: usize = 0;
    pub const FUNCTION: usize = 1;
    pub const PLIST: usize = 2;
    pub const PACKAGE: usize = 3;
    pub const NAME: usize = 4;
    pub const TLS_INDEX: usize = 5;
    pub const HASH: usize = 6;
    pub const FLAGS: usize = 7;
}

/// Classification of a tagged value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ObjectRef {
    Fixnum(i64),
    Character(u32),
    Cons(Word),
    Symbol(Word),
    Function(Word),
    Instance(Word),
    Other { word: Word, widetag: u8 },
    Immediate(Word),
}

/// Classify a tagged value using the information exposed by `ncl-sys`.
pub fn classify(word: Word) -> ObjectRef {
    if let Some(value) = word.as_fixnum() {
        return ObjectRef::Fixnum(value);
    }
    if word.is_character() {
        return ObjectRef::Character((word.bits() >> 4) as u32);
    }
    match word.lowtag() {
        x if x == LowTag::List as u8 => {
            if word == Word::NIL {
                ObjectRef::Symbol(word)
            } else {
                ObjectRef::Cons(word)
            }
        }
        x if x == LowTag::Function as u8 => ObjectRef::Function(word),
        x if x == LowTag::Instance as u8 => ObjectRef::Instance(word),
        x if x == LowTag::OtherImmediate as u8 => ObjectRef::Immediate(word),
        _ => ObjectRef::Other { word, widetag: 0 },
    }
}

/// Object-layer failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectError {
    TypeError,
    Storage(StorageCondition),
    Layout,
    Unbound,
    Unsupported,
}
impl std::fmt::Display for ObjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ObjectError {}
impl From<StorageCondition> for ObjectError {
    fn from(value: StorageCondition) -> Self {
        Self::Storage(value)
    }
}

/// Shared runtime heap and registries.
#[derive(Debug)]
pub struct Runtime {
    heap: Heap,
    package_count: usize,
    function_count: usize,
}
impl Runtime {
    /// Create a runtime with the default heap policy.
    pub fn new() -> Self {
        Self::with_config(HeapConfig::default())
    }
    /// Create a runtime with an explicit heap policy.
    pub fn with_config(config: HeapConfig) -> Self {
        Self {
            heap: Heap::new(config),
            package_count: 0,
            function_count: 0,
        }
    }
    /// Register all object layouts supported by this layer.
    pub fn register_layouts(&self) -> Result<(), ObjectError> {
        for tag in 1..=widetag::CODE {
            ncl_sys::register_layout(
                &self.heap,
                tag,
                ReferenceLayout {
                    reference_words: Vec::new(),
                },
            )
            .map_err(|_| ObjectError::Layout)?;
        }
        Ok(())
    }
    /// Return the underlying heap.
    pub const fn heap(&self) -> &Heap {
        &self.heap
    }
}
impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

/// Per-mutator object-layer context.
#[derive(Debug)]
pub struct ThreadContext {
    thread: Thread,
    bindings: Vec<(u32, Word)>,
    values: Vec<Word>,
    pending: Option<ObjectError>,
}
impl ThreadContext {
    /// Create an unregistered context.
    pub fn new() -> Self {
        Self {
            thread: Thread::new(),
            bindings: Vec::new(),
            values: Vec::new(),
            pending: None,
        }
    }
    /// Register this context with a runtime.
    pub fn register(&mut self, runtime: &Runtime) -> Result<(), ObjectError> {
        ncl_sys::register_thread(&runtime.heap, &mut self.thread).map_err(Into::into)
    }
    /// Bind a special variable, preserving stack order.
    pub fn bind(&mut self, index: u32, value: Word) {
        self.bindings.push((index, value));
    }
    /// Remove the latest binding for an index.
    pub fn unbind(&mut self, index: u32) -> Result<Word, ObjectError> {
        if let Some(position) = self.bindings.iter().rposition(|(key, _)| *key == index) {
            Ok(self.bindings.remove(position).1)
        } else {
            Err(ObjectError::Unbound)
        }
    }
    /// Set multiple values.
    pub fn set_values(&mut self, values: &[Word]) {
        self.values.clear();
        self.values.extend_from_slice(values);
    }
    /// Read multiple values.
    pub fn values(&self) -> &[Word] {
        &self.values
    }
    /// Record a pending condition.
    pub fn set_pending(&mut self, error: ObjectError) {
        self.pending = Some(error);
    }
    /// Take the pending condition.
    pub fn take_pending(&mut self) -> Option<ObjectError> {
        self.pending.take()
    }
}
impl Default for ThreadContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Allocate a cons cell.
pub fn make_cons(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    car: Word,
    cdr: Word,
) -> Result<Word, ObjectError> {
    ncl_sys::alloc_cons(&mut ctx.thread, &runtime.heap, car, cdr).map_err(Into::into)
}
/// Allocate a header object with a widetag and payload words.
pub fn allocate(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    tag: u8,
    words: usize,
) -> Result<Word, ObjectError> {
    ncl_sys::alloc(
        &mut ctx.thread,
        &runtime.heap,
        TypeTag { widetag: tag },
        words,
    )
    .map_err(Into::into)
}
/// Push a precise root.
pub fn push_root(ctx: &mut ThreadContext, value: &mut Word) -> RootToken {
    ncl_sys::push_root(&mut ctx.thread, value)
}
/// Pop a precise root.
pub fn pop_root(ctx: &mut ThreadContext, token: RootToken) -> bool {
    ncl_sys::pop_root(&mut ctx.thread, token)
}
/// Return the car of a cons cell.
pub fn car(_ctx: &mut ThreadContext, word: Word) -> Result<Word, ObjectError> {
    if word.is_cons() {
        Err(ObjectError::Unsupported)
    } else {
        Err(ObjectError::TypeError)
    }
}
/// Return the cdr of a cons cell.
pub fn cdr(ctx: &mut ThreadContext, word: Word) -> Result<Word, ObjectError> {
    car(ctx, word)
}

/// Calling convention status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum NclStatus {
    Ok = 0,
    Error = 1,
}
/// Registered builtin function descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Builtin {
    pub arity: u8,
    pub direct: bool,
}
/// Function registration callback.
pub type RegisterFn = fn(&Runtime);

/// Declare fixed-arity builtin metadata.
#[macro_export]
macro_rules! builtin {
    ($name:ident, $arity:expr) => {
        pub const $name: $crate::Builtin = $crate::Builtin {
            arity: $arity,
            direct: true,
        };
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classify_immediates() {
        assert_eq!(classify(Word::fixnum(-2)), ObjectRef::Fixnum(-2));
        assert_eq!(classify(Word::NIL), ObjectRef::Symbol(Word::NIL));
    }
    #[test]
    fn allocation_requires_registration() {
        let runtime = Runtime::new();
        let mut ctx = ThreadContext::new();
        assert!(make_cons(&mut ctx, &runtime, Word::NIL, Word::NIL).is_err());
        assert!(ctx.register(&runtime).is_ok());
        assert!(make_cons(&mut ctx, &runtime, Word::NIL, Word::NIL).is_ok());
    }
    #[test]
    fn bindings_are_lifo() {
        let mut ctx = ThreadContext::new();
        ctx.bind(1, Word::fixnum(1));
        ctx.bind(1, Word::fixnum(2));
        assert_eq!(ctx.unbind(1), Ok(Word::fixnum(2)));
    }
    #[test]
    fn builtin_metadata_expands() {
        builtin!(TEST_BUILTIN, 2);
        assert_eq!(TEST_BUILTIN.arity, 2);
    }
}
