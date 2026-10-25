use ncl_object::{ObjectError, Runtime, ThreadContext, Word};

use crate::form::{list, symbol};

use super::deftype_lambda_list::deftype_lambda_list;
use super::macro_definition::macro_function_lambda;
use super::{
    DEFTYPE_PROPERTY, ensure_form_operator, ensure_symbol, form_elements, property_definition,
    required,
};

pub(super) fn deftype(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let parts = form_elements(ctx, required(args, 0)?)?;
    ncl_object::with_roots(ctx, &parts, |ctx, parts| {
        let parts = parts.iter().map(|value| **value).collect::<Vec<_>>();
        ensure_form_operator(ctx, runtime, &parts, "DEFTYPE")?;
        let name = ensure_symbol(ctx, required(&parts, 1)?)?;
        let lambda_list = required(&parts, 2)?;
        let body = parts.get(3..).ok_or(ObjectError::TypeError)?;
        deftype_definition(ctx, runtime, name, lambda_list, body)
    })
}

fn deftype_definition(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: Word,
    lambda_list: Word,
    body: &[Word],
) -> Result<Word, ObjectError> {
    ncl_object::with_roots(ctx, &[name, lambda_list], |ctx, roots| {
        let name = **roots.first().ok_or(ObjectError::TypeError)?;
        let lambda_list =
            deftype_lambda_list(ctx, runtime, **roots.get(1).ok_or(ObjectError::TypeError)?)?;
        ncl_object::with_root(ctx, &mut lambda_list.clone(), |ctx, lambda_list| {
            let mut block_symbol = symbol(ctx, runtime, "BLOCK")?;
            ncl_object::with_root(ctx, &mut block_symbol, |ctx, block_symbol| {
                let mut block_parts = vec![*block_symbol, name];
                block_parts.extend_from_slice(body);
                let mut block = list(ctx, runtime, &block_parts)?;
                ncl_object::with_root(ctx, &mut block, |ctx, block| {
                    let function_body = [*block];
                    let mut lambda = macro_function_lambda(
                        ctx,
                        runtime,
                        name,
                        lambda_list.get(),
                        &function_body,
                    )?;
                    ncl_object::with_root(ctx, &mut lambda, |ctx, lambda| {
                        let mut function_symbol = symbol(ctx, runtime, "FUNCTION")?;
                        ncl_object::with_root(ctx, &mut function_symbol, |ctx, function_symbol| {
                            let mut function = list(ctx, runtime, &[*function_symbol, *lambda])?;
                            ncl_object::with_root(ctx, &mut function, |ctx, function| {
                                property_definition(ctx, runtime, name, DEFTYPE_PROPERTY, *function)
                            })
                        })
                    })
                })
            })
        })
    })
}
