//! Shared runtime heap and registries.

use crate::hash_table::{HashTable, HashTest, Weakness};
use crate::{
    BuiltinIdentifier, BuiltinImplementation, LispErrorConverter, ObjectError, ThreadContext,
    Word, make_string, with_root,
};
use ncl_sys::{Heap, HeapConfig, RootToken, StorageCondition};
use std::collections::HashMap;
use std::sync::Mutex;

/// Shared runtime heap and registries.
#[derive(Debug)]
pub struct Runtime {
    pub(crate) heap: Box<Heap>,
    functions: Mutex<Option<RootedTable>>,
    pub(crate) packages: Mutex<Option<RootedTable>>,
    pub(crate) classes: Mutex<Option<RootedTable>>,
    pub(crate) features: Mutex<Vec<String>>,
    pub(crate) layouts: Mutex<HashMap<u32, usize>>,
    pub(crate) next_layout: Mutex<u32>,
    layouts_registered: Mutex<bool>,
    pub(crate) builtins: Mutex<HashMap<Word, BuiltinImplementation>>,
    pub(crate) builtin_addresses: Mutex<HashMap<BuiltinIdentifier, usize>>,
    lisp_error_converter: Mutex<Option<LispErrorConverter>>,
}
/// Per-mutator object-layer context. Generated code obtains its stable thread
/// pointer with [`ThreadContext::thread_mut`].
#[derive(Debug)]
pub struct RootedTable {
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
        Heap::validate_address_space().map_err(ObjectError::from)?;
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
            builtin_addresses: Mutex::new(HashMap::new()),
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
        crate::gc::register_layouts(self)?;
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

    /// Return the native entry registered for a typed builtin identifier.
    #[must_use]
    pub fn builtin_address(&self, identifier: BuiltinIdentifier) -> Option<u64> {
        self.builtin_addresses
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&identifier)
            .copied()
            .and_then(|entry| u64::try_from(entry).ok())
    }
    pub(crate) fn table(registry: &Mutex<Option<RootedTable>>) -> Result<Word, ObjectError> {
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
