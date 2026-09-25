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
pub mod hash_table;
mod instance;
mod layout;
mod number;
mod object_access;
pub mod package;
mod readtable;
mod registry_extensions;
mod roots;
mod specialized_array;
mod stream;
mod structure;
mod symbol_extensions;
mod typed;
pub use array::{
    ArrayElementType, ArrayOptions, array_dimensions, array_displacement, array_fill_pointer,
    array_row_major_ref, array_row_major_set, array_set_fill_pointer, make_array,
    make_simple_vector, make_string, simple_vector_length, simple_vector_ref, simple_vector_set,
    string_length, string_ref, string_set,
};
pub use builtin::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, FunctionObject, KeywordAdapter, LambdaList, MultipleValues,
    LispErrorConverter, NclStatus, Parameter, ParameterType, RegisterFn, RustBuiltin,
};
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
    symbol_flag, symbol_offset, widetag,
};
pub use ncl_sys::{ThreadLayout, thread_layout};
pub use number::{
    Bignum, Complex, DoubleFloat, Ratio, bignum_limbs, double_value, make_bignum_from_i128,
    make_complex, make_double, make_ratio,
};
pub use number::{bignum_sign, complex_imag, complex_real, ratio_denominator, ratio_numerator};
pub use package::{FindStatus, Package};
pub use readtable::readtable_slot;
pub use readtable::{
    Readtable, make_readtable, readtable_case, readtable_dispatch, readtable_syntax,
};
pub(crate) use roots::{finish_root, with_root, with_roots};
pub use roots::{pop_root, push_root, try_pop_root, try_push_root};
pub use specialized_array::{
    make_specialized_array, specialized_array_element_type, specialized_array_ref,
    specialized_array_set,
};
pub use stream::stream_slot;
pub use stream::{
    Stream, make_stream, stream_direction, stream_element_type, stream_external_format,
    stream_implementation, stream_state,
};
pub use structure::{
    StructureLayout, make_structure, structure_layout, structure_ref, structure_set,
};
pub use symbol_extensions::{
    set_symbol_constant, set_symbol_macro, set_symbol_package_locked, set_symbol_special,
    set_symbol_value, symbol_flags, symbol_function, symbol_is_constant, symbol_is_macro,
    symbol_is_package_locked, symbol_is_special, symbol_name, symbol_package, symbol_plist,
    symbol_value,
};
pub use typed::{
    ArithmeticError, Array, CellError, Character, Closure, Cons, ControlError, FileError, Fixnum,
    FromLispArg, FunctionDesignator, Integer, LispError, LispString, List, Number, ObjectErrorKind,
    ObjectType, PackageDesignator, PackageError, ProgramError, Rational, Real, Sequence,
    SimpleVector, SpecializedArray, StreamError, StringDesignator, StringObject, StructureObject,
    Symbol, TypeError, TypedRustBuiltin, WordView,
};
/// Object-layer failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectError {
    TypeError,
    Storage(StorageCondition),
    Layout,
    Unbound,
    Unsupported,
    PackageConflict,
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
    functions: Mutex<Option<RootedTable>>,
    packages: Mutex<Option<RootedTable>>,
    classes: Mutex<Option<RootedTable>>,
    features: Mutex<Vec<String>>,
    layouts: Mutex<HashMap<u32, usize>>,
    next_layout: Mutex<u32>,
    layouts_registered: Mutex<bool>,
    builtins: Mutex<HashMap<Word, BuiltinImplementation>>,
    lisp_error_converter: Mutex<Option<LispErrorConverter>>,
}
/// Per-mutator object-layer context. Generated code obtains its stable thread
/// pointer with [`ThreadContext::thread_mut`].
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
            functions: Mutex::new(None),
            packages: Mutex::new(None),
            classes: Mutex::new(None),
            features: Mutex::new(Vec::new()),
            layouts: Mutex::new(HashMap::new()),
            next_layout: Mutex::new(1),
            layouts_registered: Mutex::new(false),
            builtins: Mutex::new(HashMap::new()),
            lisp_error_converter: Mutex::new(None),
        };
        runtime.register_layouts()?;
        let mut context = ThreadContext::new();
        ncl_sys::register_thread(&runtime.heap, &mut context.thread).map_err(ObjectError::from)?;
        context.registered = true;
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
        for name in ["COMMON-LISP", "COMMON-LISP-USER", "KEYWORD", "NCL"] {
            runtime.ensure_package(&mut context, name)?;
        }
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
    /// Register a low-level object layout with this runtime's heap.
    ///
    /// # Errors
    /// Returns [`ObjectError::Layout`] if the widetag is already registered.
    pub fn register_layout(
        &self,
        widetag: u8,
        layout: ncl_sys::ReferenceLayout,
    ) -> Result<(), ObjectError> {
        ncl_sys::register_layout(&self.heap, widetag, layout).map_err(|_| ObjectError::Layout)
    }
    /// Return the widetag of an object, if it is valid for this runtime.
    #[must_use]
    pub fn widetag(&self, object: Word) -> Option<u8> {
        ncl_sys::widetag(&self.heap, object)
    }
    /// Configure strict stale-word checking for this runtime heap.
    pub fn set_strict_forwarding(&self, on: bool) {
        self.heap.set_strict_forwarding(on);
    }
    /// Register a function object under a package and name.
    ///
    /// # Errors
    /// Returns an allocation, layout, or storage error.
    pub fn define_function(
        &self,
        ctx: &mut ThreadContext,
        package: &str,
        name: &str,
        function: Word,
    ) -> Result<(), ObjectError> {
        let key = format!("{package}::{name}");
        let mut function = function;
        with_root(ctx, &mut function, |context, function| {
            let mut key = make_string(context, self, &key.chars().collect::<Vec<_>>())?;
            with_root(context, &mut key, |context, key| {
                HashTable::from_word(Self::table(&self.functions)?)
                    .insert(context, self, *key, *function)
            })
        })
    }
    /// Look up a registered function object.
    #[must_use]
    pub fn function(&self, ctx: &mut ThreadContext, package: &str, name: &str) -> Option<Word> {
        let key = format!("{package}::{name}");
        let key = make_string(ctx, self, &key.chars().collect::<Vec<_>>()).ok()?;
        let table = Self::table(&self.functions).ok()?;
        HashTable::from_word(table).get(ctx, key).ok().flatten()
    }
    fn table(registry: &Mutex<Option<RootedTable>>) -> Result<Word, ObjectError> {
        registry
            .lock()
            .map_err(|_| ObjectError::Storage(StorageCondition::ThreadNotRegistered))?
            .as_ref()
            .map(|root| *root.slot)
            .ok_or(ObjectError::Layout)
    }

    /// Install the higher-layer converter for typed builtin failures.
    pub fn register_lisp_error_converter(&self, converter: LispErrorConverter) {
        *self
            .lisp_error_converter
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(converter);
    }

    pub(crate) fn lisp_error_converter(&self) -> Option<LispErrorConverter> {
        self.lisp_error_converter
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .copied()
    }
}
#[derive(Debug)]
pub struct ThreadContext {
    pub(crate) thread: Box<Thread>,
    registered: bool,
    bindings: Vec<(u32, Word)>,
    values: Vec<Word>,
    pending: Option<ObjectError>,
    pending_lisp_error: Option<LispError>,
    pending_condition: Option<Word>,
    non_local_exit: bool,
    handler: Option<usize>,
    cleanup: Option<usize>,
    catch: Option<usize>,
    gc_stress: bool,
}
impl ThreadContext {
    /// Create an unregistered context.
    #[must_use]
    pub fn new() -> Self {
        Self {
            thread: Box::new(Thread::new()),
            registered: false,
            bindings: Vec::new(),
            values: Vec::new(),
            pending: None,
            pending_lisp_error: None,
            pending_condition: None,
            non_local_exit: false,
            handler: None,
            cleanup: None,
            catch: None,
            gc_stress: false,
        }
    }
    /// Register this context with a runtime.
    ///
    /// The thread state is heap allocated, so moving this context after registration is safe.
    /// The `runtime` must outlive every registered context: dropping it while a context is
    /// still registered would leave the thread's heap reference dangling.
    ///
    /// # Errors
    ///
    /// Returns the storage condition reported by the heap.
    pub fn register(&mut self, runtime: &Runtime) -> Result<(), ObjectError> {
        ncl_sys::register_thread(&runtime.heap, &mut self.thread).map_err(ObjectError::from)?;
        self.registered = true;
        for name in ["COMMON-LISP", "COMMON-LISP-USER", "KEYWORD", "NCL"] {
            runtime.ensure_package(self, name)?;
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
    pub fn set_pending_lisp_error(&mut self, error: LispError) {
        self.pending_lisp_error = Some(error);
    }
    pub(crate) const fn take_pending_lisp_error(&mut self) -> Option<LispError> {
        self.pending_lisp_error.take()
    }
    /// Store the condition object produced for the latest typed builtin error.
    pub const fn set_pending_condition(&mut self, condition: Word) {
        self.pending_condition = Some(condition);
    }
    /// Take the pending condition object, if one was produced at the builtin boundary.
    pub const fn take_pending_condition(&mut self) -> Option<Word> {
        self.pending_condition.take()
    }
    /// Run a collection for this registered context.
    ///
    /// # Errors
    /// Returns a storage error if this context is not registered.
    pub fn collect(&mut self, full: bool) -> Result<(), ObjectError> {
        self.require_registered()?;
        ncl_sys::collect(&mut self.thread, full);
        Ok(())
    }
    /// Force a full collection before every object allocation when enabled.
    pub const fn set_gc_stress(&mut self, on: bool) {
        self.gc_stress = on;
    }
    /// Configure strict stale-word checking for this context's heap.
    pub fn set_strict_forwarding(&self, on: bool) {
        ncl_sys::set_strict_forwarding(&self.thread, on);
    }
    /// Return the stable thread pointer used by generated code.
    pub fn thread_mut(&mut self) -> &mut Thread {
        &mut self.thread
    }
    /// Mark an object as weak with the requested policy.
    ///
    /// This low-level operation does not validate registration or context movement.
    #[must_use]
    pub fn make_weak(&self, value: Word, weakness: ncl_sys::Weakness) -> Word {
        ncl_sys::make_weak(&self.thread, value, weakness)
    }
    /// Read the value slot of a weak object.
    ///
    /// This low-level operation does not validate registration or context movement.
    #[must_use]
    pub fn weak_value(&self, value: Word) -> Word {
        ncl_sys::weak_value(&self.thread, value)
    }
    const fn require_registered(&self) -> Result<(), ObjectError> {
        if !self.registered {
            return Err(ObjectError::Storage(StorageCondition::ThreadNotRegistered));
        }
        Ok(())
    }
}
/// Unregister the context from its heap.
///
/// `ncl_sys::unregister_thread` resolves the heap through the `Thread`'s stored
/// heap reference, so the `Runtime` that owns the heap must outlive every
/// registered `ThreadContext`. Dropping a `Runtime` while a registered context
/// is still alive would dereference freed heap.
impl Drop for ThreadContext {
    fn drop(&mut self) {
        if self.registered {
            ncl_sys::unregister_thread(&self.thread);
            self.registered = false;
        }
    }
}
impl Default for ThreadContext {
    fn default() -> Self {
        Self::new()
    }
}
/// Allocate a cons cell.
/// # Errors
/// Returns the allocation failure reported by the heap.
pub fn make_cons(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    car: Word,
    cdr: Word,
) -> Result<Word, ObjectError> {
    ctx.require_registered()?;
    let mut car = car;
    crate::with_root(ctx, &mut car, |ctx, car| {
        let mut cdr = cdr;
        crate::with_root(ctx, &mut cdr, |ctx, cdr| {
            if ctx.gc_stress {
                ctx.collect(true)?;
            }
            ncl_sys::alloc_cons(&mut ctx.thread, &runtime.heap, *car, *cdr).map_err(Into::into)
        })
    })
}
/// Allocate a header object with a widetag and payload words.
/// # Errors
/// Returns the allocation failure reported by the heap.
pub fn allocate(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    tag: u8,
    words: usize,
) -> Result<Word, ObjectError> {
    ctx.require_registered()?;
    if ctx.gc_stress {
        ctx.collect(true)?;
    }
    ncl_sys::alloc(
        &mut ctx.thread,
        &runtime.heap,
        TypeTag { widetag: tag },
        words,
    )
    .map_err(Into::into)
}
/// Allocate a symbol with an initial name and unbound value/function cells.
/// # Errors
///
/// Returns the allocation or storage failure reported by the heap.
pub fn make_symbol(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: Word,
) -> Result<Word, ObjectError> {
    let mut name = name;
    crate::with_root(ctx, &mut name, |ctx, name| {
        let symbol = allocate(ctx, runtime, widetag::SYMBOL, 8)?;
        for (slot, value) in [
            (symbol_offset::VALUE, Word::UNBOUND),
            (symbol_offset::FUNCTION, Word::UNBOUND),
            (symbol_offset::PLIST, Word::NIL),
            (symbol_offset::PACKAGE, Word::NIL),
            (symbol_offset::NAME, *name),
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
    })
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
