//! Shared runtime heap and registries.

use crate::hash_table::{HashTable, HashTest, Weakness};
use crate::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, LambdaList, LispError, LispErrorConverter, ObjectError, ObjectRef,
    Parameter, ParameterType, PlaceExpander, ProgramError, ThreadContext, Word, car, cdr,
    classify_object, make_cons, make_string, string_length, string_ref, symbol_name,
    symbol_package, with_root, with_roots,
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
    pub(crate) builtins: Mutex<Vec<crate::builtin::BuiltinEntry>>,
    pub(crate) builtin_addresses: Mutex<HashMap<BuiltinIdentifier, usize>>,
    pub(crate) place_expanders: Mutex<crate::place::PlaceExpanders>,
    lisp_error_converter: Mutex<Option<LispErrorConverter>>,
}
/// Per-mutator object-layer context. Generated code obtains its stable thread
/// pointer with [`ThreadContext::thread_mut`].
#[derive(Debug)]
pub struct RootedTable {
    slot: Box<Word>,
    _token: RootToken,
}

const KEYWORD_LIST: Parameter = Parameter {
    name: BuiltinName::new("LIST"),
    ty: ParameterType::Any,
};
const KEYWORD: Parameter = Parameter {
    name: BuiltinName::new("KEYWORD"),
    ty: ParameterType::Any,
};
const ALLOW_OTHER_KEYS: Parameter = Parameter {
    name: BuiltinName::new("ALLOW-OTHER-KEYS"),
    ty: ParameterType::Any,
};
const ALLOWED_KEYWORD: Parameter = Parameter {
    name: BuiltinName::new("ALLOWED-KEYWORD"),
    ty: ParameterType::Any,
};
const CHECK_KEYWORDS_DESCRIPTOR: Builtin = Builtin {
    lambda_list: LambdaList::new(
        &[KEYWORD_LIST, ALLOW_OTHER_KEYS],
        &[],
        Some(ALLOWED_KEYWORD),
        &[],
        false,
    ),
    convention: BuiltinConvention::Direct(Arity::exact(2)),
};
const REST_LIST_REQUIRED: &[Parameter] = &[
    Parameter {
        name: BuiltinName::new("ARGC"),
        ty: ParameterType::Fixnum,
    },
    Parameter {
        name: BuiltinName::new("START"),
        ty: ParameterType::Fixnum,
    },
];
const REST_LIST_VALUE: Parameter = Parameter {
    name: BuiltinName::new("VALUE"),
    ty: ParameterType::Any,
};
const MAKE_REST_LIST_DESCRIPTOR: Builtin = Builtin {
    lambda_list: LambdaList::with_rest(REST_LIST_REQUIRED, REST_LIST_VALUE),
    convention: BuiltinConvention::Adapted,
};
const KEYWORD_VALUE_DESCRIPTOR: Builtin = Builtin {
    lambda_list: LambdaList::new(&[KEYWORD_LIST, KEYWORD], &[], None, &[], false),
    convention: BuiltinConvention::Direct(Arity::exact(2)),
};

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
            layouts_registered: Mutex::new(false),
            builtins: Mutex::new(Vec::new()),
            builtin_addresses: Mutex::new(HashMap::new()),
            place_expanders: Mutex::new(crate::place::PlaceExpanders::default()),
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
        context.ensure_standard_packages(&runtime)?;
        runtime.register_keyword_builtins(&mut context)?;
        Ok(runtime)
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

    pub(crate) fn lisp_error_converter(&self) -> Option<LispErrorConverter> {
        self.lisp_error_converter
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .copied()
    }
}

