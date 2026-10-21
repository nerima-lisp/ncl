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
fn malformed_key_specs_and_unsupported_aux_marker_return_type_errors() -> Result<(), ObjectError> {
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

#[test]
fn registered_incf_adapter_preserves_the_place_and_delta() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    crate::register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

    let operator = symbol(&mut ctx, &runtime, "INCF")?;
    let variable = symbol(&mut ctx, &runtime, "COUNTER")?;
    let form = list(&mut ctx, &runtime, &[operator, variable, Word::fixnum(5)])?;
    let function = ncl_object::FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "INCF")
            .ok_or(ObjectError::TypeError)?,
    )?;
    let expansion = runtime.call_builtin(&mut ctx, function, &[form])?;

    let parts = elements(&mut ctx, expansion)?;
    assert_eq!(parts[0], symbol(&mut ctx, &runtime, "LET")?);
    let bindings = elements(&mut ctx, parts[1])?;
    assert_eq!(bindings.len(), 1);
    let binding = elements(&mut ctx, bindings[0])?;
    assert_eq!(binding.len(), 2);
    assert_ne!(binding[0], variable);
    let value = elements(&mut ctx, binding[1])?;
    assert_eq!(
        value,
        vec![symbol(&mut ctx, &runtime, "+")?, variable, Word::fixnum(5),]
    );
    let store = elements(&mut ctx, parts[2])?;
    assert_eq!(
        store,
        vec![symbol(&mut ctx, &runtime, "SETQ")?, variable, binding[0]]
    );
    Ok(())
}

#[test]
fn macro_adapters_reject_missing_and_empty_forms() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    for input in [&[][..], &[Word::NIL][..]] {
        let args = ncl_object::BuiltinArgs::new(input);
        let result = crate::incf_adapter(
            &mut ctx,
            &runtime,
            &args,
            &mut ncl_object::MultipleValues::new(),
        );
        assert_eq!(result, Err(ObjectError::TypeError));
    }
    Ok(())
}

#[test]
fn get_setf_expansion_builtin_returns_all_five_expansion_values() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    crate::register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

    let car = symbol(&mut ctx, &runtime, "CAR")?;
    let target = symbol(&mut ctx, &runtime, "TARGET")?;
    let place = list(&mut ctx, &runtime, &[car, target])?;
    let function = ncl_object::FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "GET-SETF-EXPANSION")
            .ok_or(ObjectError::TypeError)?,
    )?;
    let access = runtime.call_builtin(&mut ctx, function, &[place])?;
    let values = ctx.values().to_vec();
    assert_eq!(values.len(), 5);
    assert_eq!(access, values[4]);

    let temporaries = elements(&mut ctx, values[0])?;
    let value_forms = elements(&mut ctx, values[1])?;
    let stores = elements(&mut ctx, values[2])?;
    assert_eq!(temporaries.len(), 1);
    assert_eq!(value_forms, vec![target]);
    assert_eq!(stores.len(), 1);
    assert_ne!(temporaries[0], target);

    let access_form = elements(&mut ctx, values[4])?;
    assert_eq!(access_form, vec![car, temporaries[0]]);
    let store_form = elements(&mut ctx, values[3])?;
    assert_eq!(store_form[0], symbol(&mut ctx, &runtime, "PROGN")?);
    let mutation = elements(&mut ctx, store_form[1])?;
    assert_eq!(
        mutation,
        vec![
            symbol(&mut ctx, &runtime, "RPLACA")?,
            temporaries[0],
            stores[0],
        ]
    );
    assert_eq!(store_form[2], stores[0]);
    Ok(())
}
