use super::publish_values::publish_builtin_values;
use crate::{
    BuiltinArgs, BuiltinImplementation, MultipleValues, ObjectError, Runtime, ThreadContext, Word,
};

pub(super) fn call_nested_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    values: &mut MultipleValues,
    implementation: BuiltinImplementation,
) -> Result<Word, ObjectError> {
    let original = args.to_vec();
    let callback_args = if let Some(adapter) = implementation.keyword_adapter {
        adapter(&BuiltinArgs::new(&original))?
    } else {
        original
    };
    let callback_args = BuiltinArgs::new(&callback_args);
    let result = (implementation.function)(ctx, runtime, &callback_args, values);
    let pending_errors = ctx.take_pending_runtime_errors();
    if pending_errors.native.is_some() {
        values.clear();
        ctx.set_values(&[]);
        return Err(ObjectError::Layout);
    }
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
    } else if let Some(error) = pending_errors.lisp {
        ctx.set_pending_lisp_error(error);
    }
    let result = pending_errors.object.map_or(result, Err);
    publish_builtin_values(ctx, &result, values);
    result
}
