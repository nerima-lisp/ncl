//! Typed, safe values and runtime state for NCL.
#![allow(missing_docs)]
use crate::hash_table::{HashTable, HashTest, Weakness};
pub use ncl_sys::Word;
use ncl_sys::{Heap, HeapConfig, RootToken, StorageCondition, Thread, TypeTag};
use std::collections::HashMap;
use std::sync::Mutex;
pub mod array;
mod builtin;
mod classify;
mod code;
pub mod cons;
mod control_extensions;
mod function;
pub(crate) mod gc;
mod hash_support;
pub mod hash_table;
mod instance;
mod layout;
mod number;
mod object_access;
pub mod package;
mod readtable;
mod registry_extensions;
mod specialized_array;
mod stream;
mod structure;
mod symbol_extensions;
pub use array::{
    ArrayElementType, ArrayOptions, array_dimensions, array_row_major_ref, array_row_major_set,
    make_array, make_simple_vector, make_string, simple_vector_length, simple_vector_ref,
    simple_vector_set, string_length, string_ref, string_set,
};
pub use builtin::{Builtin, FunctionObject, MultipleValues, NclStatus, RegisterFn};
pub use classify::{ObjectRef, classify, classify_object};
pub use code::code_slot;
pub use code::{
    CodeObject, code_constants, code_debug, code_entry, code_size, code_stack_map, make_code_object,
};
pub use cons::{rplaca, rplacd};
pub use function::{
    Function, closure_ref, function_entry, function_name, make_closure, make_simple_fun,
};
pub use function::{function_code, function_lambda_list};
pub use gc::register;
pub use instance::{Instance, instance_class, make_instance, slot_ref, slot_set};
pub use layout::{
    array_offset, code_offset, function_offset, instance_offset, number_offset, readtable_offset,
    simple_vector_offset, specialized_array_offset, stream_offset, string_offset, structure_offset,
    symbol_offset, widetag,
};
pub use ncl_sys::{ThreadLayout, thread_layout};
pub use number::{
    Bignum, Complex, DoubleFloat, Ratio, bignum_limbs, double_value, make_bignum_from_i128,
    make_complex, make_double, make_ratio,
};
pub use number::{bignum_sign, complex_imag, complex_real, ratio_denominator, ratio_numerator};
pub use package::FindStatus;
pub use package::Package;
pub use readtable::readtable_slot;
pub use readtable::{
    Readtable, make_readtable, readtable_case, readtable_dispatch, readtable_syntax,
};
pub use specialized_array::{
    make_specialized_array, specialized_array_element_type, specialized_array_ref,
    specialized_array_set,
};
pub use stream::stream_slot;
pub use stream::{
    Stream, make_stream, stream_direction, stream_element_type, stream_external_format,
    stream_implementation, stream_state,
};
pub use structure::structure_layout;
pub use structure::{StructureLayout, make_structure, structure_ref, structure_set};
pub use symbol_extensions::{
    set_symbol_value, symbol_function, symbol_name, symbol_plist, symbol_value,
};
/// Object-layer failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectError {
    TypeError,
    ContextMoved,
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
    heap: Box<Heap>,
    registry_context: Mutex<Box<ThreadContext>>,
    functions: Mutex<Option<RootedTable>>,
    packages: Mutex<Option<RootedTable>>,
    classes: Mutex<Option<RootedTable>>,
    features: Mutex<Vec<String>>,
    layouts: Mutex<HashMap<u32, usize>>,
    next_layout: Mutex<u32>,
    layouts_registered: Mutex<bool>,
}

