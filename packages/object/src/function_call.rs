//! The object-layer port for calling Lisp functions from Rust builtins.

mod builtin_execution;
mod publish_values;
use crate::typed::{FunctionDesignator, LispError, ProgramError};
use crate::{
    Function, FunctionObject, MultipleValues, ObjectError, Runtime, ThreadContext, Word,
    function_entry, symbol_is_macro,
};
use builtin_execution::call_nested_builtin;
use publish_values::publish_builtin_values;

impl crate::BuiltinImplementation {
    /// `entry` starts at the `0` sentinel; registration resolves it to the shared trampoline.
    #[must_use]
    pub fn direct(descriptor: crate::Builtin, function: crate::RustBuiltin) -> Self {
        Self {
            descriptor,
            entry: 0,
            function,
            keyword_adapter: None,
            nested_evaluation: false,
        }
    }
    /// Construct an adapted builtin using the shared trampoline.
    #[must_use]
    pub fn adapted(
        descriptor: crate::Builtin,
        function: crate::RustBuiltin,
        adapter: crate::KeywordAdapter,
    ) -> Self {
        Self {
            descriptor,
            entry: 0,
            function,
            keyword_adapter: Some(adapter),
            nested_evaluation: false,
        }
    }
    /// Override the native entry address.
    #[must_use]
    pub const fn with_entry(self, entry: usize) -> Self {
        Self { entry, ..self }
    }
    /// Permit a builtin to evaluate forms while it is running.
    #[must_use]
    pub const fn with_nested_evaluation(self) -> Self {
        Self {
            nested_evaluation: true,
            ..self
        }
    }
}

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
        crate::with_heap_rooted_mut_slice(runtime, &rooted_values, |rooted_values| {
            let function = match designator {
                FunctionDesignator::Function(_) => {
                    FunctionObject::try_from(*rooted_values.first().ok_or(ObjectError::Layout)?)
                        .map_err(|_| ObjectError::UndefinedFunction)?
                }
                FunctionDesignator::Symbol(_) => {
                    let designator = rooted_values.first().ok_or(ObjectError::Layout)?;
                    if symbol_is_macro(ctx, *designator)? {
                        return Err(ObjectError::UndefinedFunction);
                    }
                    let word = crate::symbol_function(ctx, *designator)?;
                    FunctionObject::try_from(word).map_err(|_| ObjectError::UndefinedFunction)?
                }
            };
            let current_args = rooted_values.get_mut(1..).ok_or(ObjectError::Layout)?;
            let result = if let Some(result) =
                runtime.call_builtin_if_registered(ctx, function, current_args, values)
            {
                result?
            } else {
                call_native(ctx, runtime, function, current_args, values)?
            };
            if values.is_explicit() {
                ctx.set_values(values.as_slice());
            } else {
                ctx.set_values(std::slice::from_ref(&result));
            }
            Ok(result)
        })
    }
}

fn call_native(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    function: FunctionObject,
    args: &mut [Word],
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let entry = function_entry(ctx, Function::from_word(function.as_word()))?;
    if entry == 0 {
        return Err(ObjectError::Layout);
    }
    let mut registers = [0_u64; 4];
    for (register, argument) in args.iter().take(4).enumerate() {
        registers[register] = argument.bits();
    }
    let argument_count =
        Word::fixnum(i64::try_from(args.len()).map_err(|_| ObjectError::Layout)?).bits();
    let rest = args
        .get_mut(4..)
        .filter(|rest| !rest.is_empty())
        .map_or(Ok(0), |rest| {
            u64::try_from(rest.as_mut_ptr().addr()).map_err(|_| ObjectError::Layout)
        })?;
    let (result, count) = ctx.thread_access().invoke_entry_with_function_address(
        entry,
        function.as_word().bits(),
        argument_count,
        registers,
        rest,
    );
    let pending_errors = ctx.take_pending_runtime_errors();
    let count = usize::try_from(count).map_err(|_| ObjectError::Layout)?;
    let non_local_exit = ctx.take_non_local_exit();
    if non_local_exit {
        let non_local_values = {
            let thread = ctx.thread_access();
            thread
                .multiple_values()
                .get(..thread.multiple_value_count())
                .ok_or(ObjectError::Layout)?
                .to_vec()
        };
        values.clear();
        values.set(&non_local_values);
        ctx.set_non_local_exit(true);
        return Err(ObjectError::NonLocalExit);
    }
    if pending_errors.native.is_some() {
        values.clear();
        return Err(ObjectError::Layout);
    }
    values.clear();
    let result = match count {
        0 => {
            values.set(&[]);
            Word::NIL
        }
        1 => {
            let value = Word::from_bits(result);
            values.set(&[value]);
            value
        }
        count => {
            let thread = ctx.thread_access();
            let area = thread.multiple_values();
            if area.len() < count {
                return Err(ObjectError::Layout);
            }
            values.set(&area[..count]);
            Word::from_bits(result)
        }
    };
    if let Some(error) = pending_errors.lisp
        && let Some(converter) = runtime.lisp_error_converter()
    {
        let converted = converter(ctx, runtime, error);
        let converted_errors = ctx.take_pending_runtime_errors();
        let condition = converted?;
        ctx.set_pending_condition(condition);
        if converted_errors.native.is_some() {
            return Err(ObjectError::Layout);
        }
        if let Some(error) = converted_errors.object {
            return Err(error);
        }
    }
    if let Some(error) = pending_errors.object {
        return Err(error);
    }
    Ok(result)
}

impl Runtime {
    pub(crate) fn call_registered_builtin(
        &self,
        ctx: &mut ThreadContext,
        function: FunctionObject,
        args: &[Word],
        values: &mut MultipleValues,
    ) -> Result<Word, ObjectError> {
        let implementation = self
            .registered_builtin(ctx, function)
            .map(|(_, implementation)| implementation);
        self.call_registered_builtin_with_implementation(
            ctx,
            function,
            args,
            values,
            implementation,
        )
    }

