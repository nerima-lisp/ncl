//! Runtime-side dispatch for calling function designators.

use ncl_object::typed::FunctionDesignator;
use ncl_object::{
    code_entry, code_size, function_code, function_entry, symbol_function, Function,
    FunctionArguments, FunctionCaller, FunctionObject, MultipleValues, ObjectError,
    Runtime as ObjectRuntime, ThreadContext, Word,
};
use ncl_sys::{invoke_entry_with_function, CodePtr};

/// Calls registered Rust builtins and published native simple-funs.
#[derive(Debug, Default)]
pub struct RuntimeFunctionCaller<'a> {
    code: &'a [CodePtr],
}

impl<'a> RuntimeFunctionCaller<'a> {
    /// Create a caller backed by code allocations owned by an outer runtime.
    #[must_use]
    pub const fn new(code: &'a [CodePtr]) -> Self {
        Self { code }
    }
}

impl FunctionCaller for RuntimeFunctionCaller<'_> {
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

        self.call_native(ctx, function, args.as_slice(), values)
    }
}

impl RuntimeFunctionCaller<'_> {
    fn call_native(
        &self,
        ctx: &mut ThreadContext,
        function: FunctionObject,
        args: &[Word],
        values: &mut MultipleValues,
    ) -> Result<Word, ObjectError> {
        let function = Function::from_word(function.as_word());
        let code = function_code(ctx, function)?;
        let entry = function_entry(ctx, function)?;
        let code_start = code_entry(ctx, code)?
            .as_fixnum()
            .ok_or(ObjectError::Layout)?;
        let code_size = code_size(ctx, code)?
            .as_fixnum()
            .ok_or(ObjectError::Layout)?;
        if entry == 0 || code_start < 0 || code_size < 0 {
            return Err(ObjectError::TypeError);
        }
        let code = self
            .code
            .iter()
            .find(|code| entry >= code.address() && entry - code.address() < code.len())
            .ok_or(ObjectError::TypeError)?;
        let entry_offset = entry - code.address();
        if code_start > i64::try_from(code.len()).map_err(|_| ObjectError::Layout)?
            || code_size > i64::try_from(code.len()).map_err(|_| ObjectError::Layout)?
            || entry_offset != usize::try_from(code_start).map_err(|_| ObjectError::Layout)?
        {
            return Err(ObjectError::Layout);
        }

        let mut registers = [0_u64; 4];
        for (register, argument) in args.iter().take(4).enumerate() {
            registers[register] = argument.bits();
        }
        let rest = args
            .get(4..)
            .filter(|rest| !rest.is_empty())
            .map_or(0, |rest| rest.as_ptr() as usize as u64);
        let (result, count) = invoke_entry_with_function(
            code,
            entry_offset,
            ctx.thread_mut() as *mut ncl_sys::Thread,
            function.as_word().bits(),
            args.len() as u64,
            registers,
            rest,
        );
        values.clear();
        if count > 1 {
            return Err(ObjectError::TypeError);
        }
        Ok(Word::from_bits(result))
    }
}

fn resolve_function(
    ctx: &mut ThreadContext,
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
mod tests {
    use super::RuntimeFunctionCaller;
    use crate::Runtime;
    use ncl_object::typed::FunctionDesignator;
    use ncl_object::{
        make_closure, make_code_object, Arity, Builtin, BuiltinArgs, BuiltinConvention,
        BuiltinIdentifier, BuiltinImplementation, BuiltinName, BuiltinPackage, FunctionArguments,
        FunctionCaller, LambdaList, MultipleValues, Package, Parameter, ParameterType, Word,
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
    fn calls_registered_builtin_through_function_and_symbol_designators() {
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
            .map(|(symbol, _)| ncl_object::typed::Symbol::from_word(symbol))
            .unwrap_or_else(|| panic!("test symbol was not interned"));
        let argument_words = [Word::fixnum(2), Word::fixnum(3)];
        let args = FunctionArguments::new(&argument_words);
        let mut caller = RuntimeFunctionCaller::new(&runtime.code);

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
        let mut caller = RuntimeFunctionCaller::new(&runtime.code);
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
        assert!(values.is_empty());
    }
}
