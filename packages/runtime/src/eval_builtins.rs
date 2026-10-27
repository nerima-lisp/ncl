use crate::{Runtime, RuntimeError};
use ncl_object::{
    Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, HeapRootedWord, LambdaList, MultipleValues, ObjectError, Parameter,
    ParameterType, Runtime as ObjectRuntime, ThreadContext, Word,
};

const FORM: Parameter = Parameter {
    name: BuiltinName::new("FORM"),
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
        Err(
            RuntimeError::Object(error)
            | RuntimeError::Front(ncl_compiler_front::FrontError::Object(error)),
        ) => Err(error),
        Err(RuntimeError::UndefinedFunction { .. }) => {
            let condition = undefined_function_condition.ok_or(ObjectError::Layout)?;
            ctx.set_pending_condition(condition);
            Err(ObjectError::TypeError)
        }
        Err(RuntimeError::NativeFailure { condition, .. }) => {
            crate::native_error::signal_native_failure(ctx, object, condition)
        }
        Err(_) => Err(ObjectError::Layout),
    }
}