    pub(crate) fn call_registered_builtin_with_implementation(
        &self,
        ctx: &mut ThreadContext,
        function: FunctionObject,
        args: &[Word],
        values: &mut MultipleValues,
        implementation: Option<crate::BuiltinImplementation>,
    ) -> Result<Word, ObjectError> {
        let pending_errors = ctx.take_pending_runtime_errors();
        if pending_errors.native.is_some() {
            return Err(ObjectError::Layout);
        }
        if pending_errors.object.is_some() || pending_errors.lisp.is_some() {
            return Err(ObjectError::ControlError);
        }
        if function.is_unbound() {
            return Err(ObjectError::Unbound);
        }
        let implementation = implementation.ok_or(ObjectError::Unbound)?;
        values.clear();
        if implementation.nested_evaluation {
            return call_nested_builtin(ctx, self, args, values, implementation);
        }
        let mut rooted_values = Vec::with_capacity(args.len() + 1);
        rooted_values.push(function.as_word());
        rooted_values.extend_from_slice(args);
        let result = crate::with_roots(ctx, &rooted_values, |ctx, rooted_values| {
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
            Ok(crate::with_roots(ctx, &adapted, |ctx, rooted_adapted| {
                let callback_args: Vec<Word> = rooted_adapted.iter().map(|value| **value).collect();
                let callback_args = crate::BuiltinArgs::from_rooted(&callback_args, rooted_adapted);
                (implementation.function)(ctx, self, &callback_args, values)
            }))
        });
        let pending_errors = ctx.take_pending_runtime_errors();
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                values.clear();
                ctx.set_values(&[]);
                return Err(error);
            }
        };
        if pending_errors.native.is_some() {
            values.clear();
            ctx.set_values(&[]);
            return Err(ObjectError::Layout);
        }
        if let Some(error) = pending_errors.lisp
            && let Some(converter) = self.lisp_error_converter()
        {
            let converted = converter(ctx, self, error);
            let converted_errors = ctx.take_pending_runtime_errors();
            let condition = converted?;
            ctx.set_pending_condition(condition);
            if converted_errors.native.is_some() {
                return Err(ObjectError::Layout);
            }
            if let Some(error) = converted_errors.object {
                values.clear();
                ctx.set_values(&[]);
                return Err(error);
            }
        }
        let result = pending_errors.object.map_or(result, Err);
        publish_builtin_values(ctx, &result, values);
        result
    }

    /// Call a registered builtin from generated code. Generated code checks the pending-error
    /// flag; the enclosing Rust boundary consumes the thread's pending error payloads after
    /// generated code returns.
    ///
    /// # Errors
    ///
    /// Returns an `ObjectError` produced while resolving or invoking the builtin.
    pub fn call_builtin_from_generated_code(
        &self,
        ctx: &mut ThreadContext,
        function: FunctionObject,
        args: &[Word],
    ) -> Result<Word, ObjectError> {
        self.call_registered_builtin(ctx, function, args, &mut MultipleValues::new())
    }
}

#[macro_export]
macro_rules! builtin {
    ($name:ident, $arity:expr) => {
        pub const $name: $crate::Builtin = $crate::Builtin {
            lambda_list: $crate::LambdaList::new(&[], &[], None, &[], false),
            convention: $crate::BuiltinConvention::Direct($crate::Arity::exact($arity)),
        };
    };
    ($name:ident, $arity:expr, $lambda_list:expr) => {
        pub const $name: $crate::Builtin = $crate::Builtin {
            lambda_list: $lambda_list,
            convention: $crate::BuiltinConvention::Direct($crate::Arity::exact($arity)),
        };
    };
    (@descriptor $name:ident, $arity:expr, $lambda_list:expr) => {
        pub const $name: $crate::Builtin = $crate::Builtin {
            lambda_list: $lambda_list,
            convention: $crate::BuiltinConvention::Direct($crate::Arity::exact($arity)),
        };
    };
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn unused_builtin(
        _ctx: &mut ThreadContext,
        _runtime: &Runtime,
        _args: &crate::BuiltinArgs<'_>,
        _values: &mut MultipleValues,
    ) -> Result<Word, ObjectError> {
        Err(ObjectError::Unsupported)
    }

    #[test]
    fn registered_builtin_drains_pending_payloads_before_returning() {
        let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
        let mut context = ThreadContext::new();
        context
            .register(&runtime)
            .unwrap_or_else(|error| panic!("register context: {error:?}"));
        let descriptor = crate::Builtin {
            lambda_list: crate::LambdaList::new(&[], &[], None, &[], false),
            convention: crate::BuiltinConvention::Direct(crate::Arity::exact(0)),
        };
        let function = runtime
            .register_builtin(
                &mut context,
                crate::BuiltinIdentifier::new(
                    crate::BuiltinPackage::NclTest,
                    crate::BuiltinName::new("PENDING-DRAIN"),
                ),
                crate::BuiltinImplementation::direct(descriptor, unused_builtin),
            )
            .unwrap_or_else(|error| panic!("register builtin: {error:?}"));
        context.set_pending(ObjectError::TypeError);
        context.set_pending_lisp_error(LispError::EndOfFile);

        assert_eq!(
            runtime.call_builtin(&mut context, function, &[]),
            Err(ObjectError::ControlError)
        );
        assert!(!context.thread_access().runtime_error_pending());
        assert_eq!(
            context.take_pending_runtime_errors(),
            crate::PendingRuntimeErrors {
                object: None,
                lisp: None,
                native: None,
            }
        );
    }
}
