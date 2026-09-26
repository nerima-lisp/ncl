//! Builtin registration for standard macros and macroexpansion.

use ncl_object::{
    Builtin, BuiltinArgs, BuiltinConvention, BuiltinFunctionCaller, BuiltinIdentifier,
    BuiltinImplementation, BuiltinName, BuiltinPackage, FunctionArguments, FunctionCaller,
    FunctionDesignator, LambdaList, LispError, MultipleValues, ObjectError, ObjectType, Parameter,
    ParameterType, Runtime, ThreadContext, Word, car, cdr,
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
    let mut rooted = Vec::with_capacity(arguments.len() + 1);
    rooted.push(designator);
    rooted.extend_from_slice(arguments);
    ncl_object::with_roots(ctx, &rooted, |ctx, rooted| {
        let designator_word = *rooted[0];
        let designator = match function_designator(ctx, designator_word) {
            Ok(designator) => designator,
            Err(error) => {
                ctx.set_pending_lisp_error(LispError::TypeError {
                    datum: designator_word,
                    expected: ObjectType::Function,
                });
                return Err(error);
            }
        };
        let arguments: Vec<Word> = rooted[1..].iter().map(|word| **word).collect();
        let mut caller = BuiltinFunctionCaller;
        caller
            .call_function(
                ctx,
                runtime,
                designator,
                FunctionArguments::new(&arguments),
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
    })
}

fn funcall(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let designator = args.required(0)?;
    let arguments: Vec<Word> = (1..args.len())
        .filter_map(|index| args.get(index))
        .collect();
    call_designator(ctx, runtime, designator, &arguments, values)
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
    let supplied: Vec<Word> = (0..args.len())
        .filter_map(|index| args.get(index))
        .collect();
    let last = *supplied.last().ok_or(ObjectError::TypeError)?;
    let mut arguments = supplied[1..supplied.len() - 1].to_vec();
    append_list(ctx, last, &mut arguments)?;
    call_designator(ctx, runtime, designator, &arguments, values)
}

/// Register the function-calling builtins owned by this crate.
///
/// # Errors
/// Returns an object-layer error if either builtin cannot be registered.
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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use ncl_object::{Arity, Package, make_cons};

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

    #[allow(clippy::unnecessary_wraps)]
    fn returns_zero(
        _ctx: &mut ThreadContext,
        _runtime: &Runtime,
        _args: &BuiltinArgs<'_>,
        values: &mut MultipleValues,
    ) -> Result<Word, ObjectError> {
        values.clear();
        Ok(Word::NIL)
    }

    #[allow(clippy::unnecessary_wraps)]
    fn returns_one(
        _ctx: &mut ThreadContext,
        _runtime: &Runtime,
        _args: &BuiltinArgs<'_>,
        values: &mut MultipleValues,
    ) -> Result<Word, ObjectError> {
        let value = Word::fixnum(11);
        values.set(&[value]);
        Ok(value)
    }

    #[allow(clippy::unnecessary_wraps)]
    fn returns_two(
        _ctx: &mut ThreadContext,
        _runtime: &Runtime,
        _args: &BuiltinArgs<'_>,
        values: &mut MultipleValues,
    ) -> Result<Word, ObjectError> {
        let values_to_return = [Word::fixnum(21), Word::fixnum(22)];
        values.set(&values_to_return);
        Ok(values_to_return[0])
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

    fn register_result_builtin(
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: &'static str,
        function: ncl_object::RustBuiltin,
    ) -> ncl_object::FunctionObject {
        runtime
            .register_builtin(
                ctx,
                BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new(name)),
                BuiltinImplementation::direct(
                    Builtin {
                        lambda_list: LambdaList::new(&[], &[], None, &[], false),
                        convention: BuiltinConvention::Direct(Arity::exact(0)),
                    },
                    function,
                ),
            )
            .unwrap()
    }

    #[test]
    fn funcall_and_apply_forward_arguments_and_multiple_values() {
        let (runtime, mut ctx, function) = setup();
        let funcall = runtime
            .function(&mut ctx, "COMMON-LISP", "FUNCALL")
            .unwrap();
        let apply = runtime.function(&mut ctx, "COMMON-LISP", "APPLY").unwrap();
        let funcall_function = ncl_object::FunctionObject::try_from(funcall).unwrap();
        let apply_function = ncl_object::FunctionObject::try_from(apply).unwrap();
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

        for (name, callback, result, values) in [
            (
                "RETURNS-ZERO",
                returns_zero as ncl_object::RustBuiltin,
                Word::NIL,
                &[][..],
            ),
            (
                "RETURNS-ONE",
                returns_one as ncl_object::RustBuiltin,
                Word::fixnum(11),
                &[Word::fixnum(11)][..],
            ),
            (
                "RETURNS-TWO",
                returns_two as ncl_object::RustBuiltin,
                Word::fixnum(21),
                &[Word::fixnum(21), Word::fixnum(22)][..],
            ),
        ] {
            let result_function = register_result_builtin(&mut ctx, &runtime, name, callback);
            let result_function_word = result_function.as_word();
            assert_eq!(
                runtime.call_builtin(&mut ctx, funcall_function, &[result_function_word],),
                Ok(result)
            );
            assert_eq!(ctx.values(), values);
            assert_eq!(
                runtime.call_builtin(&mut ctx, apply_function, &[result_function_word, Word::NIL],),
                Ok(result)
            );
            assert_eq!(ctx.values(), values);
        }
    }

    #[test]
    fn funcall_and_apply_forward_five_arguments_with_rooted_values_under_gc_stress() {
        let runtime = Runtime::new().unwrap();
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).unwrap();
        let function = runtime
            .register_builtin(
                &mut ctx,
                BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("ADD-STRESS")),
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
        let mut function_word = function.as_word();
        let function_token = ncl_object::push_root(&mut ctx, &mut function_word);
        let mut list = Word::NIL;
        let list_token = ncl_object::push_root(&mut ctx, &mut list);
        for value in (1..=5).rev() {
            list = make_cons(&mut ctx, &runtime, Word::fixnum(value), list).unwrap();
        }
        let mut funcall_word = runtime
            .function(&mut ctx, "COMMON-LISP", "FUNCALL")
            .unwrap();
        let funcall_token = ncl_object::push_root(&mut ctx, &mut funcall_word);
        let mut apply_word = runtime.function(&mut ctx, "COMMON-LISP", "APPLY").unwrap();
        let apply_token = ncl_object::push_root(&mut ctx, &mut apply_word);
        let funcall = ncl_object::FunctionObject::try_from(funcall_word).unwrap();
        let apply = ncl_object::FunctionObject::try_from(apply_word).unwrap();
        let target = ncl_object::FunctionObject::try_from(function_word).unwrap();
        ctx.set_gc_stress(true);
        ctx.set_strict_forwarding(true);
        assert_eq!(
            runtime.call_builtin(
                &mut ctx,
                funcall,
                &[
                    target.as_word(),
                    Word::fixnum(1),
                    Word::fixnum(2),
                    Word::fixnum(3),
                    Word::fixnum(4),
                    Word::fixnum(5),
                ],
            ),
            Ok(Word::fixnum(15))
        );
        assert_eq!(ctx.values(), &[Word::fixnum(15), Word::fixnum(16)]);
        assert_eq!(
            runtime.call_builtin(&mut ctx, apply, &[target.as_word(), Word::fixnum(0), list],),
            Ok(Word::fixnum(15))
        );
        assert_eq!(ctx.values(), &[Word::fixnum(15), Word::fixnum(16)]);

        assert!(ncl_object::pop_root(&mut ctx, apply_token));
        assert!(ncl_object::pop_root(&mut ctx, funcall_token));
        assert!(ncl_object::pop_root(&mut ctx, list_token));
        assert!(ncl_object::pop_root(&mut ctx, function_token));
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
