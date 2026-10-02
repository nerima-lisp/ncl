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

fn assert_symbol_name(ctx: &ThreadContext, word: Word, expected: &str) -> Result<(), ObjectError> {
    let name = ncl_object::symbol_name(ctx, word)?;
    let length = ncl_object::string_length(ctx, name)?;
    let actual = (0..length)
        .map(|index| ncl_object::string_ref(ctx, name, index))
        .collect::<Result<String, _>>()?;
    assert_eq!(actual, expected);
    Ok(())
}

fn conditional_body(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    form: Word,
) -> Result<Vec<Word>, ObjectError> {
    let parts = elements(ctx, form)?;
    if parts.first() == Some(&symbol(ctx, runtime, "WHEN")?) {
        elements(ctx, parts[2])
    } else {
        Ok(parts)
    }
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
fn destructuring_bind_binds_a_dotted_tail_to_the_remaining_cdr() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let operator = symbol(&mut ctx, &runtime, "DESTRUCTURING-BIND")?;
    let a = symbol(&mut ctx, &runtime, "A")?;
    let b = symbol(&mut ctx, &runtime, "B")?;
    let value = symbol(&mut ctx, &runtime, "VALUE")?;
    let result = symbol(&mut ctx, &runtime, "RESULT")?;
    let pattern = ncl_object::make_cons(&mut ctx, &runtime, a, b)?;
    let input = list(&mut ctx, &runtime, &[operator, pattern, value, result])?;
    let expansion = expand(&mut ctx, &runtime, "DESTRUCTURING-BIND", input)?;
    let let_form = elements(&mut ctx, expansion)?;
    let bindings = elements(&mut ctx, let_form[1])?;
    assert_eq!(bindings.len(), 4);
    let source_binding = elements(&mut ctx, bindings[0])?;
    let source = source_binding[0];
    assert_eq!(source_binding[1], value);
    let a_binding = elements(&mut ctx, bindings[1])?;
    let a_value = elements(&mut ctx, a_binding[1])?;
    assert_eq!(a_binding[0], a);
    assert_eq!(a_value[0], symbol(&mut ctx, &runtime, "IF")?);
    assert_eq!(
        elements(&mut ctx, a_value[1])?,
        vec![symbol(&mut ctx, &runtime, "CONSP")?, source,]
    );
    assert_eq!(
        elements(&mut ctx, a_value[2])?,
        vec![symbol(&mut ctx, &runtime, "CAR")?, source,]
    );
    let cursor_binding = elements(&mut ctx, bindings[2])?;
    assert_eq!(cursor_binding[0], source);
    let cursor_value = elements(&mut ctx, cursor_binding[1])?;
    assert_eq!(cursor_value[0], symbol(&mut ctx, &runtime, "IF")?);
    assert_eq!(
        elements(&mut ctx, cursor_value[1])?,
        vec![symbol(&mut ctx, &runtime, "CONSP")?, source,]
    );
    assert_eq!(
        elements(&mut ctx, cursor_value[2])?,
        vec![symbol(&mut ctx, &runtime, "CDR")?, source,]
    );
    let b_binding = elements(&mut ctx, bindings[3])?;
    assert_eq!(b_binding, vec![b, source]);
    assert_eq!(let_form[2], result);
    Ok(())
}

#[test]
fn loop_in_destructures_nested_elements_with_car_and_cdr_assignments() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let a = symbol(&mut ctx, &runtime, "A")?;
    let b = symbol(&mut ctx, &runtime, "B")?;
    let c = symbol(&mut ctx, &runtime, "C")?;
    let sequence = symbol(&mut ctx, &runtime, "SEQUENCE")?;
    let list_operator = symbol(&mut ctx, &runtime, "LIST")?;
    let body_form = list(&mut ctx, &runtime, &[list_operator, a, b, c])?;
    let nested_pattern = list(&mut ctx, &runtime, &[b, c])?;
    let pattern = list(&mut ctx, &runtime, &[a, nested_pattern])?;
    let expansion = super::r#loop::expand_loop_ast(
        &mut ctx,
        &runtime,
        &super::r#loop::LoopAst {
            name: None,
            clauses: vec![
                super::r#loop::LoopClause::In {
                    variable: pattern,
                    sequence,
                    on: false,
                    by: None,
                },
                super::r#loop::LoopClause::Do(vec![body_form]),
            ],
        },
    )?;
    let outer = elements(&mut ctx, expansion)?;
    let bindings = elements(&mut ctx, outer[1])?;
    assert_eq!(bindings.len(), 4);
    let cursor = elements(&mut ctx, bindings[0])?[0];
    assert_eq!(elements(&mut ctx, bindings[0])?, vec![cursor, sequence]);
    assert_eq!(elements(&mut ctx, bindings[1])?, vec![a, Word::NIL]);
    assert_eq!(elements(&mut ctx, bindings[2])?, vec![b, Word::NIL]);
    assert_eq!(elements(&mut ctx, bindings[3])?, vec![c, Word::NIL]);

    let block = elements(&mut ctx, outer[2])?;
    let progn = elements(&mut ctx, block[2])?;
    let tagbody = elements(&mut ctx, progn[1])?;
    let set_a = conditional_body(&mut ctx, &runtime, tagbody[3])?;
    assert_symbol_name(&ctx, set_a[0], "SETQ")?;
    assert_eq!(set_a[1], a);
    let car_cursor = elements(&mut ctx, set_a[2])?;
    assert_symbol_name(&ctx, car_cursor[0], "CAR")?;
    let set_b = conditional_body(&mut ctx, &runtime, tagbody[4])?;
    assert_symbol_name(&ctx, set_b[0], "SETQ")?;
    assert_eq!(set_b[1], b);
    let b_value = elements(&mut ctx, set_b[2])?;
    assert_symbol_name(&ctx, b_value[0], "CAR")?;
    let set_c = conditional_body(&mut ctx, &runtime, tagbody[5])?;
    assert_symbol_name(&ctx, set_c[0], "SETQ")?;
    assert_eq!(set_c[1], c);
    let c_value = elements(&mut ctx, set_c[2])?;
    assert_symbol_name(&ctx, c_value[0], "CAR")?;
    assert_eq!(tagbody[6], body_form);
    let update_cursor = elements(&mut ctx, tagbody[7])?;
    assert_symbol_name(&ctx, update_cursor[0], "SETQ")?;
    assert_eq!(update_cursor[1], cursor);
    let next_cursor = elements(&mut ctx, update_cursor[2])?;
    assert_symbol_name(&ctx, next_cursor[0], "CDR")?;
    assert_eq!(next_cursor[1], cursor);
    let go_form = elements(&mut ctx, tagbody[8])?;
    assert_symbol_name(&ctx, go_form[0], "GO")?;
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
