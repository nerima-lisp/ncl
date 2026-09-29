//! Shared runtime heap and registries.

use crate::hash_table::{HashTable, HashTest, Weakness};
use crate::keyword_builtins::{
    CHECK_KEYWORDS_DESCRIPTOR, KEYWORD_VALUE_DESCRIPTOR, MAKE_REST_LIST_DESCRIPTOR,
    check_keywords_builtin, keyword_supplied_p_builtin, keyword_value_builtin,
    make_rest_list_builtin,
};
use crate::{
    BuiltinIdentifier, BuiltinImplementation, BuiltinName, BuiltinPackage, LispErrorConverter,
    ObjectError, PlaceExpander, ThreadContext, Word, make_string, with_root,
};
use ncl_sys::{Heap, HeapConfig, RootToken, StorageCondition};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

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
    structure_layout_parents: Mutex<HashMap<u32, Option<u32>>>,
    structure_classes: Mutex<HashMap<u32, String>>,
    structure_layout_names: Mutex<HashMap<String, u32>>,
    layouts_registered: Mutex<bool>,
    pub(crate) builtins: Mutex<Vec<crate::builtin::BuiltinEntry>>,
    pub(crate) builtin_addresses: Mutex<HashMap<BuiltinIdentifier, usize>>,
    pub(crate) place_expanders: Mutex<crate::place::PlaceExpanders>,
    lisp_error_converter: Mutex<Option<LispErrorConverter>>,
    /// Native entry shared by every builtin that has no ISA-specific fast
    /// path. Zero means "not installed yet"; see
    /// [`Runtime::install_generic_builtin_entry`].
    generic_builtin_entry: AtomicUsize,
    load_port: Mutex<Option<std::sync::Arc<dyn crate::LoadPort>>>,
}
/// Per-mutator object-layer context. Generated code obtains its stable thread
/// pointer with [`ThreadContext::thread_mut`].
#[derive(Debug)]
pub struct RootedTable {
    slot: Box<Word>,
    _token: RootToken,
}

/// A runtime-owned word that remains a precise GC root for the runtime life.
#[derive(Debug)]
pub struct RootedWord {
    slot: Box<Word>,
    _token: RootToken,
}

impl RootedWord {
    pub(crate) fn new(heap: &Heap, value: Word) -> Self {
        let mut slot = Box::new(value);
        let token = ncl_sys::push_heap_root(heap, &mut slot);
        Self {
            slot,
            _token: token,
        }
    }

