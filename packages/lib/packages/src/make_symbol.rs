use super::text;
use ncl_object::{
    BuiltinArgs, BuiltinName, MultipleValues, ObjectError, Parameter, ParameterType, Runtime,
    ThreadContext, Word, make_string, make_symbol,
};

pub(super) const PARAMETER: Parameter = Parameter {
    name: BuiltinName::new("NAME"),
    ty: ParameterType::StringDesignator,
};

pub(super) fn builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let name = make_string(
        ctx,
        runtime,
        &text(ctx, args.required(0)?)?.chars().collect::<Vec<_>>(),
    )?;
    make_symbol(ctx, runtime, name)
}
