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

fn string_text(ctx: &ThreadContext, string: Word) -> Result<String, ObjectError> {
    (0..ncl_object::string_length(ctx, string)?)
        .map(|index| ncl_object::string_ref(ctx, string, index))
        .collect()
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

fn source_binding(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    expansion: Word,
) -> Result<Word, ObjectError> {
    let form = elements(ctx, expansion)?;
    if form.first().copied() != Some(symbol(ctx, runtime, "LET*")?) {
        return Err(ObjectError::TypeError);
    }
    let bindings = elements(ctx, *form.get(1).ok_or(ObjectError::TypeError)?)?;
    let input_plist = symbol(ctx, runtime, "INPUT-PLIST")?;
    for binding in bindings {
        let source = elements(ctx, binding)?;
        if source.get(1).copied() == Some(input_plist) {
            if source.len() != 2 {
                return Err(ObjectError::TypeError);
            }
            return Ok(source[0]);
        }
    }
    Err(ObjectError::TypeError)
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

fn assert_keyword_lookup(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    tree: Word,
    expected: Word,
    input_list: Word,
) -> Result<(), ObjectError> {
    let labels = elements(ctx, tree)?;
    assert_eq!(labels.len(), 3);
    assert_eq!(labels[0], symbol(ctx, runtime, "LABELS")?);
    let definitions = elements(ctx, labels[1])?;
    assert_eq!(definitions.len(), 1);
    let definition = elements(ctx, definitions[0])?;
    assert_eq!(definition.len(), 3);
    let finder = definition[0];
    let parameters = elements(ctx, definition[1])?;
    assert_eq!(parameters.len(), 1);
    let param = parameters[0];

    let cond = elements(ctx, definition[2])?;
    assert_eq!(cond.len(), 4);
    assert_eq!(cond[0], symbol(ctx, runtime, "COND")?);
    let stop_clause = elements(ctx, cond[1])?;
    assert_eq!(stop_clause.len(), 2);
    assert_eq!(stop_clause[1], Word::NIL);
    let null_test = elements(ctx, stop_clause[0])?;
    assert_eq!(null_test, vec![symbol(ctx, runtime, "NULL")?, param]);

    let match_clause = elements(ctx, cond[2])?;
    assert_eq!(match_clause.len(), 2);
    assert_eq!(match_clause[1], param);
    let equality = elements(ctx, match_clause[0])?;
    assert_eq!(equality.len(), 3);
    assert_eq!(equality[0], symbol(ctx, runtime, "EQ")?);
    let car_call = elements(ctx, equality[1])?;
    assert_eq!(car_call, vec![symbol(ctx, runtime, "CAR")?, param]);
    let quoted_keyword = elements(ctx, equality[2])?;
    assert_eq!(
        quoted_keyword,
        vec![symbol(ctx, runtime, "QUOTE")?, expected]
    );

    let recurse_clause = elements(ctx, cond[3])?;
    assert_eq!(recurse_clause.len(), 2);
    assert_eq!(recurse_clause[0], symbol(ctx, runtime, "T")?);
    let recurse_call = elements(ctx, recurse_clause[1])?;
    assert_eq!(recurse_call.len(), 2);
    assert_eq!(recurse_call[0], finder);
    let cddr = elements(ctx, recurse_call[1])?;
    assert_eq!(cddr.len(), 2);
    assert_eq!(cddr[0], symbol(ctx, runtime, "CDR")?);
    let inner_cdr = elements(ctx, cddr[1])?;
    assert_eq!(inner_cdr, vec![symbol(ctx, runtime, "CDR")?, param]);

    let initial_call = elements(ctx, labels[2])?;
    assert_eq!(initial_call.len(), 2);
    assert_eq!(initial_call[0], finder);
    assert_eq!(initial_call[1], input_list);
    Ok(())
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
    let input_list = source_binding(&mut ctx, &runtime, expansion)?;
    assert_keyword_lookup(&mut ctx, &runtime, found_value, keyword, input_list)?;
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
    let input_list = source_binding(&mut ctx, &runtime, expansion)?;
    assert_keyword_lookup(&mut ctx, &runtime, found_value, keyword, input_list)?;
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
    let cursor_initial = binding_value_before(&mut ctx, &runtime, expansion, right_cursor, left)?
        .ok_or(ObjectError::TypeError)?;
    let cursor_parts = elements(&mut ctx, cursor_initial)?;
    assert_eq!(cursor_parts.len(), 4);
    assert_eq!(cursor_parts[0], symbol(&mut ctx, &runtime, "IF")?);
    let found = cursor_parts[1];
    assert_eq!(cursor_parts[3], Word::NIL);
    let found_value = elements(&mut ctx, cursor_parts[2])?;
    assert_eq!(
        found_value,
        vec![symbol(&mut ctx, &runtime, "CADR")?, found]
    );
    let finder =
        binding_value(&mut ctx, &runtime, expansion, found)?.ok_or(ObjectError::TypeError)?;
    let input_list = source_binding(&mut ctx, &runtime, expansion)?;
    assert_keyword_lookup(&mut ctx, &runtime, finder, keyword, input_list)?;
    let left_consp = elements(&mut ctx, left_parts[1])?;
    assert_eq!(left_consp[0], symbol(&mut ctx, &runtime, "CONSP")?);
    assert_eq!(left_consp[1], right_cursor);
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
    for too_short in [left_parts[3], advanced_parts[3], right_parts[3]] {
        let error = elements(&mut ctx, too_short)?;
        assert_eq!(error.len(), 2);
        assert_eq!(error[0], symbol(&mut ctx, &runtime, "ERROR")?);
        assert_eq!(
            string_text(&ctx, error[1])?,
            "destructuring-bind: too few elements"
        );
    }
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
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0], symbol(&mut ctx, &runtime, "LET*")?);
    let bindings = elements(&mut ctx, parts[1])?;
    assert_eq!(bindings.len(), 2);
    let delta = elements(&mut ctx, bindings[0])?;
    assert_eq!(delta.len(), 2);
    assert_ne!(delta[0], variable);
    assert_eq!(delta[1], Word::fixnum(5));
    let old = elements(&mut ctx, bindings[1])?;
    assert_eq!(old.len(), 2);
    assert_ne!(old[0], variable);
    assert_ne!(old[0], delta[0]);
    assert_eq!(old[1], variable);

    let store = elements(&mut ctx, parts[2])?;
    assert_eq!(store.len(), 3);
    assert_eq!(store[0], symbol(&mut ctx, &runtime, "LET")?);
    let store_bindings = elements(&mut ctx, store[1])?;
    assert_eq!(store_bindings.len(), 1);
    let store_binding = elements(&mut ctx, store_bindings[0])?;
    assert_eq!(store_binding.len(), 2);
    let value = elements(&mut ctx, store_binding[1])?;
    assert_eq!(
        value,
        vec![symbol(&mut ctx, &runtime, "+")?, old[0], delta[0]]
    );
    let assignment = elements(&mut ctx, store[2])?;
    assert_eq!(
        assignment,
        vec![
            symbol(&mut ctx, &runtime, "SETQ")?,
            variable,
            store_binding[0]
        ]
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
