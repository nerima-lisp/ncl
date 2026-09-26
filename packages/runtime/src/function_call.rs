//! Runtime-side dispatch for calling function designators.

use ncl_object::typed::FunctionDesignator;
use ncl_object::{
    symbol_function, FunctionArguments, FunctionCaller, FunctionObject, MultipleValues,
    ObjectError, Runtime as ObjectRuntime, ThreadContext, Word,
};

/// Calls function designators that resolve to registered Rust builtins.
///
/// Compiled functions and closures are not yet published with enough runtime
/// metadata for this boundary to invoke them safely. They are rejected with a
/// type error instead of being treated as an unimplemented fallback.
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
        if runtime.builtin_descriptor(function).is_none() {
            return Err(ObjectError::TypeError);
        }

        let result = runtime.call_builtin(ctx, function, args.as_slice())?;
        values.set(ctx.values());
        Ok(result)
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
    fn rejects_unregistered_closures_with_a_type_error() {
        let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
        let code = make_code_object(
            &mut runtime.context,
            &runtime.object,
            0,
            0,
            Word::NIL,
            Word::NIL,
            Word::NIL,
        )
        .unwrap_or_else(|error| panic!("code object: {error:?}"));
        let closure = make_closure(
            &mut runtime.context,
            &runtime.object,
            0,
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
            Err(ncl_object::ObjectError::TypeError)
        );
    }
}
