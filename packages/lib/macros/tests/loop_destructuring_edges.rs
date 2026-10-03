#![allow(missing_docs)]

use ncl_lib_macros::{elements, list, register, symbol};
use ncl_object::{FunctionObject, ObjectError, Runtime, ThreadContext, Word};

fn fixture() -> Result<(Runtime, ThreadContext), ObjectError> {
    let runtime = Runtime::new()?;
    register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    Ok((runtime, ctx))
}

fn macroexpand(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    form: Word,
) -> Result<Word, ObjectError> {
    let function = runtime
        .function(ctx, "COMMON-LISP", name)
        .ok_or(ObjectError::UndefinedFunction)
        .and_then(|word| FunctionObject::try_from(word).map_err(|_| ObjectError::TypeError))?;
    runtime.call_builtin(ctx, function, &[form])
}

fn contains_word(ctx: &ThreadContext, form: Word, needle: Word) -> Result<bool, ObjectError> {
    if form == needle || !form.is_cons() {
        return Ok(form == needle);
    }
    Ok(ncl_object::car(ctx, form)? == needle
        || contains_word(ctx, ncl_object::car(ctx, form)?, needle)?
        || contains_word(ctx, ncl_object::cdr(ctx, form)?, needle)?)
}

fn binding_for(
    ctx: &mut ThreadContext,
    bindings: Word,
    name: Word,
) -> Result<Vec<Word>, ObjectError> {
    let mut cursor = bindings;
    while cursor != Word::NIL {
        let binding = ncl_object::car(ctx, cursor)?;
        let parts = elements(ctx, binding)?;
        if parts.first() == Some(&name) {
            return Ok(parts);
        }
        cursor = ncl_object::cdr(ctx, cursor)?;
    }
    Err(ObjectError::TypeError)
}

#[test]
fn public_destructuring_expansion_handles_optional_body_and_dotted_tail() -> Result<(), ObjectError>
{
    let (runtime, mut ctx) = fixture()?;
    let operator = symbol(&mut ctx, &runtime, "DESTRUCTURING-BIND")?;
    let optional = symbol(&mut ctx, &runtime, "&OPTIONAL")?;
    let body = symbol(&mut ctx, &runtime, "&BODY")?;
    let a = symbol(&mut ctx, &runtime, "A")?;
    let default = symbol(&mut ctx, &runtime, "DEFAULT")?;
    let supplied = symbol(&mut ctx, &runtime, "SUPPLIED")?;
    let rest = symbol(&mut ctx, &runtime, "REST")?;
    let value = symbol(&mut ctx, &runtime, "VALUE")?;
    let result = symbol(&mut ctx, &runtime, "RESULT")?;
    let optional_spec = list(&mut ctx, &runtime, &[a, default, supplied])?;
    let pattern = list(&mut ctx, &runtime, &[optional, optional_spec, body, rest])?;
    let form = list(&mut ctx, &runtime, &[operator, pattern, value, result])?;

    let expansion = macroexpand(&runtime, &mut ctx, "DESTRUCTURING-BIND", form)?;
    let parts = elements(&mut ctx, expansion)?;
    assert_eq!(parts[0], symbol(&mut ctx, &runtime, "LET*")?);
    assert_eq!(parts[2], result);
    let bindings = parts[1];
    let first_binding = ncl_object::car(&ctx, bindings)?;
    let first_parts = elements(&mut ctx, first_binding)?;
    let source = first_parts[0];
    assert_eq!(first_parts[1], value);

    let a_binding = binding_for(&mut ctx, bindings, a)?;
    assert_eq!(
        elements(&mut ctx, a_binding[1])?[0],
        symbol(&mut ctx, &runtime, "IF")?
    );
    let supplied_binding = binding_for(&mut ctx, bindings, supplied)?;
    assert_eq!(
        elements(&mut ctx, supplied_binding[1])?[0],
        symbol(&mut ctx, &runtime, "IF")?
    );
    let rest_binding = binding_for(&mut ctx, bindings, rest)?;
    assert_eq!(rest_binding[1], source);

    let dotted_pattern = ncl_object::make_cons(&mut ctx, &runtime, a, rest)?;
    let dotted_form = list(
        &mut ctx,
        &runtime,
        &[operator, dotted_pattern, value, result],
    )?;
    let dotted_expansion = macroexpand(&runtime, &mut ctx, "DESTRUCTURING-BIND", dotted_form)?;
    let dotted_parts = elements(&mut ctx, dotted_expansion)?;
    let dotted_bindings = dotted_parts[1];
    assert_eq!(binding_for(&mut ctx, dotted_bindings, a)?[0], a);
    assert_eq!(binding_for(&mut ctx, dotted_bindings, rest)?[0], rest);
    let unless = symbol(&mut ctx, &runtime, "UNLESS")?;
    assert!(!contains_word(&ctx, dotted_expansion, unless)?);
    Ok(())
}

