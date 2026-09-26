//! Builtin registration for standard macros and macroexpansion.

use ncl_object::{
    car, cdr, Builtin, BuiltinArgs, BuiltinConvention, BuiltinFunctionCaller, BuiltinIdentifier,
    BuiltinImplementation, BuiltinName, BuiltinPackage, FunctionArguments, FunctionCaller,
    FunctionDesignator, LambdaList, LispError, MultipleValues, ObjectError, ObjectType, Parameter,
    ParameterType, Runtime, ThreadContext, Word,
};

const FUNCTION: Parameter = Parameter {
    name: BuiltinName::new("FUNCTION"),
    ty: ParameterType::FunctionDesignator,
};
const ARGUMENT: Parameter = Parameter {
    name: BuiltinName::new("ARGUMENT"),
    ty: ParameterType::Any,
};
const REST_ARGUMENT: Parameter = Parameter {
    name: BuiltinName::new("ARGUMENTS"),
    ty: ParameterType::Any,
};

fn function_designator(ctx: &ThreadContext, word: Word) -> Result<FunctionDesignator, ObjectError> {
    FunctionDesignator::try_from_word(ctx, word).map_err(|_| ObjectError::TypeError)
}

fn call_designator(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    designator: Word,
    arguments: &[Word],
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let designator = match function_designator(ctx, designator) {
        Ok(designator) => designator,
        Err(error) => {
            ctx.set_pending_lisp_error(LispError::TypeError {
                datum: designator,
                expected: ObjectType::Function,
            });
            return Err(error);
        }
    };
    let mut caller = BuiltinFunctionCaller;
    caller
        .call_function(
            ctx,
            runtime,
            designator,
            FunctionArguments::new(arguments),
            values,
        )
        .map_err(|error| {
            if matches!(error, ObjectError::Unbound | ObjectError::UndefinedFunction) {
                ctx.set_pending_lisp_error(LispError::CellError(
                    ncl_object::CellError::UndefinedFunction,
                ));
                ObjectError::UndefinedFunction
            } else {
                error
            }
        })
}

fn funcall(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let designator = args.required(0)?;
    let arguments = &args.as_slice()[1..];
    call_designator(ctx, runtime, designator, arguments, values)
}

fn append_list(
    ctx: &mut ThreadContext,
    list: Word,
    output: &mut Vec<Word>,
) -> Result<(), ObjectError> {
    let mut cursor = list;
    while cursor != Word::NIL {
        if !cursor.is_cons() {
            return Err(ObjectError::TypeError);
        }
        output.push(car(ctx, cursor)?);
        cursor = cdr(ctx, cursor)?;
    }
    Ok(())
}

fn apply(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let designator = args.required(0)?;
    let supplied = args.as_slice();
    let last = *supplied.last().ok_or(ObjectError::TypeError)?;
    let mut arguments = supplied[1..supplied.len() - 1].to_vec();
    append_list(ctx, last, &mut arguments)?;
    call_designator(ctx, runtime, designator, &arguments, values)
}

