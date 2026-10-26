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
fn do_symbols_macros_expand_with_declarations_and_blocks() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

    let variable = symbol(&mut ctx, &runtime, "SYMBOL")?;
    let declare = symbol(&mut ctx, &runtime, "DECLARE")?;
    let special = symbol(&mut ctx, &runtime, "SPECIAL")?;
    let special_spec = list(&mut ctx, &runtime, &[special, variable])?;
    let declaration = list(&mut ctx, &runtime, &[declare, special_spec])?;
    let block = symbol(&mut ctx, &runtime, "BLOCK")?;
    let let_symbol = symbol(&mut ctx, &runtime, "LET")?;

    for name in ["DO-SYMBOLS", "DO-EXTERNAL-SYMBOLS", "DO-ALL-SYMBOLS"] {
        let operator = symbol(&mut ctx, &runtime, name)?;
        let spec = list(&mut ctx, &runtime, &[variable])?;
        let input = list(&mut ctx, &runtime, &[operator, spec, declaration, variable])?;
        let expansion = expand(&mut ctx, &runtime, name, input)?;
        let parts = elements(&mut ctx, expansion)?;
        assert_eq!(parts[0], block, "{name}");
        let body = elements(&mut ctx, parts[2])?;
        assert_eq!(body[0], let_symbol, "{name}");
        assert_eq!(body[2], declaration, "{name}");

        let no_spec = list(&mut ctx, &runtime, &[operator])?;
        assert_eq!(
            expand(&mut ctx, &runtime, name, no_spec),
            Err(ObjectError::TypeError),
            "{name}"
        );

        let malformed_input = list(&mut ctx, &runtime, &[operator, Word::fixnum(1)])?;
        assert_eq!(
            expand(&mut ctx, &runtime, name, malformed_input),
            Err(ObjectError::TypeError),
            "{name}"
        );

        let bad_spec = list(&mut ctx, &runtime, &[])?;
        let bad_input = list(&mut ctx, &runtime, &[operator, bad_spec])?;
        assert_eq!(
            expand(&mut ctx, &runtime, name, bad_input),
            Err(ObjectError::TypeError),
            "{name}"
        );

        let invalid_variable = list(&mut ctx, &runtime, &[Word::fixnum(1)])?;
        let invalid_input = list(&mut ctx, &runtime, &[operator, invalid_variable])?;
        assert_eq!(
            expand(&mut ctx, &runtime, name, invalid_input),
            Err(ObjectError::TypeError),
            "{name}"
        );
    }
    Ok(())
}