#[derive(Debug)]
struct RootedTable {
    slot: Box<Word>,
    _token: RootToken,
}
impl Runtime {
    /// Create a runtime with the default heap policy.
    ///
    /// # Errors
    /// Returns an allocation, layout, or thread-registration error.
    pub fn new() -> Result<Self, ObjectError> {
        Self::with_config(HeapConfig::default())
    }
    /// Create a runtime with an explicit heap policy.
    ///
    /// # Errors
    /// Returns an allocation, layout, or thread-registration error.
    pub fn with_config(config: HeapConfig) -> Result<Self, ObjectError> {
        let runtime = Self {
            heap: Box::new(Heap::new(config)),
            registry_context: Mutex::new(Box::new(ThreadContext::new())),
            functions: Mutex::new(None),
            packages: Mutex::new(None),
            classes: Mutex::new(None),
            features: Mutex::new(Vec::new()),
            layouts: Mutex::new(HashMap::new()),
            next_layout: Mutex::new(1),
            layouts_registered: Mutex::new(false),
        };
        runtime.register_layouts()?;
        let mut context = runtime
            .registry_context
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        ncl_sys::register_thread(&runtime.heap, &mut context.thread).map_err(ObjectError::from)?;
        context.registered_thread_address = Some((&raw const context.thread) as usize);
        for target in [&runtime.functions, &runtime.packages, &runtime.classes] {
            let table =
                HashTable::new(&mut context, &runtime, HashTest::Equal, Weakness::None)?.as_word();
            let mut slot = Box::new(table);
            let token = ncl_sys::push_heap_root(&runtime.heap, &mut slot);
            *target
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(RootedTable {
                slot,
                _token: token,
            });
        }
        ncl_sys::enter_native(&mut context.thread);
        drop(context);
        Ok(runtime)
    }
    /// Register all object layouts supported by this layer.
    ///
    /// # Errors
    ///
    /// Returns [`ObjectError::Layout`] when a widetag is already registered.
    pub fn register_layouts(&self) -> Result<(), ObjectError> {
        let mut registered = self
            .layouts_registered
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if *registered {
            return Ok(());
        }
        gc::register_layouts(self)?;
        *registered = true;
        drop(registered);
        Ok(())
    }
    /// Return the underlying heap.
    pub const fn heap(&self) -> &Heap {
        &self.heap
    }
    /// Register a function object under a package and name.
    ///
    /// # Errors
    /// Returns an allocation, layout, or storage error.
    pub fn define_function(
        &self,
        package: &str,
        name: &str,
        function: Word,
    ) -> Result<(), ObjectError> {
        let mut context = self
            .registry_context
            .lock()
            .map_err(|_| ObjectError::Storage(StorageCondition::ThreadNotRegistered))?;
        let key = format!("{package}::{name}");
        let mut key = make_string(&mut context, self, &key.chars().collect::<Vec<_>>())?;
        let mut function = function;
        let result = with_root(&mut context, &mut key, |context, key| {
            with_root(context, &mut function, |context, function| {
                HashTable::from(Self::table(&self.functions)?).insert(context, self, key, function)
            })
        });
        drop(context);
        result
    }
    /// Look up a registered function object.
    #[must_use]
    pub fn function(&self, package: &str, name: &str) -> Option<Word> {
        let mut context = self.registry_context.lock().ok()?;
        let key = format!("{package}::{name}");
        let key = make_string(&mut context, self, &key.chars().collect::<Vec<_>>()).ok()?;
        let table = Self::table(&self.functions).ok()?;
        let result = HashTable::from(table).get(&mut context, key).ok().flatten();
        drop(context);
        result
    }

