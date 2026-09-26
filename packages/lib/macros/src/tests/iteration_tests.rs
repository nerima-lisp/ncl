use super::*;

fn expand(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    input: Word,
) -> Result<Word, ObjectError> {
    let callback = callback_for(name).ok_or(ObjectError::UndefinedFunction)?;
    let input_args = [input];
    let args = ncl_object::BuiltinArgs::new(&input_args);
    callback(ctx, runtime, &args, &mut ncl_object::MultipleValues::new())
}

#[test]
fn destructuring_bind_delegates_lambda_list_and_body() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let operator = symbol(&mut ctx, &runtime, "DESTRUCTURING-BIND")?;
    let x = symbol(&mut ctx, &runtime, "X")?;
    let lambda_list = list(&mut ctx, &runtime, &[x])?;
    let input = list(&mut ctx, &runtime, &[operator, lambda_list, Word::NIL, x])?;
    let expansion = expand(&mut ctx, &runtime, "DESTRUCTURING-BIND", input)?;
    let parts = elements(&mut ctx, expansion)?;
    assert_eq!(parts[0], symbol(&mut ctx, &runtime, "FUNCALL")?);
    let lambda = elements(&mut ctx, parts[1])?;
    assert_eq!(lambda[0], symbol(&mut ctx, &runtime, "LAMBDA")?);
    assert_eq!(lambda[1], lambda_list);
    Ok(())
}

#[test]
fn iteration_macros_validate_specs_and_build_blocks() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let x = symbol(&mut ctx, &runtime, "X")?;
    let dolist = symbol(&mut ctx, &runtime, "DOLIST")?;
    let bad = list(&mut ctx, &runtime, &[dolist, x])?;
    assert_eq!(
        expand(&mut ctx, &runtime, "DOLIST", bad),
        Err(ObjectError::TypeError)
    );
    let spec = list(&mut ctx, &runtime, &[x, Word::NIL])?;
    let input = list(&mut ctx, &runtime, &[dolist, spec, x])?;
    let expansion = expand(&mut ctx, &runtime, "DOLIST", input)?;
    let parts = elements(&mut ctx, expansion)?;
    assert_eq!(parts[0], symbol(&mut ctx, &runtime, "BLOCK")?);
    let dotimes = symbol(&mut ctx, &runtime, "DOTIMES")?;
    let spec = list(&mut ctx, &runtime, &[x, Word::fixnum(2)])?;
    let input = list(&mut ctx, &runtime, &[dotimes, spec, x])?;
    let expansion = expand(&mut ctx, &runtime, "DOTIMES", input)?;
    let parts = elements(&mut ctx, expansion)?;
    assert_eq!(parts[0], symbol(&mut ctx, &runtime, "BLOCK")?);
    Ok(())
}
