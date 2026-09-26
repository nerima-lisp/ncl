//! The object-layer port for calling Lisp functions from Rust builtins.

use crate::typed::FunctionDesignator;
use crate::{FunctionObject, MultipleValues, ObjectError, Runtime, ThreadContext, Word};

/// A GC-safe borrowed sequence of Lisp arguments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FunctionArguments<'a> {
    words: &'a [Word],
}

impl<'a> FunctionArguments<'a> {
    /// Create an argument sequence from a caller-owned slice.
    #[must_use]
    pub const fn new(words: &'a [Word]) -> Self {
        Self { words }
    }

    /// Return the number of arguments.
    #[must_use]
    pub const fn len(self) -> usize {
        self.words.len()
    }

    /// Return whether the sequence is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.words.is_empty()
    }

    /// Return the checked argument view at `index`.
    #[must_use]
    pub fn get(self, index: usize) -> Option<Word> {
        self.words.get(index).copied()
    }

    /// Expose the borrowed ABI words only at the adapter boundary.
    #[must_use]
    pub const fn as_slice(self) -> &'a [Word] {
        self.words
    }
}

/// Calls a Lisp function while retaining the runtime-specific implementation
/// outside `ncl-object`.
pub trait FunctionCaller {
    /// Invoke a function designator and write all returned values to `values`.
    ///
    /// The caller must keep `designator` and every word in `args` rooted for
    /// the complete call. Implementations must return a pending non-local exit
    /// as [`ObjectError::NonLocalExit`] after preserving the thread's unwind
    /// state; Rust must not unwind through generated Lisp frames.
    ///
    /// # Errors
    /// Returns an object-layer error when the designator is not callable or
    /// the callee signals a pending non-local exit.
    fn call_function(
        &mut self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        designator: FunctionDesignator,
        args: FunctionArguments<'_>,
        values: &mut MultipleValues,
    ) -> Result<Word, ObjectError>;
}

/// The object-layer caller used when the target is a registered Rust builtin.
#[derive(Debug, Default)]
pub struct BuiltinFunctionCaller;

impl FunctionCaller for BuiltinFunctionCaller {
    fn call_function(
        &mut self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        designator: FunctionDesignator,
        args: FunctionArguments<'_>,
        values: &mut MultipleValues,
    ) -> Result<Word, ObjectError> {
        let function = match designator {
            FunctionDesignator::Function(function) => function,
            FunctionDesignator::Symbol(symbol) => {
                let word = crate::symbol_function(ctx, symbol.into())?;
                FunctionObject::try_from(word).map_err(|_| ObjectError::UndefinedFunction)?
            }
        };
        runtime.call_builtin_with_caller(ctx, self, function, args.as_slice(), values)
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
