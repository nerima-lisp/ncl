use ncl_object::{ObjectError, Runtime, SetfExpansion, ThreadContext, Word, make_string, make_symbol};

fn expander(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    _arguments: &[Word],
) -> Result<SetfExpansion, ObjectError> {
    Err(ObjectError::Unsupported)
}

#[test]
fn place_registry_is_runtime_scoped() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let other = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let name = make_string(&mut ctx, &runtime, &['P', 'L', 'A', 'C', 'E'])?;
    let operator = make_symbol(&mut ctx, &runtime, name)?;

    runtime.register_place_expander(&ctx, operator, expander)?;
    assert!(runtime.place_expander(&ctx, operator)?.is_some());
    assert!(other.place_expander(&ctx, operator)?.is_none());
    assert_eq!(runtime.place_expander(&ctx, Word::fixnum(1)), Err(ObjectError::TypeError));
    Ok(())
}
