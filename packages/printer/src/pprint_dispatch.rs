fn copy_entries(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut Word,
    copied: &mut Word,
) -> Result<(), ObjectError> {
    while *source != Word::NIL {
        let entry = car(ctx, *source)?;
        *copied = make_cons(ctx, runtime, entry, *copied)?;
        *source = cdr(ctx, *source)?;
    }
    Ok(())
}
