//! The object-layer port for calling Lisp functions from Rust builtins.

use crate::typed::{FunctionDesignator, LispError, ProgramError};
use crate::{
    Function, FunctionObject, MultipleValues, ObjectError, Runtime, ThreadContext, Word,
    function_entry,
};
use ncl_sys::invoke_entry_with_function_address;

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
        let designator_word = match designator {
            FunctionDesignator::Function(function) => function.as_word(),
            FunctionDesignator::Symbol(symbol) => symbol.into(),
        };
        let mut rooted_values = Vec::with_capacity(args.len() + 1);
        rooted_values.push(designator_word);
        rooted_values.extend_from_slice(args.as_slice());
        crate::with_roots(ctx, &rooted_values, |ctx, rooted_values| {
            let function = match designator {
                FunctionDesignator::Function(_) => FunctionObject::try_from(*rooted_values[0])
                    .map_err(|_| ObjectError::UndefinedFunction)?,
                FunctionDesignator::Symbol(_) => {
                    let word = crate::symbol_function(ctx, *rooted_values[0])?;
                    FunctionObject::try_from(word).map_err(|_| ObjectError::UndefinedFunction)?
                }
            };
            let current_args: Vec<Word> = rooted_values[1..].iter().map(|value| **value).collect();
            let result = if runtime.builtin_descriptor(function).is_some() {
                runtime.call_builtin(ctx, function, &current_args)?
            } else {
                call_native(ctx, function, &current_args, values)?
            };
            values.set(ctx.values());
            Ok(result)
        })
    }
}

fn call_native(
    ctx: &mut ThreadContext,
    function: FunctionObject,
    args: &[Word],
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() > 4 {
        return Err(ObjectError::TypeError);
    }
    let entry = function_entry(ctx, Function::from_word(function.as_word()))?;
    if entry == 0 {
        return Err(ObjectError::UndefinedFunction);
    }
    let mut registers = [0_u64; 4];
    for (register, argument) in args.iter().take(4).enumerate() {
        registers[register] = argument.bits();
    }
    let rest = args
        .get(4..)
        .filter(|rest| !rest.is_empty())
        .map_or(0, |rest| rest.as_ptr() as usize as u64);
    let (result, count) = invoke_entry_with_function_address(
        entry,
        ctx.thread_mut() as *mut ncl_sys::Thread,
        function.as_word().bits(),
        args.len() as u64,
        registers,
        rest,
    );
    if count > 1 {
        return Err(ObjectError::TypeError);
    }
    values.clear();
    Ok(Word::from_bits(result))
}

impl Runtime {
    pub(crate) fn call_registered_builtin(
        &self,
        ctx: &mut ThreadContext,
        function: FunctionObject,
        args: &[Word],
        values: &mut MultipleValues,
    ) -> Result<Word, ObjectError> {
        if function.is_unbound() {
            return Err(ObjectError::Unbound);
        }
        let mut rooted_values = Vec::with_capacity(args.len() + 1);
        rooted_values.push(function.as_word());
        rooted_values.extend_from_slice(args);
        let result = crate::with_roots(ctx, &rooted_values, |ctx, rooted_values| {
            let function_word = *rooted_values[0];
            let implementation = self
                .builtins
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(&function_word)
                .copied()
                .ok_or(ObjectError::Unbound)?;
            if rooted_values.len() - 1 < implementation.descriptor.lambda_list.min_arity()
                || implementation
                    .descriptor
                    .lambda_list
                    .max_arity()
                    .is_some_and(|max| rooted_values.len() - 1 > max)
            {
                ctx.set_pending_lisp_error(LispError::ProgramError(
                    ProgramError::WrongNumberOfArguments {
                        minimum: implementation.descriptor.lambda_list.min_arity(),
                        maximum: implementation.descriptor.lambda_list.max_arity(),
                    },
                ));
                return Ok(Err(ObjectError::TypeError));
            }
            let original: Vec<Word> = rooted_values[1..].iter().map(|value| **value).collect();
            let args = crate::BuiltinArgs::from_rooted(&original, &rooted_values[1..]);
            let adapted = if let Some(adapter) = implementation.keyword_adapter {
                adapter(&args)?
            } else {
                original
            };
            crate::with_roots(ctx, &adapted, |ctx, rooted_adapted| {
                let callback_args: Vec<Word> = rooted_adapted.iter().map(|value| **value).collect();
                let callback_args = crate::BuiltinArgs::from_rooted(&callback_args, rooted_adapted);
                (implementation.function)(ctx, self, &callback_args, values)
            })
            .map(Ok)
        })?;
        ctx.set_values(values.as_slice());
        if let Some(error) = ctx.take_pending_lisp_error()
            && let Some(converter) = self.lisp_error_converter()
        {
            let condition = converter(ctx, self, error)?;
            ctx.set_pending_condition(condition);
        }
        let pending = ctx.take_pending();
        pending.map_or(result, Err)
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
