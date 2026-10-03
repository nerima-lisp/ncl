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
const REST_ARGUMENT: ncl_object::Parameter = ncl_object::Parameter {
    name: BuiltinName::new("ARGUMENTS"),
    ty: ncl_object::ParameterType::Any,
};

fn capture_multiple_values(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    _args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let values = ctx.values().to_vec();
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
                lambda_list: LambdaList::fixed(&[]),
                convention: BuiltinConvention::Direct(Arity::exact(0)),
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
        ),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ncl_object::FunctionObject;

    #[test]
    fn registration_exposes_multiple_value_call_list() -> Result<(), ObjectError> {
        let runtime = Runtime::new()?;
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)?;
        register(&mut ctx, &runtime)?;
        let function = runtime
            .function(&mut ctx, "NCL-EXT", "MULTIPLE-VALUE-CALL-LIST")
            .ok_or(ObjectError::TypeError)?;
        assert!(FunctionObject::try_from(function).is_ok());
        let capture = runtime
            .function(&mut ctx, "NCL-EXT", "CAPTURE-MULTIPLE-VALUES")
            .ok_or(ObjectError::TypeError)?;
        assert!(FunctionObject::try_from(capture).is_ok());
        Ok(())
    }
}
