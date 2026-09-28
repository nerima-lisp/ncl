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
    let let_head = *let_parts.first().ok_or(ObjectError::TypeError)?;
    if let_head != symbol(&mut ctx, &runtime, "LET")? { return Err(ObjectError::TypeError); }
    let bindings = elements(&mut ctx, *let_parts.get(1).ok_or(ObjectError::TypeError)?)?;
    let binding = elements(&mut ctx, *bindings.first().ok_or(ObjectError::TypeError)?)?;
    if *binding.first().ok_or(ObjectError::TypeError)? != stream { return Err(ObjectError::TypeError); }
    let open = elements(&mut ctx, *binding.get(1).ok_or(ObjectError::TypeError)?)?;
    if *open.first().ok_or(ObjectError::TypeError)? != symbol(&mut ctx, &runtime, "OPEN")? { return Err(ObjectError::TypeError); }
    if *open.get(1).ok_or(ObjectError::TypeError)? != path { return Err(ObjectError::TypeError); }

    let unwind = elements(&mut ctx, *let_parts.get(2).ok_or(ObjectError::TypeError)?)?;
    if *unwind.first().ok_or(ObjectError::TypeError)? != symbol(&mut ctx, &runtime, "UNWIND-PROTECT")? { return Err(ObjectError::TypeError); }
    let cleanup = elements(&mut ctx, *unwind.get(2).ok_or(ObjectError::TypeError)?)?;
    if *cleanup.first().ok_or(ObjectError::TypeError)? != symbol(&mut ctx, &runtime, "WHEN")? { return Err(ObjectError::TypeError); }
    let close = elements(&mut ctx, *cleanup.get(2).ok_or(ObjectError::TypeError)?)?;
    if close != vec![symbol(&mut ctx, &runtime, "CLOSE")?, stream] { return Err(ObjectError::TypeError); }
    Ok(())
}

#[test]
fn rejects_missing_stream_spec() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let operator = symbol(&mut ctx, &runtime, "WITH-OPEN-FILE")?;
    let input = list(&mut ctx, &runtime, &[operator])?;
    if expand(&mut ctx, &runtime, input) != Err(ObjectError::TypeError) {
        return Err(ObjectError::TypeError);
    }
    Ok(())
}
