//! Calling convention metadata and the safe Rust builtin boundary.

use crate::{
    LispError, ObjectError, Package, Runtime, ThreadContext, make_code_object, make_simple_fun,
    with_root,
};
use ncl_sys::Word;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum NclStatus {
    Ok = 0,
    Error = 1,
    NonLocalExit = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParameterType {
    Any,
    Fixnum,
    Integer,
    Number,
    List,
    Sequence,
    StringDesignator,
    FunctionDesignator,
    PackageDesignator,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Parameter {
    pub name: BuiltinName,
    pub ty: ParameterType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LambdaList {
    pub required: &'static [Parameter],
    pub optional: &'static [Parameter],
    pub rest: Option<Parameter>,
    pub keys: &'static [Parameter],
    pub allow_other_keys: bool,
}

impl LambdaList {
    #[must_use]
    pub const fn new(
        required: &'static [Parameter],
        optional: &'static [Parameter],
        rest: Option<Parameter>,
        keys: &'static [Parameter],
        allow_other_keys: bool,
    ) -> Self {
        Self {
            required,
            optional,
            rest,
            keys,
            allow_other_keys,
        }
    }
    #[must_use]
    pub const fn min_arity(self) -> usize {
        self.required.len()
    }
    #[must_use]
    pub const fn max_arity(self) -> Option<usize> {
        if self.rest.is_some() {
            None
        } else {
            Some(self.required.len() + self.optional.len() + self.keys.len() * 2)
        }
    }
    #[must_use]
    pub const fn is_direct(self) -> bool {
        self.optional.is_empty() && self.rest.is_none() && self.keys.is_empty()
    }
    #[must_use]
    pub const fn fixed(required: &'static [Parameter]) -> Self {
        Self::new(required, &[], None, &[], false)
    }
    #[must_use]
    pub const fn with_optional(
        required: &'static [Parameter],
        optional: &'static [Parameter],
    ) -> Self {
        Self::new(required, optional, None, &[], false)
    }
    #[must_use]
    pub const fn with_rest(required: &'static [Parameter], rest: Parameter) -> Self {
        Self::new(required, &[], Some(rest), &[], false)
    }
    #[must_use]
    pub const fn with_keys(
        required: &'static [Parameter],
        keys: &'static [Parameter],
        allow_other_keys: bool,
    ) -> Self {
        Self::new(required, &[], None, keys, allow_other_keys)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arity(u8);

impl Arity {
    #[must_use]
    pub const fn exact(value: u8) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuiltinConvention {
    Direct(Arity),
    Adapted,
}

impl BuiltinConvention {
    #[must_use]
    pub const fn direct(self) -> bool {
        matches!(self, Self::Direct(_))
    }

    #[must_use]
    pub const fn arity(self) -> Option<Arity> {
        match self {
            Self::Direct(arity) => Some(arity),
            Self::Adapted => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Builtin {
    pub lambda_list: LambdaList,
    pub convention: BuiltinConvention,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum BuiltinPackage {
    CommonLisp,
    AsdfInterface,
    UiopDriver,
    NclThreads,
    NclFfi,
    NclMop,
    NclGray,
    NclGc,
    NclImage,
    NclUnicode,
    NclOs,
    NclExt,
    NclSys,
    NclTest,
}

impl BuiltinPackage {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CommonLisp => "COMMON-LISP",
            Self::AsdfInterface => "ASDF/INTERFACE",
            Self::UiopDriver => "UIOP/DRIVER",
            Self::NclThreads => "NCL-THREADS",
            Self::NclFfi => "NCL-FFI",
            Self::NclMop => "NCL-MOP",
            Self::NclGray => "NCL-GRAY",
            Self::NclGc => "NCL-GC",
            Self::NclImage => "NCL-IMAGE",
            Self::NclUnicode => "NCL-UNICODE",
            Self::NclOs => "NCL-OS",
            Self::NclExt => "NCL-EXT",
            Self::NclSys => "NCL-SYS",
            Self::NclTest => "NCL-TEST",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BuiltinName(&'static str);

impl BuiltinName {
    #[must_use]
    ///
    /// # Panics
    ///
    /// Panics during const evaluation when the name is empty or non-ASCII.
    pub const fn new(value: &'static str) -> Self {
        assert!(value.is_ascii() && !value.is_empty());
        Self(value)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BuiltinIdentifier {
    pub package: BuiltinPackage,
    pub name: BuiltinName,
}

impl BuiltinIdentifier {
    #[must_use]
    pub const fn new(package: BuiltinPackage, name: BuiltinName) -> Self {
        Self { package, name }
    }
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct MultipleValues {
    values: Vec<Word>,
}
impl MultipleValues {
    #[must_use]
    pub const fn new() -> Self {
        Self { values: Vec::new() }
    }
    pub fn clear(&mut self) {
        self.values.clear();
    }
    pub fn set(&mut self, values: &[Word]) {
        self.values = values.to_vec();
    }
    pub fn push(&mut self, value: Word) {
        self.values.push(value);
    }
    #[must_use]
    pub fn as_slice(&self) -> &[Word] {
        &self.values
    }
    #[must_use]
    pub const fn len(&self) -> usize {
        self.values.len()
    }
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FunctionObject(Word);
impl TryFrom<Word> for FunctionObject {
    type Error = ObjectError;
    fn try_from(value: Word) -> Result<Self, Self::Error> {
        if value == Word::UNBOUND || value.lowtag() != ncl_sys::LowTag::OtherPointer as u8 {
            return Err(ObjectError::TypeError);
        }
        Ok(Self(value))
    }
}
impl From<FunctionObject> for Word {
    fn from(value: FunctionObject) -> Self {
        value.0
    }
}
impl FunctionObject {
    #[must_use]
    pub const fn as_word(self) -> Word {
        self.0
    }
    #[must_use]
    pub fn is_unbound(self) -> bool {
        self.0 == Word::UNBOUND
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BuiltinArgs<'a> {
    words: &'a [Word],
}

impl<'a> BuiltinArgs<'a> {
    #[must_use]
    pub const fn new(words: &'a [Word]) -> Self {
        Self { words }
    }

    #[must_use]
    pub const fn len(self) -> usize {
        self.words.len()
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.words.is_empty()
    }

    #[must_use]
    pub fn get(self, index: usize) -> Option<Word> {
        self.words.get(index).copied()
    }

    /// Return a required argument.
    ///
    /// # Errors
    ///
    /// Returns [`ObjectError::TypeError`] when the argument is absent.
    pub fn required(self, index: usize) -> Result<Word, ObjectError> {
        self.get(index).ok_or(ObjectError::TypeError)
    }
}

pub type RustBuiltin = fn(
    &mut ThreadContext,
    &Runtime,
    &BuiltinArgs<'_>,
    &mut MultipleValues,
) -> Result<Word, ObjectError>;
pub type LispErrorConverter = fn(
    &mut ThreadContext,
    &Runtime,
    LispError,
) -> Result<Word, ObjectError>;
pub type KeywordAdapter = fn(&BuiltinArgs<'_>) -> Result<Vec<Word>, super::ObjectError>;
pub type RegisterFn = fn(&super::Runtime);

#[derive(Clone, Copy, Debug)]
pub struct BuiltinImplementation {
    pub descriptor: Builtin,
    /// Native entry address stored in the function object's ENTRY slot.
    pub entry: usize,
    pub function: RustBuiltin,
    pub keyword_adapter: Option<KeywordAdapter>,
}

impl Runtime {
    /// Register a safe Rust builtin and install its function object in a symbol cell.
    ///
    /// # Errors
    ///
    /// Returns an allocation, layout, or storage error.
    pub fn register_builtin(
        &self,
        ctx: &mut ThreadContext,
        identifier: BuiltinIdentifier,
        implementation: BuiltinImplementation,
    ) -> Result<FunctionObject, ObjectError> {
        let package = identifier.package.as_str();
        let name = identifier.name.as_str();
        let package_word = self.ensure_package(ctx, package)?;
        let (mut symbol, _) = Package::from_word(package_word).intern(ctx, self, name)?;
        with_root(ctx, &mut symbol, |ctx, symbol| {
            let lambda_list = crate::make_string(
                ctx,
                self,
                &implementation
                    .descriptor
                    .lambda_list
                    .required
                    .iter()
                    .map(|parameter| parameter.name.as_str())
                    .collect::<String>()
                    .chars()
                    .collect::<Vec<_>>(),
            )?;
            let code = make_code_object(ctx, self, 0, 0, Word::NIL, Word::NIL, Word::NIL)?;
            let function =
                make_simple_fun(ctx, self, implementation.entry, *symbol, lambda_list, code)?;
            let mut function_word = function.as_word();
            with_root(ctx, &mut function_word, |ctx, function_word| {
                ctx.write_object_slot(
                    *symbol,
                    crate::layout::symbol_offset::FUNCTION,
                    *function_word,
                )?;
                self.define_function(ctx, package, name, *function_word)?;
                self.builtins
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(*function_word, implementation);
                FunctionObject::try_from(*function_word)
            })
        })
    }

    /// Call a registered builtin through the safe Rust boundary.
    ///
    /// # Errors
    ///
    /// Returns an arity, pending-condition, or builtin callback error.
    pub fn call_builtin(
        &self,
        ctx: &mut ThreadContext,
        function: FunctionObject,
        args: &[Word],
    ) -> Result<Word, ObjectError> {
        if function.is_unbound() {
            return Err(ObjectError::Unbound);
        }
        let implementation = self
            .builtins
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&function.as_word())
            .copied()
            .ok_or(ObjectError::Unbound)?;
        if args.len() < implementation.descriptor.lambda_list.min_arity()
            || implementation
                .descriptor
                .lambda_list
                .max_arity()
                .is_some_and(|max| args.len() > max)
        {
            return Err(ObjectError::TypeError);
        }
        let original = args.to_vec();
        let args = BuiltinArgs::new(&original);
        let adapted = if let Some(adapter) = implementation.keyword_adapter {
            adapter(&args)?
        } else {
            original
        };
        let mut values = MultipleValues::new();
        let adapted = BuiltinArgs::new(&adapted);
        let result = (implementation.function)(ctx, self, &adapted, &mut values);
        ctx.set_values(values.as_slice());
        if let Some(error) = ctx.take_pending_lisp_error() {
            if let Some(converter) = self.lisp_error_converter() {
                let condition = converter(ctx, self, error)?;
                ctx.set_pending_condition(condition);
            }
        }
        let pending = ctx.take_pending();
        pending.map_or(result, Err)
    }

    /// Return the descriptor installed for a function object.
    #[must_use]
    pub fn builtin_descriptor(&self, function: FunctionObject) -> Option<Builtin> {
        self.builtins
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&function.as_word())
            .map(|entry| entry.descriptor)
    }
}
impl BuiltinImplementation {
    #[must_use]
    pub fn direct(descriptor: Builtin, function: RustBuiltin) -> Self {
        Self {
            descriptor,
            entry: function as usize,
            function,
            keyword_adapter: None,
        }
    }
    #[must_use]
    pub fn adapted(descriptor: Builtin, function: RustBuiltin, adapter: KeywordAdapter) -> Self {
        Self {
            descriptor,
            entry: function as usize,
            function,
            keyword_adapter: Some(adapter),
        }
    }
    /// Override the native entry address while keeping the safe Rust callback.
    #[must_use]
    pub const fn with_entry(self, entry: usize) -> Self {
        Self { entry, ..self }
    }
}

#[macro_export]
macro_rules! builtin {
    ($name:ident, $arity:expr) => { pub const $name: $crate::Builtin = $crate::Builtin { lambda_list: $crate::LambdaList::new(&[], &[], None, &[], false), convention: $crate::BuiltinConvention::Direct($crate::Arity::exact($arity)) }; };
    ($name:ident, $arity:expr, $lambda_list:expr) => { pub const $name: $crate::Builtin = $crate::Builtin { lambda_list: $lambda_list, convention: $crate::BuiltinConvention::Direct($crate::Arity::exact($arity)) }; };
    ($name:ident, 0, $lambda_list:expr, $direct:ident, $variadic:ident) => { $crate::builtin!(@descriptor $name, 0, $lambda_list); pub extern "C" fn $direct(_ctx: *mut $crate::ThreadContext) -> $crate::Word { $crate::Word::UNBOUND } $crate::builtin!(@variadic $variadic); };
    ($name:ident, 1, $lambda_list:expr, $direct:ident, $variadic:ident) => { $crate::builtin!(@descriptor $name, 1, $lambda_list); pub extern "C" fn $direct(_ctx: *mut $crate::ThreadContext, _a0: $crate::Word) -> $crate::Word { $crate::Word::UNBOUND } $crate::builtin!(@variadic $variadic); };
    ($name:ident, 2, $lambda_list:expr, $direct:ident, $variadic:ident) => { $crate::builtin!(@descriptor $name, 2, $lambda_list); pub extern "C" fn $direct(_ctx: *mut $crate::ThreadContext, _a0: $crate::Word, _a1: $crate::Word) -> $crate::Word { $crate::Word::UNBOUND } $crate::builtin!(@variadic $variadic); };
    ($name:ident, 3, $lambda_list:expr, $direct:ident, $variadic:ident) => { $crate::builtin!(@descriptor $name, 3, $lambda_list); pub extern "C" fn $direct(_ctx: *mut $crate::ThreadContext, _a0: $crate::Word, _a1: $crate::Word, _a2: $crate::Word) -> $crate::Word { $crate::Word::UNBOUND } $crate::builtin!(@variadic $variadic); };
    ($name:ident, 4, $lambda_list:expr, $direct:ident, $variadic:ident) => { $crate::builtin!(@descriptor $name, 4, $lambda_list); pub extern "C" fn $direct(_ctx: *mut $crate::ThreadContext, _a0: $crate::Word, _a1: $crate::Word, _a2: $crate::Word, _a3: $crate::Word) -> $crate::Word { $crate::Word::UNBOUND } $crate::builtin!(@variadic $variadic); };
    (@descriptor $name:ident, $arity:expr, $lambda_list:expr) => { pub const $name: $crate::Builtin = $crate::Builtin { lambda_list: $lambda_list, convention: $crate::BuiltinConvention::Direct($crate::Arity::exact($arity)) }; };
    (@variadic $name:ident) => { pub extern "C" fn $name(_ctx: *mut $crate::ThreadContext, _argc: usize, _args: *const $crate::Word, _values: *mut $crate::MultipleValues) -> $crate::NclStatus { $crate::NclStatus::Error };
    };
}
