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

fn assert_declaration_placement(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    macro_name: &str,
    binding_operator: &str,
) -> Result<(), ObjectError> {
    let operator = symbol(ctx, runtime, macro_name)?;
    let block_operator = symbol(ctx, runtime, "BLOCK")?;
    let let_operator = symbol(ctx, runtime, binding_operator)?;
    let progn_operator = symbol(ctx, runtime, "PROGN")?;
    let tagbody_operator = symbol(ctx, runtime, "TAGBODY")?;
    let variable = symbol(ctx, runtime, "X")?;
    let misplaced_variable = symbol(ctx, runtime, "Y")?;
    let declare = symbol(ctx, runtime, "DECLARE")?;
    let ignore = symbol(ctx, runtime, "IGNORE")?;
    let declaration_spec = list(ctx, runtime, &[ignore, variable])?;
    let leading_declaration = list(ctx, runtime, &[declare, declaration_spec])?;
    let leading_declaration = ncl_object::HeapRootedWord::new(ctx, leading_declaration)?;
    let declaration_spec = list(ctx, runtime, &[ignore, variable])?;
    let second_leading_declaration = list(ctx, runtime, &[declare, declaration_spec])?;
    let second_leading_declaration =
        ncl_object::HeapRootedWord::new(ctx, second_leading_declaration)?;
    let misplaced_spec = list(ctx, runtime, &[ignore, misplaced_variable])?;
    let misplaced_declaration = list(ctx, runtime, &[declare, misplaced_spec])?;
    let misplaced_declaration = ncl_object::HeapRootedWord::new(ctx, misplaced_declaration)?;
    let before = symbol(ctx, runtime, "BEFORE")?;
    let after = symbol(ctx, runtime, "AFTER")?;
    let mut prefix = ncl_object::HeapRootedWords::default();
    match macro_name {
        "DOLIST" => {
            let spec = list(ctx, runtime, &[variable, Word::NIL])?;
            prefix.push(ctx, spec)?;
        }
        "DOTIMES" => {
            let spec = list(ctx, runtime, &[variable, Word::fixnum(1)])?;
            prefix.push(ctx, spec)?;
        }
        "DO" | "DO*" => {
            let variable_spec = list(ctx, runtime, &[variable, Word::fixnum(0)])?;
            let variable_spec = ncl_object::HeapRootedWord::new(ctx, variable_spec)?;
            let variable_specs = list(ctx, runtime, &[variable_spec.get()])?;
            prefix.push(ctx, variable_specs)?;
            let end_clause = list(ctx, runtime, &[Word::NIL])?;
            prefix.push(ctx, end_clause)?;
        }
        _ => return Err(ObjectError::TypeError),
    }
    let mut input_values = vec![operator];
    input_values.extend(prefix.iter().map(ncl_object::HeapRootedWord::get));
    input_values.extend([
        leading_declaration.get(),
        second_leading_declaration.get(),
        before,
        misplaced_declaration.get(),
        after,
    ]);
    let input = list(ctx, runtime, &input_values)?;
    let expansion_word = expand(ctx, runtime, macro_name, input)?;
    let expansion = ncl_object::HeapRootedWord::new(ctx, expansion_word)?;
    let block_parts = elements(ctx, expansion.get())?;
    assert_eq!(block_parts[0], block_operator);
    let let_form = ncl_object::HeapRootedWord::new(ctx, block_parts[2])?;
    let let_parts = elements(ctx, let_form.get())?;
    assert_eq!(let_parts[0], let_operator);
    assert_eq!(let_parts[2], leading_declaration.get());
    assert_eq!(let_parts[3], second_leading_declaration.get());
    let let_body = ncl_object::HeapRootedWord::new(ctx, let_parts[4])?;
    let tagbody = if macro_name == "DOLIST" {
        let progn_parts = elements(ctx, let_body.get())?;
        assert_eq!(progn_parts[0], progn_operator);
        ncl_object::HeapRootedWord::new(ctx, progn_parts[1])?
    } else {
        let_body
    };
    let tagbody_parts = elements(ctx, tagbody.get())?;
    assert_eq!(tagbody_parts[0], tagbody_operator);
    let before_index = tagbody_parts
        .iter()
        .position(|form| *form == before)
        .ok_or(ObjectError::TypeError)?;
    assert_eq!(tagbody_parts[before_index + 1], misplaced_declaration.get());
    assert_eq!(tagbody_parts[before_index + 2], after);
    Ok(())
}

#[test]
fn iteration_macros_move_only_leading_declarations_before_tagbody() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

    assert_declaration_placement(&mut ctx, &runtime, "DOLIST", "LET")?;
    assert_declaration_placement(&mut ctx, &runtime, "DOTIMES", "LET")?;
    assert_declaration_placement(&mut ctx, &runtime, "DO", "LET")?;
    assert_declaration_placement(&mut ctx, &runtime, "DO*", "LET*")?;
    Ok(())
}

#[test]
fn dotimes_keeps_declarations_after_a_leading_string_in_tagbody() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

    let dotimes = symbol(&mut ctx, &runtime, "DOTIMES")?;
    let variable = symbol(&mut ctx, &runtime, "X")?;
    let spec = list(&mut ctx, &runtime, &[variable, Word::fixnum(1)])?;
    let string_form =
        ncl_object::make_string(&mut ctx, &runtime, &"body".chars().collect::<Vec<_>>())?;
    let declare = symbol(&mut ctx, &runtime, "DECLARE")?;
    let ignore = symbol(&mut ctx, &runtime, "IGNORE")?;
    let declaration_spec = list(&mut ctx, &runtime, &[ignore, variable])?;
    let declaration = list(&mut ctx, &runtime, &[declare, declaration_spec])?;
    let input = list(
        &mut ctx,
        &runtime,
        &[dotimes, spec, string_form, declaration],
    )?;
    let expansion = expand(&mut ctx, &runtime, "DOTIMES", input)?;
    let block_parts = elements(&mut ctx, expansion)?;
    let let_form = ncl_object::HeapRootedWord::new(&ctx, block_parts[2])?;
    let let_parts = elements(&mut ctx, let_form.get())?;
    let tagbody = ncl_object::HeapRootedWord::new(&ctx, let_parts[2])?;
    let tagbody_parts = elements(&mut ctx, tagbody.get())?;

    assert_eq!(tagbody_parts[0], symbol(&mut ctx, &runtime, "TAGBODY")?);
    assert_eq!(tagbody_parts[3], string_form);
    assert_eq!(tagbody_parts[4], declaration);
    Ok(())
}