fn make_rest_list_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    builtin_args: &BuiltinArgs<'_>,
    _values: &mut crate::MultipleValues,
) -> Result<Word, ObjectError> {
    let argc = builtin_args
        .required(0)?
        .as_fixnum()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(ObjectError::TypeError)?;
    let start = builtin_args
        .required(1)?
        .as_fixnum()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(ObjectError::TypeError)?;
    let values = builtin_args
        .as_slice()
        .get(2..)
        .ok_or(ObjectError::TypeError)?;
    if start > argc || argc > values.len() {
        return Err(ObjectError::TypeError);
    }
    with_roots(ctx, &values[..argc], |ctx, values| {
        let mut list = Word::NIL;
        with_root(ctx, &mut list, |ctx, list| {
            let mut list_word = *list;
            for value in values[start..].iter().rev() {
                list_word = make_cons(ctx, runtime, **value, list_word)?;
            }
            Ok(list_word)
        })
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KeywordEntriesError {
    Type,
    Odd,
}

impl KeywordEntriesError {
    const fn object_error() -> ObjectError {
        ObjectError::TypeError
    }
}

fn keyword_entries(
    ctx: &mut ThreadContext,
    list: Word,
) -> Result<Vec<(Word, Word)>, KeywordEntriesError> {
    let mut entries = Vec::new();
    let mut cursor = list;
    while cursor != Word::NIL {
        let key = car(ctx, cursor).map_err(|_| KeywordEntriesError::Type)?;
        let tail = cdr(ctx, cursor).map_err(|_| KeywordEntriesError::Type)?;
        if tail == Word::NIL {
            return Err(KeywordEntriesError::Odd);
        }
        let value = car(ctx, tail).map_err(|_| KeywordEntriesError::Type)?;
        let next = cdr(ctx, tail).map_err(|_| KeywordEntriesError::Type)?;
        if matches!(classify_object(ctx, key), ObjectRef::Symbol(_)) {
            entries.push((key, value));
        } else {
            return Err(KeywordEntriesError::Type);
        }
        cursor = next;
    }
    Ok(entries)
}

fn is_allow_other_keys(
    ctx: &ThreadContext,
    runtime: &Runtime,
    symbol: Word,
) -> Result<bool, ObjectError> {
    let Some(keyword_package) = runtime.find_package(ctx, "KEYWORD") else {
        return Ok(false);
    };
    if symbol_package(ctx, symbol)? != keyword_package {
        return Ok(false);
    }
    let name = symbol_name(ctx, symbol)?;
    if string_length(ctx, name)? != "ALLOW-OTHER-KEYS".len() {
        return Ok(false);
    }
    Ok("ALLOW-OTHER-KEYS"
        .chars()
        .enumerate()
        .all(|(index, expected)| string_ref(ctx, name, index) == Ok(expected)))
}

fn check_keywords_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut crate::MultipleValues,
) -> Result<Word, ObjectError> {
    let list = args.required(0)?;
    let lambda_allows_other_keys = args.required(1)? != Word::NIL;
    let entries = match keyword_entries(ctx, list) {
        Ok(entries) => entries,
        Err(KeywordEntriesError::Odd) => {
            ctx.set_pending_lisp_error(LispError::ProgramError(ProgramError::OddKeywordArguments));
            return Err(ObjectError::TypeError);
        }
        Err(_error) => return Err(KeywordEntriesError::object_error()),
    };
    let mut call_allows_other_keys = false;
    for (keyword, value) in &entries {
        if is_allow_other_keys(ctx, runtime, *keyword)? && *value != Word::NIL {
            call_allows_other_keys = true;
        }
    }
    if !lambda_allows_other_keys && !call_allows_other_keys {
        let allowed = &args.as_slice()[2..];
        for (keyword, _) in &entries {
            if !is_allow_other_keys(ctx, runtime, *keyword)? && !allowed.contains(keyword) {
                ctx.set_pending_lisp_error(LispError::ProgramError(ProgramError::UnknownKeyword));
                return Err(ObjectError::TypeError);
            }
        }
    }
    Ok(Word::NIL)
}

fn keyword_value_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut crate::MultipleValues,
) -> Result<Word, ObjectError> {
    let list = args.required(0)?;
    let keyword = args.required(1)?;
    keyword_entries(ctx, list)
        .map_err(|_| KeywordEntriesError::object_error())?
        .into_iter()
        .find(|(candidate, _)| *candidate == keyword)
        .map_or(Ok(Word::NIL), |(_, value)| Ok(value))
}

fn keyword_supplied_p_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut crate::MultipleValues,
) -> Result<Word, ObjectError> {
    let list = args.required(0)?;
    let keyword = args.required(1)?;
    Ok(
        if keyword_entries(ctx, list)
            .map_err(|_| KeywordEntriesError::object_error())?
            .into_iter()
            .any(|(candidate, _)| candidate == keyword)
        {
            Word::TRUE
        } else {
            Word::NIL
        },
    )
}