/// Register the function-calling builtins owned by this crate.
pub fn register(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("FUNCALL")),
        BuiltinImplementation::adapted(
            Builtin {
                lambda_list: LambdaList::with_rest(&[FUNCTION], REST_ARGUMENT),
                convention: BuiltinConvention::Adapted,
            },
            funcall,
            |args| Ok(args.as_slice().to_vec()),
        ),
    )?;
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("APPLY")),
        BuiltinImplementation::adapted(
            Builtin {
                lambda_list: LambdaList::with_rest(&[FUNCTION, ARGUMENT], REST_ARGUMENT),
                convention: BuiltinConvention::Adapted,
            },
            apply,
            |args| Ok(args.as_slice().to_vec()),
        ),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ncl_object::{make_cons, Arity, Package};

    fn add(
        _ctx: &mut ThreadContext,
        _runtime: &Runtime,
        args: &BuiltinArgs<'_>,
        values: &mut MultipleValues,
    ) -> Result<Word, ObjectError> {
        let sum = (0..args.len())
            .map(|index| {
                args.get(index)
                    .and_then(Word::as_fixnum)
                    .ok_or(ObjectError::TypeError)
            })
            .try_fold(0_i64, |sum, value| value.map(|value| sum + value))?;
        values.set(&[Word::fixnum(sum), Word::fixnum(sum + 1)]);
        Ok(Word::fixnum(sum))
    }

    fn setup() -> (Runtime, ThreadContext, ncl_object::FunctionObject) {
        let runtime = Runtime::new().unwrap();
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).unwrap();
        let function = runtime
            .register_builtin(
                &mut ctx,
                BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("ADD")),
                BuiltinImplementation::direct(
                    Builtin {
                        lambda_list: LambdaList::with_rest(&[], ARGUMENT),
                        convention: BuiltinConvention::Direct(Arity::exact(0)),
                    },
                    add,
                ),
            )
            .unwrap();
        register(&mut ctx, &runtime).unwrap();
        (runtime, ctx, function)
    }

    #[test]
    fn funcall_and_apply_forward_arguments_and_multiple_values() {
        let (runtime, mut ctx, function) = setup();
        let funcall = runtime
            .function(&mut ctx, "COMMON-LISP", "FUNCALL")
            .unwrap();
        let apply = runtime.function(&mut ctx, "COMMON-LISP", "APPLY").unwrap();
        let tail = make_cons(&mut ctx, &runtime, Word::fixnum(3), Word::NIL).unwrap();
        let list = make_cons(&mut ctx, &runtime, Word::fixnum(2), tail).unwrap();
        assert_eq!(
            runtime.call_builtin(
                &mut ctx,
                ncl_object::FunctionObject::try_from(funcall).unwrap(),
                &[function.as_word(), Word::fixnum(1), Word::fixnum(2)]
            ),
            Ok(Word::fixnum(3))
        );
        assert_eq!(ctx.values(), &[Word::fixnum(3), Word::fixnum(4)]);
        assert_eq!(
            runtime.call_builtin(
                &mut ctx,
                ncl_object::FunctionObject::try_from(apply).unwrap(),
                &[function.as_word(), Word::fixnum(1), list]
            ),
            Ok(Word::fixnum(6))
        );
        assert_eq!(ctx.values(), &[Word::fixnum(6), Word::fixnum(7)]);
    }

    #[test]
    fn invalid_and_unbound_designators_report_their_error_kind() {
        let (runtime, mut ctx, _function) = setup();
        let funcall = ncl_object::FunctionObject::try_from(
            runtime
                .function(&mut ctx, "COMMON-LISP", "FUNCALL")
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            runtime.call_builtin(&mut ctx, funcall, &[Word::fixnum(0)]),
            Err(ObjectError::TypeError)
        );
        let package = runtime.ensure_package(&mut ctx, "NCL-TEST").unwrap();
        let (symbol, _) = Package::from_word(package)
            .intern(&mut ctx, &runtime, "ADD")
            .unwrap();
        assert_eq!(
            runtime.call_builtin(&mut ctx, funcall, &[symbol, Word::fixnum(1)]),
            Ok(Word::fixnum(1))
        );
        assert_eq!(ctx.values(), &[Word::fixnum(1), Word::fixnum(2)]);
        let (symbol, _) = Package::from_word(package)
            .intern(&mut ctx, &runtime, "MISSING")
            .unwrap();
        assert_eq!(
            runtime.call_builtin(&mut ctx, funcall, &[symbol, Word::fixnum(1)]),
            Err(ObjectError::UndefinedFunction)
        );
    }

    #[test]
    fn apply_checks_arity_and_proper_list() {
        let (runtime, mut ctx, function) = setup();
        let apply = ncl_object::FunctionObject::try_from(
            runtime.function(&mut ctx, "COMMON-LISP", "APPLY").unwrap(),
        )
        .unwrap();
        assert_eq!(
            runtime.call_builtin(&mut ctx, apply, &[function.as_word()]),
            Err(ObjectError::TypeError)
        );
        assert_eq!(
            runtime.call_builtin(
                &mut ctx,
                apply,
                &[function.as_word(), Word::fixnum(1), Word::fixnum(2)]
            ),
            Err(ObjectError::TypeError)
        );
    }
}
