//! Regression tests for runtime-owned generalized-reference expanders.

use ncl_object::{
    ObjectError, Runtime, SetfExpansion, ThreadContext, Word, make_string, make_symbol, push_root,
};

fn expander(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    arguments: &[Word],
) -> Result<SetfExpansion, ObjectError> {
    let argument = arguments.first().copied().ok_or(ObjectError::TypeError)?;
    if runtime.place_expander(ctx, argument)?.is_none() {
        return Err(ObjectError::Layout);
    }
    let mut rooted_argument = argument;
    ncl_object::with_root(ctx, &mut rooted_argument, |ctx, rooted_argument| {
        let marker = make_string(ctx, runtime, &['O', 'K'])?;
        Ok(SetfExpansion {
            temporary_variables: Vec::new(),
            value_forms: Vec::new(),
            store_variables: vec![*rooted_argument],
            store_form: marker,
            access_form: *rooted_argument,
        })
    })
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
    assert_eq!(
        runtime.place_expander(&ctx, Word::fixnum(1)),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn registered_place_expander_follows_symbol_after_full_collection() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);

    let name = make_string(&mut ctx, &runtime, &['M', 'O', 'V', 'E'])?;
    let mut operator = make_symbol(&mut ctx, &runtime, name)?;
    let operator_root = push_root(&mut ctx, &mut operator);
    runtime.register_place_expander(&ctx, operator, expander)?;

    let registered_operator = operator;
    ctx.collect(true)?;
    assert_ne!(operator, registered_operator);
    let callback = runtime
        .place_expander(&ctx, operator)?
        .ok_or(ObjectError::Layout)?;
    let expansion = callback(&mut ctx, &runtime, &[operator])?;
    assert_eq!(expansion.store_variables, vec![operator]);
    assert_eq!(expansion.access_form, operator);
    assert_eq!(ncl_object::string_length(&ctx, expansion.store_form), Ok(2));

    assert!(ncl_object::pop_root(&mut ctx, operator_root));
    Ok(())
}
