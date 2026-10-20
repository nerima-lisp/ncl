use crate::{destructuring::expand_destructuring_bind, elements, list, symbol};
use ncl_object::{ObjectError, Runtime, ThreadContext, Word};

fn fixture() -> Result<(Runtime, ThreadContext), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    Ok((runtime, ctx))
}

fn expand_pattern(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    pattern: Word,
) -> Result<Word, ObjectError> {
    let value = symbol(ctx, runtime, "INPUT-PLIST")?;
    let body = symbol(ctx, runtime, "BODY")?;
    expand_destructuring_bind(ctx, runtime, pattern, value, &[body])
}

fn binding_value(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    expansion: Word,
    variable: Word,
) -> Result<Option<Word>, ObjectError> {
    let form = elements(ctx, expansion)?;
    if form.first().copied() != Some(symbol(ctx, runtime, "LET*")?) {
        return Err(ObjectError::TypeError);
    }
    let bindings = elements(ctx, *form.get(1).ok_or(ObjectError::TypeError)?)?;
    let mut value = None;
    for binding in bindings {
        let pair = elements(ctx, binding)?;
        if pair.first().copied() == Some(variable) {
            value = pair.get(1).copied();
        }
    }
    Ok(value)
}

fn binding_value_before(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    expansion: Word,
    variable: Word,
    before: Word,
) -> Result<Option<Word>, ObjectError> {
    let form = elements(ctx, expansion)?;
    if form.first().copied() != Some(symbol(ctx, runtime, "LET*")?) {
        return Err(ObjectError::TypeError);
    }
    let bindings = elements(ctx, *form.get(1).ok_or(ObjectError::TypeError)?)?;
    let mut value = None;
    for binding in bindings {
        let pair = elements(ctx, binding)?;
        if pair.first().copied() == Some(before) {
            return Ok(value);
        }
        if pair.first().copied() == Some(variable) {
            value = pair.get(1).copied();
        }
    }
    Ok(value)
}

fn contains_quoted_symbol(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    tree: Word,
    expected: Word,
) -> Result<bool, ObjectError> {
    if !tree.is_cons() {
        return Ok(false);
    }
    let parts = elements(ctx, tree)?;
    if parts.len() == 2 && parts[0] == symbol(ctx, runtime, "QUOTE")? && parts[1] == expected {
        return Ok(true);
    }
    let head = ncl_object::car(ctx, tree)?;
    if contains_quoted_symbol(ctx, runtime, head, expected)? {
        return Ok(true);
    }
    let tail = ncl_object::cdr(ctx, tree)?;
    contains_quoted_symbol(ctx, runtime, tail, expected)
}

#[test]
fn bare_key_uses_its_keyword_default_and_skips_allow_other_keys() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let key_marker = symbol(&mut ctx, &runtime, "&KEY")?;
    let name = symbol(&mut ctx, &runtime, "ITEM")?;
    let allow_other_keys = symbol(&mut ctx, &runtime, "&ALLOW-OTHER-KEYS")?;
    let pattern = list(&mut ctx, &runtime, &[key_marker, name, allow_other_keys])?;

    let expansion = expand_pattern(&mut ctx, &runtime, pattern)?;
    let value =
        binding_value(&mut ctx, &runtime, expansion, name)?.ok_or(ObjectError::TypeError)?;
    let value_parts = elements(&mut ctx, value)?;
    assert_eq!(value_parts.len(), 4);
    assert_eq!(value_parts[0], symbol(&mut ctx, &runtime, "IF")?);
    assert_eq!(value_parts[3], Word::NIL);
    let found = value_parts[1];
    let cadr = elements(&mut ctx, value_parts[2])?;
    assert_eq!(cadr[0], symbol(&mut ctx, &runtime, "CADR")?);
    assert_eq!(cadr[1], found);

    let found_value =
        binding_value(&mut ctx, &runtime, expansion, found)?.ok_or(ObjectError::TypeError)?;
    let keyword = symbol(&mut ctx, &runtime, "KEYWORD::ITEM")?;
    assert!(contains_quoted_symbol(
        &mut ctx,
        &runtime,
        found_value,
        keyword
    )?);
    assert_eq!(
        binding_value(&mut ctx, &runtime, expansion, allow_other_keys)?,
        None
    );
    Ok(())
}

