use super::*;

#[test]
fn iteration_macros_propagate_declaration_symbol_errors() -> Result<()> {
    let runtime = Runtime::new()?;
    crate::register(&runtime)?;
    let mut ctx = ThreadContext::new();

    assert!(leading_declaration_count(&mut ctx, &runtime, std::iter::empty()).is_err());
    assert!(dolist(&mut ctx, &runtime, &[]).is_err());
    assert!(dotimes(&mut ctx, &runtime, &[]).is_err());
    Ok(())
}

#[test]
fn iteration_macros_propagate_body_car_errors() -> Result<()> {
    let foreign_runtime = Runtime::new()?;
    let mut foreign_ctx = ThreadContext::new();
    foreign_ctx.register(&foreign_runtime)?;
    let foreign_form = list(&mut foreign_ctx, &foreign_runtime, &[Word::fixnum(7)])?;

    let runtime = Runtime::new()?;
    crate::register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let variable = symbol(&mut ctx, &runtime, "ITEM")?;
    let dolist_spec = list(&mut ctx, &runtime, &[variable, Word::NIL])?;
    let dotimes_spec = list(&mut ctx, &runtime, &[variable, Word::fixnum(1)])?;

    // The body cons passes the tag check but cannot be read from this runtime's heap.
    assert!(matches!(
        dolist(&mut ctx, &runtime, &[dolist_spec, foreign_form]),
        Err(ObjectError::Storage(_))
    ));
    assert!(matches!(
        dotimes(&mut ctx, &runtime, &[dotimes_spec, foreign_form]),
        Err(ObjectError::Storage(_))
    ));
    Ok(())
}
