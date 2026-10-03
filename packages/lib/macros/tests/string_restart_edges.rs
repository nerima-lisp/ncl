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

fn keyword(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Result<Word, ObjectError> {
    let package = runtime.ensure_package(ctx, "KEYWORD")?;
    Ok(ncl_object::Package::from_word(package)
        .intern(ctx, runtime, name)?
        .0)
}

fn expand(
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

fn string_text(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    (0..ncl_object::string_length(ctx, word)?)
        .map(|index| ncl_object::string_ref(ctx, word, index))
        .collect()
}

fn contains_operator(ctx: &mut ThreadContext, form: Word, name: &str) -> Result<bool, ObjectError> {
    let Ok(parts) = elements(ctx, form) else {
        return Ok(false);
    };
    if let Some(operator) = parts.first().copied()
        && ncl_object::symbol_name(ctx, operator)
            .ok()
            .and_then(|word| string_text(ctx, word).ok())
            .is_some_and(|actual| actual == name)
    {
        return Ok(true);
    }
    parts.into_iter().try_fold(false, |found, part| {
        if found {
            Ok(true)
        } else {
            contains_operator(ctx, part, name)
        }
    })
}

fn operator_form(
    ctx: &mut ThreadContext,
    form: Word,
    name: &str,
) -> Result<Option<Vec<Word>>, ObjectError> {
    let Ok(parts) = elements(ctx, form) else {
        return Ok(None);
    };
    if let Some(operator) = parts.first().copied()
        && ncl_object::symbol_name(ctx, operator)
            .ok()
            .and_then(|word| string_text(ctx, word).ok())
            .is_some_and(|actual| actual == name)
    {
        return Ok(Some(parts));
    }
    for part in parts {
        if let Some(found) = operator_form(ctx, part, name)? {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

#[test]
fn registered_string_stream_macros_preserve_option_shapes() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let output_name = symbol(&mut ctx, &runtime, "WITH-OUTPUT-TO-STRING")?;
    let input_name = symbol(&mut ctx, &runtime, "WITH-INPUT-FROM-STRING")?;
    let stream = symbol(&mut ctx, &runtime, "STREAM")?;
    let position = symbol(&mut ctx, &runtime, "POSITION")?;
    let text = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b', 'c'])?;
    let element_type = keyword(&mut ctx, &runtime, "ELEMENT-TYPE")?;
    let character = symbol(&mut ctx, &runtime, "CHARACTER")?;
    let quote = symbol(&mut ctx, &runtime, "QUOTE")?;
    let quoted_character = list(&mut ctx, &runtime, &[quote, character])?;
    let unused = keyword(&mut ctx, &runtime, "UNUSED")?;
    let allow_other_keys = keyword(&mut ctx, &runtime, "ALLOW-OTHER-KEYS")?;
    let write = symbol(&mut ctx, &runtime, "WRITE-STRING")?;
    let output_body = list(&mut ctx, &runtime, &[write, text, stream])?;
    let output_spec = list(
        &mut ctx,
        &runtime,
        &[
            stream,
            text,
            element_type,
            quoted_character,
            unused,
            Word::TRUE,
            allow_other_keys,
            Word::TRUE,
        ],
    )?;
    let output_form = list(&mut ctx, &runtime, &[output_name, output_spec, output_body])?;
    let output = expand(&runtime, &mut ctx, "WITH-OUTPUT-TO-STRING", output_form)?;
    let output_parts = elements(&mut ctx, output)?;
    assert_eq!(output_parts[0], symbol(&mut ctx, &runtime, "LET")?);
    assert_eq!(elements(&mut ctx, output_parts[1])?.len(), 1);
    let output_body = elements(&mut ctx, output_parts[2])?;
    assert_eq!(output_body[0], symbol(&mut ctx, &runtime, "PROGN")?);
    assert_eq!(
        elements(&mut ctx, output_body[2])?[0],
        symbol(&mut ctx, &runtime, "GET-OUTPUT-STREAM-STRING")?
    );

    let start = keyword(&mut ctx, &runtime, "START")?;
    let end = keyword(&mut ctx, &runtime, "END")?;
    let index = keyword(&mut ctx, &runtime, "INDEX")?;
    let read = symbol(&mut ctx, &runtime, "READ-CHAR")?;
    let input_body = list(&mut ctx, &runtime, &[read, stream])?;
    let input_spec = list(
        &mut ctx,
        &runtime,
        &[
            stream,
            text,
            start,
            Word::fixnum(1),
            end,
            Word::fixnum(3),
            index,
            position,
        ],
    )?;
    let input_form = list(&mut ctx, &runtime, &[input_name, input_spec, input_body])?;
    let input = expand(&runtime, &mut ctx, "WITH-INPUT-FROM-STRING", input_form)?;
    let input_parts = elements(&mut ctx, input)?;
    assert_eq!(input_parts[0], symbol(&mut ctx, &runtime, "LET")?);
    assert_eq!(elements(&mut ctx, input_parts[1])?.len(), 2);
    let input_bindings = elements(&mut ctx, input_parts[1])?;
    let make_stream_binding = elements(&mut ctx, input_bindings[0])?;
    let make_stream = elements(&mut ctx, make_stream_binding[1])?;
    assert_eq!(
        make_stream[0],
        symbol(&mut ctx, &runtime, "MAKE-STRING-INPUT-STREAM")?
    );
    assert_eq!(make_stream[2], Word::fixnum(1));
    assert_eq!(make_stream[3], Word::fixnum(3));
    assert_eq!(
        elements(&mut ctx, input_parts[2])?[0],
        symbol(&mut ctx, &runtime, "PROG1")?
    );
    assert!(contains_operator(&mut ctx, input, "FILE-POSITION")?);
    Ok(())
}

#[test]
fn registered_restart_case_preserves_all_option_values_and_shape() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let restart_case = symbol(&mut ctx, &runtime, "RESTART-CASE")?;
    let use_value = symbol(&mut ctx, &runtime, "USE-VALUE")?;
    let value = symbol(&mut ctx, &runtime, "VALUE")?;
    let report = keyword(&mut ctx, &runtime, "REPORT-FUNCTION")?;
    let interactive = keyword(&mut ctx, &runtime, "INTERACTIVE-FUNCTION")?;
    let test = keyword(&mut ctx, &runtime, "TEST-FUNCTION")?;
    let report_value = symbol(&mut ctx, &runtime, "REPORT-VALUE")?;
    let interactive_value = symbol(&mut ctx, &runtime, "INTERACTIVE-VALUE")?;
    let test_value = symbol(&mut ctx, &runtime, "TEST-VALUE")?;
    let body = symbol(&mut ctx, &runtime, "BODY")?;
    let lambda_list = list(&mut ctx, &runtime, &[value])?;
    let clause = list(
        &mut ctx,
        &runtime,
        &[
            use_value,
            lambda_list,
            report,
            report_value,
            interactive,
            interactive_value,
            test,
            test_value,
            body,
        ],
    )?;
    let form = list(&mut ctx, &runtime, &[restart_case, body, clause])?;
    let expansion = expand(&runtime, &mut ctx, "RESTART-CASE", form)?;
    let top = elements(&mut ctx, expansion)?;
    assert_eq!(top[0], symbol(&mut ctx, &runtime, "BLOCK")?);
    let push = operator_form(&mut ctx, expansion, "PUSH-RESTART")?.ok_or(ObjectError::Layout)?;
    assert_eq!(push.len(), 6);
    let quoted_name = elements(&mut ctx, push[1])?;
    assert_eq!(quoted_name[0], symbol(&mut ctx, &runtime, "QUOTE")?);
    assert_eq!(quoted_name[1], ncl_object::symbol_name(&ctx, use_value)?);
    assert_eq!(push[3], report_value);
    assert_eq!(push[4], interactive_value);
    assert_eq!(push[5], test_value);
    assert!(contains_operator(&mut ctx, expansion, "POP-RESTART")?);
    Ok(())
}

#[test]
fn registered_macro_errors_keep_their_object_error_type() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let output_name = symbol(&mut ctx, &runtime, "WITH-OUTPUT-TO-STRING")?;
    let stream = symbol(&mut ctx, &runtime, "STREAM")?;
    let unknown = keyword(&mut ctx, &runtime, "UNKNOWN")?;
    let output_spec = list(&mut ctx, &runtime, &[stream, unknown, Word::TRUE])?;
    let output_form = list(&mut ctx, &runtime, &[output_name, output_spec, Word::NIL])?;
    assert_eq!(
        expand(&runtime, &mut ctx, "WITH-OUTPUT-TO-STRING", output_form),
        Err(ObjectError::TypeError)
    );
    let input_name = symbol(&mut ctx, &runtime, "WITH-INPUT-FROM-STRING")?;
    let text = ncl_object::make_string(&mut ctx, &runtime, &['x'])?;
    let start = keyword(&mut ctx, &runtime, "START")?;
    let duplicate = list(
        &mut ctx,
        &runtime,
        &[stream, text, start, Word::fixnum(0), start, Word::fixnum(1)],
    )?;
    let input_form = list(&mut ctx, &runtime, &[input_name, duplicate, Word::NIL])?;
    assert_eq!(
        expand(&runtime, &mut ctx, "WITH-INPUT-FROM-STRING", input_form),
        Err(ObjectError::TypeError)
    );

    let restart_case = symbol(&mut ctx, &runtime, "RESTART-CASE")?;
    let malformed_clause = list(&mut ctx, &runtime, &[])?;
    let malformed = list(
        &mut ctx,
        &runtime,
        &[restart_case, Word::NIL, malformed_clause],
    )?;
    assert_eq!(
        expand(&runtime, &mut ctx, "RESTART-CASE", malformed),
        Err(ObjectError::TypeError)
    );
    Ok(())
}
