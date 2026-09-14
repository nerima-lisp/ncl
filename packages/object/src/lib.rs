//! Typed, safe values and runtime state for NCL.

use ncl_sys::{
    Heap, HeapConfig, LowTag, ReferenceLayout, RootToken, StorageCondition, Thread, TypeTag, Word,
};
use std::collections::HashMap;
use std::sync::Mutex;

pub mod hash_table;
pub mod package;

use package::Package;

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
    functions: Mutex<HashMap<(String, String), Word>>,
    packages: Mutex<HashMap<String, Package>>,
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
            functions: Mutex::new(HashMap::new()),
            packages: Mutex::new(
                [
                    ("COMMON-LISP".to_owned(), Package::new("COMMON-LISP")),
                    ("KEYWORD".to_owned(), Package::new("KEYWORD")),
                    (
                        "COMMON-LISP-USER".to_owned(),
                        Package::new("COMMON-LISP-USER"),
                    ),
                    ("NCL".to_owned(), Package::new("NCL")),
                ]
                .into_iter()
                .collect(),
            ),
        }
    }
    /// Register all object layouts supported by this layer.
    pub fn register_layouts(&self) -> Result<(), ObjectError> {
        for (tag, reference_words) in [
            (widetag::SYMBOL, vec![0, 1, 2, 3, 4]),
            (widetag::STRING, vec![]),
            (widetag::SIMPLE_VECTOR, vec![1]),
            (widetag::ARRAY, vec![0, 1, 2]),
            (widetag::HASH_TABLE, vec![0, 1]),
            (widetag::STRUCTURE, vec![0]),
            (widetag::INSTANCE, vec![0, 1]),
            (widetag::SIMPLE_FUN, vec![0, 1]),
            (widetag::CLOSURE, vec![0, 1, 2]),
            (widetag::BIGNUM, vec![]),
            (widetag::RATIO, vec![0, 1]),
            (widetag::DOUBLE_FLOAT, vec![]),
            (widetag::COMPLEX, vec![0, 1]),
            (widetag::PACKAGE, vec![0, 1, 2]),
            (widetag::READTABLE, vec![0]),
            (widetag::STREAM, vec![0, 1]),
            (widetag::CODE, vec![0]),
        ] {
            ncl_sys::register_layout(&self.heap, tag, ReferenceLayout { reference_words })
                .map_err(|_| ObjectError::Layout)?;
        }
        Ok(())
    }
    /// Return the underlying heap.
    pub const fn heap(&self) -> &Heap {
        &self.heap
    }

    /// Create a package if it does not already exist.
    pub fn ensure_package(&self, name: &str) -> bool {
        let mut packages = match self.packages.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if packages.contains_key(name) {
            false
        } else {
            packages.insert(name.to_owned(), Package::new(name));
            true
        }
    }

    /// Register a function object under a package and name.
    pub fn define_function(&self, package: &str, name: &str, function: Word) {
        let mut functions = match self.functions.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        functions.insert((package.to_owned(), name.to_owned()), function);
    }

    /// Look up a registered function object.
    #[must_use]
    pub fn function(&self, package: &str, name: &str) -> Option<Word> {
        let functions = match self.functions.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        functions
            .get(&(package.to_owned(), name.to_owned()))
            .copied()
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

/// Allocate a symbol with an initial name and unbound value/function cells.
pub fn make_symbol(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: Word,
) -> Result<Word, ObjectError> {
    let symbol = allocate(ctx, runtime, widetag::SYMBOL, 8)?;
    for (slot, value) in [
        (symbol_offset::VALUE, Word::UNBOUND),
        (symbol_offset::FUNCTION, Word::UNBOUND),
        (symbol_offset::PLIST, Word::NIL),
        (symbol_offset::PACKAGE, Word::NIL),
        (symbol_offset::NAME, name),
        (symbol_offset::TLS_INDEX, Word::fixnum(0)),
        (symbol_offset::HASH, Word::fixnum(0)),
        (symbol_offset::FLAGS, Word::fixnum(0)),
    ] {
        if !ncl_sys::write_object_word(&mut ctx.thread, symbol, slot, value) {
            return Err(ObjectError::Storage(StorageCondition::ThreadNotRegistered));
        }
    }
    Ok(symbol)
}

fn symbol_slot(ctx: &ThreadContext, symbol: Word, slot: usize) -> Result<Word, ObjectError> {
    if symbol != Word::NIL
        && (symbol.lowtag() != LowTag::OtherPointer as u8
            || ncl_sys::object_widetag(&ctx.thread, symbol) != Some(widetag::SYMBOL))
    {
        return Err(ObjectError::TypeError);
    }
    if symbol == Word::NIL {
        return Ok(Word::NIL);
    }
    ncl_sys::read_object_word(&ctx.thread, symbol, slot)
        .ok_or(ObjectError::Storage(StorageCondition::ThreadNotRegistered))
}

/// Read a symbol's value cell.
pub fn symbol_value(ctx: &ThreadContext, symbol: Word) -> Result<Word, ObjectError> {
    symbol_slot(ctx, symbol, symbol_offset::VALUE)
}

/// Set a symbol's value cell.
pub fn set_symbol_value(
    ctx: &mut ThreadContext,
    symbol: Word,
    value: Word,
) -> Result<(), ObjectError> {
    symbol_slot(ctx, symbol, symbol_offset::VALUE)?;
    if symbol == Word::NIL
        || !ncl_sys::write_object_word(&mut ctx.thread, symbol, symbol_offset::VALUE, value)
    {
        return Err(ObjectError::TypeError);
    }
    ncl_sys::write_barrier(&mut ctx.thread, symbol, symbol_offset::VALUE);
    Ok(())
}

/// Read a symbol's function cell.
pub fn symbol_function(ctx: &ThreadContext, symbol: Word) -> Result<Word, ObjectError> {
    symbol_slot(ctx, symbol, symbol_offset::FUNCTION)
}

/// Read a symbol's property list.
pub fn symbol_plist(ctx: &ThreadContext, symbol: Word) -> Result<Word, ObjectError> {
    symbol_slot(ctx, symbol, symbol_offset::PLIST)
}

/// Read a symbol's name object.
pub fn symbol_name(ctx: &ThreadContext, symbol: Word) -> Result<Word, ObjectError> {
    symbol_slot(ctx, symbol, symbol_offset::NAME)
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
    if word == Word::NIL {
        return Ok(Word::NIL);
    }
    if !word.is_cons() {
        return Err(ObjectError::TypeError);
    }
    ncl_sys::read_cons_word(&_ctx.thread, word, 0)
        .ok_or(ObjectError::Storage(StorageCondition::ThreadNotRegistered))
}
/// Return the cdr of a cons cell.
pub fn cdr(ctx: &mut ThreadContext, word: Word) -> Result<Word, ObjectError> {
    if word == Word::NIL {
        return Ok(Word::NIL);
    }
    if !word.is_cons() {
        return Err(ObjectError::TypeError);
    }
    ncl_sys::read_cons_word(&ctx.thread, word, 1)
        .ok_or(ObjectError::Storage(StorageCondition::ThreadNotRegistered))
}

/// Replace the car of a cons cell.
pub fn rplaca(ctx: &mut ThreadContext, word: Word, value: Word) -> Result<Word, ObjectError> {
    if !word.is_cons() {
        return Err(ObjectError::TypeError);
    }
    if !ncl_sys::write_cons_word(&mut ctx.thread, word, 0, value) {
        return Err(ObjectError::Storage(StorageCondition::ThreadNotRegistered));
    }
    ncl_sys::write_barrier(&mut ctx.thread, word, 0);
    Ok(word)
}

/// Replace the cdr of a cons cell.
pub fn rplacd(ctx: &mut ThreadContext, word: Word, value: Word) -> Result<Word, ObjectError> {
    if !word.is_cons() {
        return Err(ObjectError::TypeError);
    }
    if !ncl_sys::write_cons_word(&mut ctx.thread, word, 1, value) {
        return Err(ObjectError::Storage(StorageCondition::ThreadNotRegistered));
    }
    ncl_sys::write_barrier(&mut ctx.thread, word, 1);
    Ok(word)
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

/// Register the object layer's built-in definitions.
pub fn register(_runtime: &Runtime) {}

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
