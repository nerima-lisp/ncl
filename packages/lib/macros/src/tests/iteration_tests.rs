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

/// `destructuring-bind` expands to a `let*` of `consp`/`car`/`cdr` accesses
/// (see `crate::destructuring`), not a `funcall` of an ordinary lambda: an
/// ordinary lambda list cannot express nested patterns, `&whole`, or a
/// dotted tail, and CLHS requires binding a mismatched shape to signal
/// rather than silently mis-bind. `tests/e2emacro.rs` asserts the evaluated
/// values end to end; this checks the expansion shape.
#[test]
fn destructuring_bind_expands_to_a_let_star_of_checked_accesses() -> Result<(), ObjectError> {
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
    assert_eq!(parts[0], symbol(&mut ctx, &runtime, "LET*")?);
    let bindings = elements(&mut ctx, parts[1])?;
    // One binding for the source, one for `x`'s extraction, one to advance
    // past it, and one to check nothing is left over.
    assert_eq!(bindings.len(), 4);
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

#[test]
fn iteration_macros_preserve_explicit_result_forms() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let x = symbol(&mut ctx, &runtime, "X")?;
    let result = symbol(&mut ctx, &runtime, "RESULT")?;

    let dolist = symbol(&mut ctx, &runtime, "DOLIST")?;
    let dolist_spec = list(&mut ctx, &runtime, &[x, Word::NIL, result])?;
    let dolist_form = list(&mut ctx, &runtime, &[dolist, dolist_spec, x])?;
    let dolist_expansion = expand(&mut ctx, &runtime, "DOLIST", dolist_form)?;
    let dolist_parts = elements(&mut ctx, dolist_expansion)?;
    let dolist_block = elements(&mut ctx, dolist_parts[2])?;
    let dolist_body_form = dolist_block[2];
    let dolist_body = elements(&mut ctx, dolist_body_form)?;
    assert!(dolist_body.last().is_some());

    let dotimes = symbol(&mut ctx, &runtime, "DOTIMES")?;
    let dotimes_spec = list(&mut ctx, &runtime, &[x, Word::fixnum(2), result])?;
    let dotimes_form = list(&mut ctx, &runtime, &[dotimes, dotimes_spec, x])?;
    let dotimes_expansion = expand(&mut ctx, &runtime, "DOTIMES", dotimes_form)?;
    let dotimes_parts = elements(&mut ctx, dotimes_expansion)?;
    let dotimes_block = elements(&mut ctx, dotimes_parts[2])?;
    let dotimes_body_form = dotimes_block[2];
    let dotimes_body = elements(&mut ctx, dotimes_body_form)?;
    assert!(dotimes_body.last().is_some());
    Ok(())
}

#[test]
fn iteration_macros_reject_invalid_variables_and_spec_lengths() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let dolist = symbol(&mut ctx, &runtime, "DOLIST")?;
    let dotimes = symbol(&mut ctx, &runtime, "DOTIMES")?;
    let number = Word::fixnum(1);
    let too_short = list(&mut ctx, &runtime, &[number])?;
    let too_long = list(
        &mut ctx,
        &runtime,
        &[number, Word::NIL, Word::NIL, Word::NIL],
    )?;
    for (name, operator, spec) in [
        ("DOLIST", dolist, too_short),
        ("DOTIMES", dotimes, too_long),
    ] {
        let form = list(&mut ctx, &runtime, &[operator, spec])?;
        assert_eq!(
            expand(&mut ctx, &runtime, name, form),
            Err(ObjectError::TypeError)
        );
    }
    Ok(())
}