    pub(crate) fn get(&self) -> Word {
        *self.slot
    }
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
            structure_layout_parents: Mutex::new(HashMap::new()),
            structure_classes: Mutex::new(HashMap::new()),
            structure_layout_names: Mutex::new(HashMap::new()),
            layouts_registered: Mutex::new(false),
            builtins: Mutex::new(Vec::new()),
            builtin_addresses: Mutex::new(HashMap::new()),
            place_expanders: Mutex::new(crate::place::PlaceExpanders::default()),
            lisp_error_converter: Mutex::new(None),
            generic_builtin_entry: AtomicUsize::new(0),
            load_port: Mutex::new(None),
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
        context.ensure_standard_packages(&runtime)?;
        runtime.register_keyword_builtins(&mut context)?;
        Ok(runtime)
    }

    /// Associate a structure layout with its rooted structure class.
    ///
    /// # Errors
    ///
    /// Returns `Layout` if the runtime registry lock is poisoned.
    pub fn register_structure_class(
        &self,
        layout: crate::StructureLayout,
        name: impl Into<String>,
    ) -> Result<(), ObjectError> {
        self.structure_classes
            .lock()
            .map_err(|_| ObjectError::Layout)?
            .insert(layout.into(), name.into());
        Ok(())
    }

    /// Associate a structure layout with its parent layout and rooted class.
    ///
    /// The parent relation is kept as numeric layout metadata so structure
    /// predicates do not need to retain or compare class names.
    ///
    /// # Errors
    /// Returns `Layout` if a runtime registry lock is poisoned.
    pub fn register_structure_class_with_parent(
        &self,
        layout: crate::StructureLayout,
        parent: Option<crate::StructureLayout>,
        name: impl Into<String>,
    ) -> Result<(), ObjectError> {
        self.structure_layout_parents
            .lock()
            .map_err(|_| ObjectError::Layout)?
            .insert(layout.into(), parent.map(Into::into));
        let name = name.into();
        self.structure_layout_names
            .lock()
            .map_err(|_| ObjectError::Layout)?
            .insert(name.clone(), layout.into());
        self.register_structure_class(layout, name)
    }

    /// Resolve a registered structure name to its numeric layout metadata.
    #[must_use]
    pub fn structure_layout_for_name(&self, name: &str) -> Option<crate::StructureLayout> {
        self.structure_layout_names
            .lock()
            .ok()
            .and_then(|names| names.get(name).copied())
            .map(crate::StructureLayout::from)
    }

    /// Return whether `layout` is `expected` or derives from it.
    #[must_use]
    pub fn structure_layout_is_a(
        &self,
        layout: crate::StructureLayout,
        expected: crate::StructureLayout,
    ) -> bool {
        let parents = self
            .structure_layout_parents
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut current = Some(layout.into());
        let mut result = false;
        while let Some(id) = current {
            if id == expected.into() {
                result = true;
                break;
            }
            current = parents.get(&id).copied().flatten();
        }
        drop(parents);
        result
    }

    /// Return the class associated with a structure layout, if any.
    ///
    /// The class is resolved at lookup time so the managed class word is
    /// obtained through the runtime's rooted class registry.
    #[must_use]
    pub fn structure_class(
        &self,
        ctx: &mut ThreadContext,
        layout: crate::StructureLayout,
    ) -> Option<Word> {
        self.structure_classes
            .lock()
            .ok()
            .and_then(|classes| classes.get(&layout.into()).cloned())
            .and_then(|name| self.class(ctx, &name))
    }

    /// Install the evaluator service used by the Common Lisp `LOAD` builtin.
    pub fn set_load_port(&self, port: Box<dyn crate::LoadPort>) {
        *self
            .load_port
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(port.into());
    }

    /// Dispatch a load request to the installed evaluator service.
    ///
    /// # Errors
    ///
    /// Returns `Unsupported` when no evaluator service is installed.
    pub fn load_port(
        &self,
        ctx: &mut ThreadContext,
        args: &crate::BuiltinArgs<'_>,
        values: &mut crate::MultipleValues,
    ) -> Result<Word, ObjectError> {
        let port = self
            .load_port
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .cloned()
            .ok_or(ObjectError::Layout)?;
        port.load(ctx, self, args, values)
    }

    /// Return whether a registered builtin may recursively evaluate forms.
    #[must_use]
    pub fn builtin_allows_nested_evaluation(&self, function: crate::FunctionObject) -> bool {
        let Some(function_word) = self.heap.forwarded_word(function.as_word()) else {
            return false;
        };
        self.builtins
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .find(|entry| *entry.function == function_word)
            .is_some_and(|entry| entry.implementation.nested_evaluation)
    }

    /// Return the native entry shared by builtins with no ISA-specific fast
    /// path, or `0` before [`Runtime::install_generic_builtin_entry`] runs.
    pub(crate) fn generic_builtin_entry(&self) -> usize {
        self.generic_builtin_entry.load(Ordering::Relaxed)
    }

    /// Install the shared native trampoline used as the default `ENTRY` for
    /// builtins registered without an ISA-specific fast path (see
    /// [`crate::BuiltinImplementation::with_entry`]).
    ///
    /// A caller above this layer (which alone knows how to publish executable
    /// code and reach back into the running `ThreadContext`/`Runtime` pair
    /// from native code) builds the trampoline and calls this once, early in
    /// its own construction. Builtins registered *before* this call (today,
    /// only [`Runtime::register_keyword_builtins`]'s own bootstrap
    /// registrations) already baked the unset `0` sentinel into their
    /// function object, so this also patches those objects' `ENTRY` slot in
    /// place.
    ///
    /// # Errors
    /// Returns an allocation or layout error while patching an
    /// already-registered builtin's `ENTRY` slot.
    pub fn install_generic_builtin_entry(
        &self,
        ctx: &mut ThreadContext,
        address: usize,
    ) -> Result<(), ObjectError> {
        self.generic_builtin_entry.store(address, Ordering::Relaxed);
        let pending: Vec<Word> = self
            .builtins
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter(|entry| entry.implementation.entry == 0)
            .map(|entry| *entry.function)
            .collect();
        let entry_word = crate::object_access::fix(address)?;
        for function_word in pending {
            crate::object_access::put(
                ctx,
                function_word,
                crate::function_offset::ENTRY,
                entry_word,
            )?;
        }
        Ok(())
    }

    /// Register the compiler's keyword-argument helper builtins.
    ///
    /// These helpers are object-runtime operations rather than user-facing
    /// Common Lisp functions.  They are registered in `NCL-EXT` so the native
    /// runtime and the safe builtin caller use the same implementation.
    ///
    /// # Errors
    /// Returns an allocation, layout, or storage error.
    pub fn register_keyword_builtins(&self, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
        let registrations = [
            (
                BuiltinName::new("MAKE-REST-LIST"),
                BuiltinImplementation::direct(MAKE_REST_LIST_DESCRIPTOR, make_rest_list_builtin),
            ),
            (
                BuiltinName::new("CHECK-KEYWORDS"),
                BuiltinImplementation::direct(CHECK_KEYWORDS_DESCRIPTOR, check_keywords_builtin),
            ),
            (
                BuiltinName::new("KEYWORD-VALUE"),
                BuiltinImplementation::direct(KEYWORD_VALUE_DESCRIPTOR, keyword_value_builtin),
            ),
            (
                BuiltinName::new("KEYWORD-SUPPLIED-P"),
                BuiltinImplementation::direct(KEYWORD_VALUE_DESCRIPTOR, keyword_supplied_p_builtin),
            ),
        ];
        for (name, implementation) in registrations {
            self.register_builtin(
                ctx,
                BuiltinIdentifier::new(BuiltinPackage::NclExt, name),
                implementation,
            )?;
        }
        Ok(())
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

    /// Register a generalized-reference expander owned by this runtime.
    ///
    /// The operator must be a symbol. The callback is copied out of the
    /// registry before invocation, so no registry lock is held while Lisp
    /// objects or another registry may be touched.
    ///
    /// # Errors
    /// Returns [`ObjectError::TypeError`] when `operator` is not a symbol.
    pub fn register_place_expander(
        &self,
        ctx: &ThreadContext,
        operator: Word,
        expander: PlaceExpander,
    ) -> Result<(), ObjectError> {
        let operator = crate::place::SymbolId::from_symbol(ctx, operator)?;
        let mut expanders = self
            .place_expanders
            .lock()
            .map_err(|_| ObjectError::Storage(StorageCondition::ThreadNotRegistered))?;
        expanders.register(&self.heap, operator, expander);
        drop(expanders);
        Ok(())
    }

    /// Look up a generalized-reference expander without invoking it.
    ///
    /// # Errors
    /// Returns [`ObjectError::TypeError`] when `operator` is not a symbol.
    pub fn place_expander(
        &self,
        ctx: &ThreadContext,
        operator: Word,
    ) -> Result<Option<PlaceExpander>, ObjectError> {
        let operator = crate::place::SymbolId::from_symbol(ctx, operator)?;
        let expanders = self
            .place_expanders
            .lock()
            .map_err(|_| ObjectError::Storage(StorageCondition::ThreadNotRegistered))?;
        Ok(expanders.get(operator))
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

    pub fn lisp_error_converter(&self) -> Option<LispErrorConverter> {
        self.lisp_error_converter
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .copied()
    }
}
