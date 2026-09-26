use super::expansion::expand_loop_ast;
use super::{MultipleValues, Result, Runtime, ThreadContext, elements, parse_loop};
use ncl_object::{BuiltinArgs, ObjectError};

pub fn expand_loop_callback(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result {
    let form = args.get(0).ok_or(ObjectError::TypeError)?;
    let input = elements(ctx, form)?;
    let input = input.get(1..).ok_or(ObjectError::TypeError)?;
    let ast = parse_loop(ctx, input)?;
    expand_loop_ast(ctx, runtime, &ast)
}
