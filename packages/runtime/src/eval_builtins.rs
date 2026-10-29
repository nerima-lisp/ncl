use crate::{Runtime, RuntimeError};
use ncl_object::{
    Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, FunctionObject, HeapRootedWord, LambdaList, Local, MultipleValues, ObjectError,
    Parameter, ParameterType, Runtime as ObjectRuntime, Scope, ThreadContext, Word,
};

const FORM: Parameter = Parameter {
    name: BuiltinName::new("FORM"),
    ty: ParameterType::Any,
};

const NAME: Parameter = Parameter {
    name: BuiltinName::new("NAME"),
    ty: ParameterType::Any,
};

const DEFINITION: Parameter = Parameter {
    name: BuiltinName::new("DEFINITION"),
    ty: ParameterType::Any,
};

pub fn register(ctx: &mut ThreadContext, object: &ObjectRuntime) -> Result<(), ObjectError> {
    object
        .register_builtin(
            ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("EVAL")),
            BuiltinImplementation::adapted(
                Builtin {
                    lambda_list: LambdaList::fixed(&[FORM]),
                    convention: BuiltinConvention::Adapted,
                },
                eval_builtin,
                |args| Ok(args.as_slice().to_vec()),
            )
            .with_nested_evaluation(),
        )
        .map(|_| ())?;
    object
        .register_builtin(
            ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("COMPILE")),
            BuiltinImplementation::adapted(
                Builtin {
                    lambda_list: LambdaList::with_optional(&[NAME], &[DEFINITION]),
                    convention: BuiltinConvention::Adapted,
                },
                compile_builtin,
                |args| Ok(args.as_slice().to_vec()),
            )
            .with_nested_evaluation(),
        )
        .map(|_| ())
}

fn eval_builtin(
    ctx: &mut ThreadContext,
    object: &ObjectRuntime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let Some(pointer) = ctx.evaluator_runtime() else {
        return Err(ObjectError::Layout);
    };
    let form = HeapRootedWord::new(ctx, args.required(0)?)?;
    let (result, undefined_function_condition) =
        ncl_sys::with_opaque_mut(pointer, |runtime: &mut Runtime| {
            let result = runtime.eval_form_with_values(form.get());
            let condition = if matches!(&result, Err(RuntimeError::UndefinedFunction { .. })) {
                runtime.pending_undefined_function_condition()
            } else {
                None
            };
            (result, condition)
        });
    match result {
        Ok((value, evaluated_values)) => {
            values.set(&evaluated_values);
            Ok(value)
        }
        Err(error) => Err(crate::native_error::runtime_error_to_object_error(
            ctx,
            object,
            error,
            undefined_function_condition,
        )),
    }
}

fn compile_builtin(
    ctx: &mut ThreadContext,
    object: &ObjectRuntime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let name = HeapRootedWord::new(ctx, args.required(0)?)?;
    if name.get() != Word::NIL {
        ncl_object::symbol_is_macro(ctx, name.get())?;
    }

    let definition = args
        .get(1)
        .map(|word| HeapRootedWord::new(ctx, word))
        .transpose()?;
    let result = if let Some(definition) = definition.as_ref() {
        let definition_word = definition.get();
        let function = match FunctionObject::try_from(definition_word) {
            Ok(function) => {
                ncl_object::function_entry(
                    ctx,
                    ncl_object::Function::from_word(function.as_word()),
                )?;
                definition_word
            }
            Err(_) if definition_word.is_cons() => {
                compile_lambda_definition(ctx, object, definition_word)?
            }
            Err(_) => return Err(ObjectError::TypeError),
        };
        let function = HeapRootedWord::new(ctx, function)?;
        if name.get() == Word::NIL {
            function.get()
        } else {
            ctx.write_object_slot(
                name.get(),
                ncl_object::symbol_offset::FUNCTION,
                function.get(),
            )?;
            name.get()
        }
    } else if name.get() == Word::NIL {
        Word::NIL
    } else {
        // With no definition, an undefined name has undefined consequences.
        let _ = ncl_object::symbol_function(ctx, name.get())?;
        name.get()
    };

    values.set(&[result, Word::NIL, Word::NIL]);
    Ok(result)
}

fn compile_lambda_definition(
    ctx: &mut ThreadContext,
    object: &ObjectRuntime,
    definition: Word,
) -> Result<Word, ObjectError> {
    let function_form = {
        let mut scope = Scope::new(ctx);
        let definition = scope.root(Local::from_word(definition));
        let operator = scope.intern(object, "COMMON-LISP", "FUNCTION")?;
        let nil = scope.root(Local::from_word(Word::NIL));
        let arguments = scope.make_cons(object, definition, nil)?;
        let form = scope.make_cons(object, operator, arguments)?;
        form.get(&scope).as_word()
    };
    let form = HeapRootedWord::new(ctx, function_form)?;
    let Some(pointer) = ctx.evaluator_runtime() else {
        return Err(ObjectError::Layout);
    };
    let (result, undefined_function_condition) =
        ncl_sys::with_opaque_mut(pointer, |runtime: &mut Runtime| {
            let result = runtime.eval_form_with_values(form.get());
            let condition = if matches!(&result, Err(RuntimeError::UndefinedFunction { .. })) {
                runtime.pending_undefined_function_condition()
            } else {
                None
            };
            (result, condition)
        });
    match result {
        Ok((function, _)) => Ok(function),
        Err(error) => Err(crate::native_error::runtime_error_to_object_error(
            ctx,
            object,
            error,
            undefined_function_condition,
        )),
    }
}