#[test]
fn explicit_key_default_and_supplied_p_share_the_lookup_result() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let key_marker = symbol(&mut ctx, &runtime, "&KEY")?;
    let keyword = symbol(&mut ctx, &runtime, "KEYWORD::LIMIT")?;
    let value_name = symbol(&mut ctx, &runtime, "LIMIT")?;
    let supplied_name = symbol(&mut ctx, &runtime, "LIMIT-P")?;
    let key_name = list(&mut ctx, &runtime, &[keyword, value_name])?;
    let key_spec = list(
        &mut ctx,
        &runtime,
        &[key_name, Word::fixnum(17), supplied_name],
    )?;
    let pattern = list(&mut ctx, &runtime, &[key_marker, key_spec])?;

    let expansion = expand_pattern(&mut ctx, &runtime, pattern)?;
    let value =
        binding_value(&mut ctx, &runtime, expansion, value_name)?.ok_or(ObjectError::TypeError)?;
    let value_parts = elements(&mut ctx, value)?;
    assert_eq!(value_parts[0], symbol(&mut ctx, &runtime, "IF")?);
    assert_eq!(value_parts[3], Word::fixnum(17));
    let found = value_parts[1];
    let supplied = binding_value(&mut ctx, &runtime, expansion, supplied_name)?
        .ok_or(ObjectError::TypeError)?;
    let supplied_parts = elements(&mut ctx, supplied)?;
    assert_eq!(
        supplied_parts,
        vec![
            symbol(&mut ctx, &runtime, "IF")?,
            found,
            symbol(&mut ctx, &runtime, "T")?,
            Word::NIL,
        ]
    );

    let found_value =
        binding_value(&mut ctx, &runtime, expansion, found)?.ok_or(ObjectError::TypeError)?;
    assert!(contains_quoted_symbol(
        &mut ctx,
        &runtime,
        found_value,
        keyword
    )?);
    Ok(())
}

#[test]
fn key_values_can_bind_a_nested_pattern() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let key_marker = symbol(&mut ctx, &runtime, "&KEY")?;
    let keyword = symbol(&mut ctx, &runtime, "KEYWORD::PAIR")?;
    let left = symbol(&mut ctx, &runtime, "LEFT")?;
    let right = symbol(&mut ctx, &runtime, "RIGHT")?;
    let nested = list(&mut ctx, &runtime, &[left, right])?;
    let key_name = list(&mut ctx, &runtime, &[keyword, nested])?;
    let key_spec = list(&mut ctx, &runtime, &[key_name])?;
    let pattern = list(&mut ctx, &runtime, &[key_marker, key_spec])?;

    let expansion = expand_pattern(&mut ctx, &runtime, pattern)?;
    let left_value =
        binding_value(&mut ctx, &runtime, expansion, left)?.ok_or(ObjectError::TypeError)?;
    let left_parts = elements(&mut ctx, left_value)?;
    assert_eq!(left_parts[0], symbol(&mut ctx, &runtime, "IF")?);
    let car_call = elements(&mut ctx, left_parts[2])?;
    assert_eq!(car_call[0], symbol(&mut ctx, &runtime, "CAR")?);
    let right_value =
        binding_value(&mut ctx, &runtime, expansion, right)?.ok_or(ObjectError::TypeError)?;
    let right_parts = elements(&mut ctx, right_value)?;
    assert_eq!(right_parts[0], symbol(&mut ctx, &runtime, "IF")?);
    let right_consp = elements(&mut ctx, right_parts[1])?;
    assert_eq!(right_consp[0], symbol(&mut ctx, &runtime, "CONSP")?);
    let right_cursor = right_consp[1];
    let right_car = elements(&mut ctx, right_parts[2])?;
    assert_eq!(right_car[0], symbol(&mut ctx, &runtime, "CAR")?);
    assert_eq!(right_car[1], right_cursor);

    let advanced = binding_value_before(&mut ctx, &runtime, expansion, right_cursor, right)?
        .ok_or(ObjectError::TypeError)?;
    let advanced_parts = elements(&mut ctx, advanced)?;
    assert_eq!(advanced_parts[0], symbol(&mut ctx, &runtime, "IF")?);
    let advance_consp = elements(&mut ctx, advanced_parts[1])?;
    assert_eq!(advance_consp[0], symbol(&mut ctx, &runtime, "CONSP")?);
    assert_eq!(advance_consp[1], right_cursor);
    assert_eq!(car_call[1], right_cursor);
    let advance_cdr = elements(&mut ctx, advanced_parts[2])?;
    assert_eq!(advance_cdr[0], symbol(&mut ctx, &runtime, "CDR")?);
    assert_eq!(advance_cdr[1], right_cursor);
    let too_short = elements(&mut ctx, advanced_parts[3])?;
    assert_eq!(too_short[0], symbol(&mut ctx, &runtime, "ERROR")?);
    Ok(())
}

#[test]
fn malformed_key_specs_and_aux_markers_return_type_errors() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let key_marker = symbol(&mut ctx, &runtime, "&KEY")?;
    let aux_marker = symbol(&mut ctx, &runtime, "&AUX")?;
    let malformed = list(&mut ctx, &runtime, &[key_marker, aux_marker])?;
    let value = symbol(&mut ctx, &runtime, "INPUT-PLIST")?;
    assert_eq!(
        expand_destructuring_bind(&mut ctx, &runtime, malformed, value, &[]),
        Err(ObjectError::TypeError)
    );

    let custom_keyword = symbol(&mut ctx, &runtime, "KEYWORD::MISSING-NAME")?;
    let missing_name = list(&mut ctx, &runtime, &[custom_keyword])?;
    let key_spec = list(&mut ctx, &runtime, &[missing_name])?;
    let malformed = list(&mut ctx, &runtime, &[key_marker, key_spec])?;
    assert_eq!(
        expand_destructuring_bind(&mut ctx, &runtime, malformed, value, &[]),
        Err(ObjectError::TypeError)
    );
    Ok(())
}
