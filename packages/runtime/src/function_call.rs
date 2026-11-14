//! Runtime-side dispatch for calling function designators.

use ncl_object::typed::FunctionDesignator;
use ncl_object::{
    Function, FunctionArguments, FunctionCaller, FunctionObject, MultipleValues, ObjectError,
    Runtime as ObjectRuntime, ThreadContext, Word, function_entry, symbol_function,
    symbol_is_macro,
};

/// Calls registered Rust builtins and published native simple-funs.
#[derive(Debug, Default)]
pub struct RuntimeFunctionCaller;

pub fn invoke_condition_handler(
    runtime: std::ptr::NonNull<()>,
    ctx: &mut ThreadContext,
    handler: Word,
    arguments: &[Word],
) -> Result<Word, ObjectError> {
    ncl_sys::with_opaque_mut(runtime, |runtime: &mut crate::Runtime| {
        let mut caller = RuntimeFunctionCaller;
        let mut values = MultipleValues::default();
        caller.call_function(
            ctx,
            &runtime.object,
            FunctionDesignator::Function(FunctionObject::try_from(handler)?),
            FunctionArguments::new(arguments),
            &mut values,
        )
    })
}

impl FunctionCaller for RuntimeFunctionCaller {
    fn call_function(
        &mut self,
        ctx: &mut ThreadContext,
        runtime: &ObjectRuntime,
        designator: FunctionDesignator,
        args: FunctionArguments<'_>,
        values: &mut MultipleValues,
    ) -> Result<Word, ObjectError> {
        let function = resolve_function(ctx, designator)?;
        if let Some(result) =
            runtime.call_builtin_if_registered(ctx, function, args.as_slice(), values)
        {
            let result = result?;
            publish_context_values(ctx, result, values);
            return Ok(result);
        }

        let result = ncl_object::with_heap_rooted_mut_slice(runtime, args.as_slice(), |args| {
            call_native(ctx, runtime, function, args, values)
        })?;
        publish_context_values(ctx, result, values);
        Ok(result)
    }
}

fn publish_context_values(ctx: &mut ThreadContext, primary: Word, values: &MultipleValues) {
    if values.is_explicit() {
        ctx.set_values(values.as_slice());
    } else {
        ctx.set_values(std::slice::from_ref(&primary));
    }
}

fn call_native(
    ctx: &mut ThreadContext,
    runtime: &ObjectRuntime,
    function: FunctionObject,
    args: &mut [Word],
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let function = Function::from_word(function.as_word());
    let entry = function_entry(ctx, function)?;
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
    if ctx.take_non_local_exit() {
        return propagate_non_local_exit(ctx, values);
    }
    if let Some(error) = pending_errors.native {
        let Some(signaled) = crate::native_error::signal_pending_native_error(ctx, runtime, error)
        else {
            values.clear();
            return Err(ObjectError::Layout);
        };
        if let Err(error) = signaled {
            values.clear();
            return Err(error);
        }
        if ctx.take_non_local_exit() {
            return propagate_non_local_exit(ctx, values);
        }
        values.clear();
        return Ok(Word::NIL);
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
        values.clear();
        return Err(error);
    }
    Ok(result)
}

fn propagate_non_local_exit(
    ctx: &mut ThreadContext,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
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
    Err(ObjectError::NonLocalExit)
}

fn resolve_function(
    ctx: &ThreadContext,
    designator: FunctionDesignator,
) -> Result<FunctionObject, ObjectError> {
    match designator {
        FunctionDesignator::Function(function) => Ok(function),
        FunctionDesignator::Symbol(symbol) => {
            let symbol = symbol.into();
            if symbol_is_macro(ctx, symbol)? {
                return Err(ObjectError::UndefinedFunction);
            }
            let word = symbol_function(ctx, symbol)?;
            FunctionObject::try_from(word).map_err(|_| ObjectError::UndefinedFunction)
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "The included unit tests unwrap successful function setup and calls to assert results."
)]
#[path = "tests_function_call.rs"]
mod tests;

#[cfg(test)]
mod pending_native_error_tests {
    use ncl_object::{
        Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
        BuiltinName, BuiltinPackage, Function, FunctionObject, LambdaList, MultipleValues,
        ObjectError, Parameter, ParameterType, ThreadContext, Word, function_code,
    };

    use crate::Runtime;
    use crate::support::NativeInvocation;

    const PENDING_OBJECT_ERROR_ID: BuiltinIdentifier = BuiltinIdentifier::new(
        BuiltinPackage::NclTest,
        BuiltinName::new("RUNTIME-PENDING-OBJECT-ERROR"),
    );
    const PENDING_ERROR_PARAMETERS: &[Parameter] = &[Parameter {
        name: BuiltinName::new("VALUE"),
        ty: ParameterType::Any,
    }];

    fn pending_object_error(
        context: &mut ThreadContext,
        _runtime: &ncl_object::Runtime,
        _args: &BuiltinArgs<'_>,
        _values: &mut MultipleValues,
    ) -> Result<Word, ObjectError> {
        context.set_pending(ObjectError::Layout);
        Err(ObjectError::Layout)
    }

    fn register_pending_builtin(
        runtime: &mut Runtime,
        identifier: BuiltinIdentifier,
        implementation: ncl_object::RustBuiltin,
    ) -> Result<FunctionObject, ObjectError> {
        let descriptor = Builtin {
            lambda_list: LambdaList::new(PENDING_ERROR_PARAMETERS, &[], None, &[], false),
            convention: BuiltinConvention::Direct(Arity::exact(1)),
        };
        runtime.object.register_builtin(
            &mut runtime.context,
            identifier,
            BuiltinImplementation::direct(descriptor, implementation),
        )
    }

    fn call_builtin_through_native_trampoline(
        runtime: &mut Runtime,
        function: FunctionObject,
        code: ncl_object::CodeObject,
        args: &mut [Word],
        values: &mut MultipleValues,
    ) -> Result<Word, ObjectError> {
        let object = &runtime.object;
        let entry_codes = &runtime.entry_codes;
        let context = &mut runtime.context;
        let thread = context.thread_access().as_non_null();
        let mut invocation = NativeInvocation {
            object,
            context,
            code,
            entry_codes,
        };
        let previous = ncl_sys::replace_native_context(
            thread,
            Some(std::ptr::NonNull::from(&mut invocation).cast()),
        );
        let result = super::call_native(
            invocation.context,
            invocation.object,
            function,
            args,
            values,
        );
        let _ = ncl_sys::replace_native_context(thread, previous);
        result
    }

    #[test]
    fn native_call_returns_object_error_without_signaling_type_error()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut runtime = Runtime::new()?;
        let function =
            register_pending_builtin(&mut runtime, PENDING_OBJECT_ERROR_ID, pending_object_error)?;
        let code = function_code(&runtime.context, Function::from_word(function.as_word()))?;
        let mut values = MultipleValues::new();
        let result = call_builtin_through_native_trampoline(
            &mut runtime,
            function,
            code,
            &mut [Word::fixnum(1)],
            &mut values,
        );

        assert_eq!(result, Err(ObjectError::Layout));
        assert!(values.is_empty());
        assert_eq!(runtime.context.take_pending_condition(), None);
        assert!(!runtime.context.take_non_local_exit());
        Ok(())
    }
}
