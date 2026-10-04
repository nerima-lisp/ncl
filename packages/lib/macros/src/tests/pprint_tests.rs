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
fn pprint_macros_are_registered_and_have_ansi_shapes() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let logical = symbol(&mut ctx, &runtime, "PPRINT-LOGICAL-BLOCK")?;
    let stream = symbol(&mut ctx, &runtime, "STREAM")?;
    let object = symbol(&mut ctx, &runtime, "OBJECT")?;
    let prefix = symbol(&mut ctx, &runtime, "PREFIX")?;
    let keyword_prefix = symbol(&mut ctx, &runtime, "KEYWORD::PREFIX")?;
    let body = symbol(&mut ctx, &runtime, "BODY")?;
    let spec = list(
        &mut ctx,
        &runtime,
        &[stream, object, keyword_prefix, prefix],
    )?;
    let input = list(&mut ctx, &runtime, &[logical, spec, body])?;
    let expanded = expand(&mut ctx, &runtime, "PPRINT-LOGICAL-BLOCK", input)?;
    let expansion = elements(&mut ctx, expanded)?;
    assert_eq!(
        expansion[0],
        symbol(&mut ctx, &runtime, "NCL-EXT::PPRINT-LOGICAL-BLOCK")?
    );
    let thunk = elements(&mut ctx, *expansion.last().ok_or(ObjectError::Layout)?)?;
    assert_eq!(thunk[0], symbol(&mut ctx, &runtime, "LAMBDA")?);
    let macrolet = elements(&mut ctx, *thunk.get(2).ok_or(ObjectError::Layout)?)?;
    assert_eq!(macrolet[0], symbol(&mut ctx, &runtime, "MACROLET")?);
    assert_eq!(
        MACROS
            .iter()
            .filter(|name| name.starts_with("PPRINT-"))
            .count(),
        3
    );
    Ok(())
}

#[test]
fn pprint_local_macros_require_no_arguments() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    for name in ["PPRINT-POP", "PPRINT-EXIT-IF-LIST-EXHAUSTED"] {
        let operator = symbol(&mut ctx, &runtime, name)?;
        let input = list(&mut ctx, &runtime, &[operator])?;
        assert!(expand(&mut ctx, &runtime, name, input).is_ok());
        let argument = symbol(&mut ctx, &runtime, "ARGUMENT")?;
        let input = list(&mut ctx, &runtime, &[operator, argument])?;
        assert_eq!(
            expand(&mut ctx, &runtime, name, input),
            Err(ObjectError::TypeError)
        );
    }
    Ok(())
}