    fn table(registry: &Mutex<Option<RootedTable>>) -> Result<Word, ObjectError> {
        registry
            .lock()
            .map_err(|_| ObjectError::Storage(StorageCondition::ThreadNotRegistered))?
            .as_ref()
            .map(|root| *root.slot)
            .ok_or(ObjectError::Layout)
    }
}
/// Per-mutator object-layer context.
///
/// The address of this value may be passed to generated code as a
/// `*mut ncl_sys::Thread`. Generated code may access only the leading `Thread`
/// portion.
#[repr(C)]
#[derive(Debug)]
pub struct ThreadContext {
    pub(crate) thread: Thread,
    registered_thread_address: Option<usize>,
    bindings: Vec<(u32, Word)>,
    values: Vec<Word>,
    pending: Option<ObjectError>,
    non_local_exit: bool,
    handler: Option<usize>,
    cleanup: Option<usize>,
    catch: Option<usize>,
}
impl ThreadContext {
    /// Create an unregistered context.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            thread: Thread::new(),
            registered_thread_address: None,
            bindings: Vec::new(),
            values: Vec::new(),
            pending: None,
            non_local_exit: false,
            handler: None,
            cleanup: None,
            catch: None,
        }
    }
    /// Register this context with a runtime.
    ///
    /// After registration, do not move this context. Keep it in a stable
    /// stack location or put it in a `Box<ThreadContext>` before registering.
    ///
    /// # Errors
    ///
    /// Returns the storage condition reported by the heap.
    pub fn register(&mut self, runtime: &Runtime) -> Result<(), ObjectError> {
        ncl_sys::register_thread(&runtime.heap, &mut self.thread).map_err(ObjectError::from)?;
        self.registered_thread_address = Some((&raw const self.thread) as usize);
        for name in ["COMMON-LISP", "COMMON-LISP-USER", "KEYWORD", "NCL"] {
            runtime.ensure_package(name)?;
        }
        Ok(())
    }
    /// Bind a special variable, preserving stack order.
    pub fn bind(&mut self, index: u32, value: Word) {
        self.bindings.push((index, value));
    }
    /// Remove the latest binding for an index.
    ///
    /// # Errors
    ///
    /// Returns [`ObjectError::Unbound`] when no binding exists.
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
    #[must_use]
    pub fn values(&self) -> &[Word] {
        &self.values
    }
    /// Record a pending condition.
    pub const fn set_pending(&mut self, error: ObjectError) {
        self.pending = Some(error);
    }
    /// Take the pending condition.
    pub const fn take_pending(&mut self) -> Option<ObjectError> {
        self.pending.take()
    }
    /// Run a collection for this registered context.
    ///
    /// # Errors
    /// Returns a storage error if this context was moved after registration.
    pub fn collect(&mut self, full: bool) -> Result<(), ObjectError> {
        self.check_registered_address()?;
        ncl_sys::collect(&mut self.thread, full);
        Ok(())
    }
    /// Mark an object as weak with the requested policy.
    #[must_use]
    pub fn make_weak(&self, value: Word, weakness: ncl_sys::Weakness) -> Word {
        ncl_sys::make_weak(&self.thread, value, weakness)
    }
    /// Read the value slot of a weak object.
    #[must_use]
    pub fn weak_value(&self, value: Word) -> Word {
        ncl_sys::weak_value(&self.thread, value)
    }

    pub(crate) fn check_registered_address(&self) -> Result<(), ObjectError> {
        if self.registered_thread_address.is_none()
            || self.registered_thread_address == Some((&raw const self.thread) as usize)
        {
            Ok(())
        } else {
            Err(ObjectError::ContextMoved)
        }
    }
}
impl Default for ThreadContext {
    fn default() -> Self {
        Self::new()
    }
}
/// Allocate a cons cell.
///
/// # Errors
///
/// Returns the allocation failure reported by the heap.
pub fn make_cons(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    car: Word,
    cdr: Word,
) -> Result<Word, ObjectError> {
    ctx.check_registered_address()?;
    ncl_sys::alloc_cons(&mut ctx.thread, &runtime.heap, car, cdr).map_err(Into::into)
}
/// Allocate a header object with a widetag and payload words.
///
/// # Errors
///
/// Returns the allocation failure reported by the heap.
pub fn allocate(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    tag: u8,
    words: usize,
) -> Result<Word, ObjectError> {
    ctx.check_registered_address()?;
    ncl_sys::alloc(
        &mut ctx.thread,
        &runtime.heap,
        TypeTag { widetag: tag },
        words,
    )
    .map_err(Into::into)
}
/// Allocate a symbol with an initial name and unbound value/function cells.
///
/// # Errors
///
/// Returns the allocation or storage failure reported by the heap.
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
        ncl_sys::write_barrier(&mut ctx.thread, symbol, slot);
    }
    Ok(symbol)
}
/// Push a precise root.
pub fn push_root(ctx: &mut ThreadContext, value: &mut Word) -> RootToken {
    ncl_sys::push_root(&mut ctx.thread, value)
}
/// Pop a precise root.
pub fn pop_root(ctx: &mut ThreadContext, token: RootToken) -> bool {
    ncl_sys::pop_root(&mut ctx.thread, token)
}
/// Push a precise root after validating the context address.
pub fn try_push_root(ctx: &mut ThreadContext, value: &mut Word) -> Result<RootToken, ObjectError> {
    ctx.check_registered_address()?;
    Ok(push_root(ctx, value))
}
/// Pop a precise root after validating the context address.
pub fn try_pop_root(ctx: &mut ThreadContext, token: RootToken) -> Result<bool, ObjectError> {
    ctx.check_registered_address()?;
    Ok(pop_root(ctx, token))
}

pub(crate) fn with_root<T>(
    ctx: &mut ThreadContext,
    value: &mut Word,
    f: impl FnOnce(&mut ThreadContext, Word) -> Result<T, ObjectError>,
) -> Result<T, ObjectError> {
    let token = try_push_root(ctx, value)?;
    let result = f(ctx, *value);
    let popped = try_pop_root(ctx, token)?;
    assert!(popped);
    result
}
/// Return the car of a cons cell.
///
/// # Errors
///
/// Returns a type or storage error when the word is not a cons.
pub fn car(ctx: &mut ThreadContext, word: Word) -> Result<Word, ObjectError> {
    if word == Word::NIL {
        return Ok(Word::NIL);
    }
    if !word.is_cons() {
        return Err(ObjectError::TypeError);
    }
    ncl_sys::read_cons_word(&ctx.thread, word, 0)
        .ok_or(ObjectError::Storage(StorageCondition::ThreadNotRegistered))
}
/// Return the cdr of a cons cell.
///
/// # Errors
///
/// Returns a type or storage error when the word is not a cons.
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
