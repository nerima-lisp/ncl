use super::*;

fn expand(ctx: &mut ThreadContext, runtime: &Runtime, input: Word) -> Result<Word, ObjectError> {
    let input_words = [input];
    let args = ncl_object::BuiltinArgs::new(&input_words);
    with_open_file::expand_adapter(ctx, runtime, &args, &mut ncl_object::MultipleValues::new())
}

#[test]
fn expands_open_once_and_closes_only_a_non_nil_stream() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let with_open_file = symbol(&mut ctx, &runtime, "WITH-OPEN-FILE")?;
    let stream = symbol(&mut ctx, &runtime, "STREAM")?;
    let path = symbol(&mut ctx, &runtime, "PATH")?;
    let body = symbol(&mut ctx, &runtime, "BODY")?;
    let spec = list(&mut ctx, &runtime, &[stream, path])?;
    let input = list(&mut ctx, &runtime, &[with_open_file, spec, body])?;

    let expansion = expand(&mut ctx, &runtime, input)?;
    let let_parts = elements(&mut ctx, expansion)?;
    assert_eq!(let_parts[0], symbol(&mut ctx, &runtime, "LET")?);
    let bindings = elements(&mut ctx, let_parts[1])?;
    let binding = elements(&mut ctx, bindings[0])?;
    assert_eq!(binding[0], stream);
    let open = elements(&mut ctx, binding[1])?;
    assert_eq!(open[0], symbol(&mut ctx, &runtime, "OPEN")?);
    assert_eq!(open[1], path);

    let unwind = elements(&mut ctx, let_parts[2])?;
    assert_eq!(unwind[0], symbol(&mut ctx, &runtime, "UNWIND-PROTECT")?);
    let cleanup = elements(&mut ctx, unwind[2])?;
    assert_eq!(cleanup[0], symbol(&mut ctx, &runtime, "WHEN")?);
    let close = elements(&mut ctx, cleanup[2])?;
    assert_eq!(close, vec![symbol(&mut ctx, &runtime, "CLOSE")?, stream]);
    Ok(())
}

#[test]
fn rejects_missing_stream_spec() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let operator = symbol(&mut ctx, &runtime, "WITH-OPEN-FILE")?;
    let input = list(&mut ctx, &runtime, &[operator])?;
    assert_eq!(
        expand(&mut ctx, &runtime, input),
        Err(ObjectError::TypeError)
    );
    Ok(())
}