#[test]
fn public_loop_expansion_emits_destructuring_updates_and_it_conditionals() -> Result<(), ObjectError>
{
    let (runtime, mut ctx) = fixture()?;
    let operator = symbol(&mut ctx, &runtime, "LOOP")?;
    let for_word = symbol(&mut ctx, &runtime, "FOR")?;
    let in_word = symbol(&mut ctx, &runtime, "IN")?;
    let when = symbol(&mut ctx, &runtime, "WHEN")?;
    let do_word = symbol(&mut ctx, &runtime, "DO")?;
    let collect = symbol(&mut ctx, &runtime, "COLLECT")?;
    let else_word = symbol(&mut ctx, &runtime, "ELSE")?;
    let return_word = symbol(&mut ctx, &runtime, "RETURN")?;
    let end = symbol(&mut ctx, &runtime, "END")?;
    let a = symbol(&mut ctx, &runtime, "A")?;
    let b = symbol(&mut ctx, &runtime, "B")?;
    let sequence = symbol(&mut ctx, &runtime, "SEQUENCE")?;
    let test = symbol(&mut ctx, &runtime, "TEST")?;
    let it = symbol(&mut ctx, &runtime, "IT")?;
    let result = symbol(&mut ctx, &runtime, "RESULT")?;
    let pattern = list(&mut ctx, &runtime, &[a, b])?;
    let clauses = [
        for_word,
        pattern,
        in_word,
        sequence,
        when,
        test,
        collect,
        it,
        else_word,
        return_word,
        result,
        end,
        do_word,
        it,
    ];
    let mut values = Vec::with_capacity(clauses.len() + 1);
    values.push(operator);
    values.extend(clauses);
    let form = list(&mut ctx, &runtime, &values)?;

    let expansion = macroexpand(&runtime, &mut ctx, "LOOP", form)?;
    assert_eq!(
        elements(&mut ctx, expansion)?[0],
        symbol(&mut ctx, &runtime, "LET")?
    );
    for name in ["CAR", "CDR", "SETQ", "IF", "RETURN-FROM", "PUSH"] {
        let operator = symbol(&mut ctx, &runtime, name)?;
        assert!(
            contains_word(&ctx, expansion, operator)?,
            "missing {name} in loop expansion"
        );
    }
    assert!(contains_word(&ctx, expansion, it)?);
    Ok(())
}

#[test]
fn public_loop_and_destructuring_malformed_forms_return_type_error() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let value = symbol(&mut ctx, &runtime, "VALUE")?;
    let result = symbol(&mut ctx, &runtime, "RESULT")?;
    let destructuring = symbol(&mut ctx, &runtime, "DESTRUCTURING-BIND")?;
    let rest = symbol(&mut ctx, &runtime, "&REST")?;
    let rest_name = symbol(&mut ctx, &runtime, "REST")?;
    let trailing = symbol(&mut ctx, &runtime, "TRAILING")?;
    let malformed_pattern = list(&mut ctx, &runtime, &[rest, rest_name, trailing])?;
    let malformed = list(
        &mut ctx,
        &runtime,
        &[destructuring, malformed_pattern, value, result],
    )?;
    assert_eq!(
        macroexpand(&runtime, &mut ctx, "DESTRUCTURING-BIND", malformed),
        Err(ObjectError::TypeError)
    );

    let loop_operator = symbol(&mut ctx, &runtime, "LOOP")?;
    let for_word = symbol(&mut ctx, &runtime, "FOR")?;
    let being = symbol(&mut ctx, &runtime, "BEING")?;
    let hash_keys = symbol(&mut ctx, &runtime, "HASH-KEYS")?;
    let of = symbol(&mut ctx, &runtime, "OF")?;
    let table = symbol(&mut ctx, &runtime, "TABLE")?;
    let key = symbol(&mut ctx, &runtime, "KEY")?;
    let value_name = symbol(&mut ctx, &runtime, "VALUE-NAME")?;
    let hash_values = symbol(&mut ctx, &runtime, "HASH-VALUES")?;
    let loop_form = list(
        &mut ctx,
        &runtime,
        &[
            loop_operator,
            for_word,
            key,
            being,
            hash_keys,
            of,
            table,
            for_word,
            value_name,
            being,
            hash_values,
            of,
            table,
        ],
    )?;
    assert_eq!(
        macroexpand(&runtime, &mut ctx, "LOOP", loop_form),
        Err(ObjectError::TypeError)
    );
    Ok(())
}
