use super::{append_list, call_designator};
use crate::list;
use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, LambdaList, MultipleValues, ObjectError, Runtime, ThreadContext,
    Word,
};

const FUNCTION: ncl_object::Parameter = ncl_object::Parameter {
    name: BuiltinName::new("FUNCTION"),
    ty: ncl_object::ParameterType::FunctionDesignator,
};
const PRIMARY_VALUE: ncl_object::Parameter = ncl_object::Parameter {
    name: BuiltinName::new("PRIMARY-VALUE"),
    ty: ncl_object::ParameterType::Any,
};
const REST_ARGUMENT: ncl_object::Parameter = ncl_object::Parameter {
    name: BuiltinName::new("ARGUMENTS"),
    ty: ncl_object::ParameterType::Any,
};

fn capture_multiple_values(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let values = {
        let thread = ctx.thread_access();
        let count = thread.multiple_value_count();
        thread
            .multiple_values()
            .get(..count)
            .ok_or(ObjectError::Layout)?
            .to_vec()
    };
    let mut values = values;
    if let Some(first) = values.first_mut() {
        *first = args.required(0)?;
    }
    ncl_object::with_roots(ctx, &values, |ctx, roots| {
        let values = roots.iter().map(|value| **value).collect::<Vec<_>>();
        list(ctx, runtime, &values)
    })
}

fn multiple_value_call_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let designator = args.required(0)?;
    let mut arguments = Vec::new();
    for index in 1..args.len() {
        append_list(
            ctx,
            args.get(index).ok_or(ObjectError::TypeError)?,
            &mut arguments,
        )?;
    }
    call_designator(ctx, runtime, designator, &arguments, values)
}

pub(super) fn register(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(
            BuiltinPackage::NclExt,
            BuiltinName::new("CAPTURE-MULTIPLE-VALUES"),
        ),
        BuiltinImplementation::direct(
            Builtin {
                lambda_list: LambdaList::fixed(&[PRIMARY_VALUE]),
                convention: BuiltinConvention::Direct(Arity::exact(1)),
            },
            capture_multiple_values,
        ),
    )?;
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(
            BuiltinPackage::NclExt,
            BuiltinName::new("MULTIPLE-VALUE-CALL-LIST"),
        ),
        BuiltinImplementation::adapted(
            Builtin {
                lambda_list: LambdaList::with_rest(&[FUNCTION], REST_ARGUMENT),
                convention: BuiltinConvention::Adapted,
            },
            multiple_value_call_list,
            |args| Ok(args.as_slice().to_vec()),
        )
        .with_nested_evaluation(),
    )?;
    Ok(())
}
