//! Runtime-side dispatch for calling function designators.

use ncl_object::typed::FunctionDesignator;
use ncl_object::{
    Function, FunctionArguments, FunctionCaller, FunctionObject, MultipleValues, ObjectError,
    Runtime as ObjectRuntime, ThreadContext, Word, function_entry, symbol_function,
};
use ncl_sys::invoke_entry_with_function_address;

/// Calls registered Rust builtins and published native simple-funs.
#[derive(Debug, Default)]
pub struct RuntimeFunctionCaller;

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
        if runtime.builtin_descriptor(function).is_some() {
            let result = runtime.call_builtin(ctx, function, args.as_slice())?;
            values.set(ctx.values());
            return Ok(result);
        }

        ncl_object::with_rooted_slice(ctx, args.as_slice(), |ctx, rooted_args| {
            call_native(ctx, function, rooted_args, values)
        })
    }
}

fn call_native(
    ctx: &mut ThreadContext,
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
    let argument_count = u64::try_from(args.len()).map_err(|_| ObjectError::Layout)?;
    let rest = args
        .get_mut(4..)
        .filter(|rest| !rest.is_empty())
        .map_or(Ok(0), |rest| {
            u64::try_from(rest.as_mut_ptr().addr()).map_err(|_| ObjectError::Layout)
        })?;
    let (result, count) = invoke_entry_with_function_address(
        entry,
        std::ptr::from_mut(ctx.thread_mut()),
        function.as_word().bits(),
        argument_count,
        registers,
        rest,
    );
    let count = usize::try_from(count).map_err(|_| ObjectError::Layout)?;
    values.clear();
    match count {
        0 => Ok(Word::NIL),
        1 => {
            let value = Word::from_bits(result);
            values.set(&[value]);
            Ok(value)
        }
        count => {
            let area = ctx.thread_mut().multiple_values();
            if area.len() < count {
                return Err(ObjectError::Layout);
            }
            values.set(&area[..count]);
            Ok(Word::from_bits(result))
        }
    }
}

fn resolve_function(
    ctx: &ThreadContext,
    designator: FunctionDesignator,
) -> Result<FunctionObject, ObjectError> {
    match designator {
        FunctionDesignator::Function(function) => Ok(function),
        FunctionDesignator::Symbol(symbol) => {
            let word = symbol_function(ctx, symbol.into())?;
            FunctionObject::try_from(word).map_err(|_| ObjectError::UndefinedFunction)
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::RuntimeFunctionCaller;
    use crate::Runtime;
    use ncl_object::typed::FunctionDesignator;
    use ncl_object::{
        Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
        BuiltinName, BuiltinPackage, FunctionArguments, FunctionCaller, LambdaList, MultipleValues,
        Package, Parameter, ParameterType, Word, make_closure, make_code_object,
    };

    const TEST_ID: BuiltinIdentifier =
        BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("RUNTIME-CALL"));
    const PARAMETERS: &[Parameter] = &[
        Parameter {
            name: BuiltinName::new("LEFT"),
            ty: ParameterType::Fixnum,
        },
        Parameter {
            name: BuiltinName::new("RIGHT"),
            ty: ParameterType::Fixnum,
        },
    ];

    fn add(
        _ctx: &mut ncl_object::ThreadContext,
        _runtime: &ncl_object::Runtime,
        args: &BuiltinArgs<'_>,
        _values: &mut MultipleValues,
    ) -> Result<Word, ncl_object::ObjectError> {
        let left = args
            .required(0)?
            .as_fixnum()
            .ok_or(ncl_object::ObjectError::TypeError)?;
        let right = args
            .required(1)?
            .as_fixnum()
            .ok_or(ncl_object::ObjectError::TypeError)?;
        Ok(Word::fixnum(left + right))
    }

    fn register_builtin(runtime: &mut Runtime) -> ncl_object::FunctionObject {
        let descriptor = Builtin {
            lambda_list: LambdaList::new(PARAMETERS, &[], None, &[], false),
            convention: BuiltinConvention::Direct(Arity::exact(2)),
        };
        runtime
            .object
            .register_builtin(
                &mut runtime.context,
                TEST_ID,
                BuiltinImplementation::direct(descriptor, add),
            )
            .unwrap_or_else(|error| panic!("register builtin: {error:?}"))
    }

    #[test]
    fn calls_registered_builtin_through_function_and_symbol_designators_under_gc_stress_and_forwarding() {
        let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
        let function = register_builtin(&mut runtime);
        let package = runtime
            .object
            .find_package(&runtime.context, "NCL-TEST")
            .and_then(|word| {
                Package::from_word(word)
                    .intern(&mut runtime.context, &runtime.object, "RUNTIME-CALL")
                    .ok()
            })
            .map_or_else(
                || panic!("test symbol was not interned"),
                |(symbol, _)| ncl_object::typed::Symbol::from_word(symbol),
            );
        runtime.context.set_gc_stress(true);
        runtime.context.set_strict_forwarding(true);
        let argument_words = [Word::fixnum(2), Word::fixnum(3)];
        let args = FunctionArguments::new(&argument_words);
        let mut caller = RuntimeFunctionCaller;

        let mut values = MultipleValues::new();
        assert_eq!(
            caller.call_function(
                &mut runtime.context,
                &runtime.object,
                FunctionDesignator::Function(function),
                args,
                &mut values,
            ),
            Ok(Word::fixnum(5))
        );
        assert!(values.is_empty());

        let mut values = MultipleValues::new();
        assert_eq!(
            caller.call_function(
                &mut runtime.context,
                &runtime.object,
                FunctionDesignator::Symbol(package),
                args,
                &mut values,
            ),
            Ok(Word::fixnum(5))
        );
    }

    #[test]
    #[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
    fn calls_a_published_simple_fun_through_the_native_abi() {
        let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
        #[cfg(target_arch = "x86_64")]
        let machine_code = [
            0xb8, 0x54, 0, 0, 0, // mov eax, (42 << 1)
            0xba, 1, 0, 0, 0,    // mov edx, 1
            0xc3, // ret
        ];
        #[cfg(target_arch = "aarch64")]
        let machine_code = [
            0x80, 0x0a, 0x80, 0xd2, // mov x0, #84
            0x21, 0x00, 0x80, 0xd2, // mov x1, #1
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ];
        let mut native = ncl_sys::alloc_code(machine_code.len()).unwrap();
        ncl_sys::write_code(&mut native, 0, &machine_code).unwrap();
        ncl_sys::publish_code(&mut native).unwrap();
        let code = make_code_object(
            &mut runtime.context,
            &runtime.object,
            0,
            machine_code.len(),
            Word::NIL,
            Word::NIL,
            Word::NIL,
        )
        .unwrap_or_else(|error| panic!("code object: {error:?}"));
        let closure = make_closure(
            &mut runtime.context,
            &runtime.object,
            native.address(),
            Word::NIL,
            Word::NIL,
            code,
            &[],
        )
        .unwrap_or_else(|error| panic!("closure: {error:?}"));
        let function = ncl_object::FunctionObject::try_from(closure.as_word())
            .unwrap_or_else(|error| panic!("function object: {error:?}"));
        let mut caller = RuntimeFunctionCaller;
        let mut values = MultipleValues::new();

        assert_eq!(
            caller.call_function(
                &mut runtime.context,
                &runtime.object,
                FunctionDesignator::Function(function),
                FunctionArguments::new(&[]),
                &mut values,
            ),
            Ok(Word::fixnum(42))
        );
        assert_eq!(values.as_slice(), &[Word::fixnum(42)]);
    }
}
