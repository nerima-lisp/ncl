use super::*;
use ncl_object::MultipleValues;

fn expand(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    input: Word,
) -> Result<Word, ObjectError> {
    let callback = callback_for(name).ok_or(ObjectError::UndefinedFunction)?;
    let input_args = [input];
    let args = ncl_object::BuiltinArgs::new(&input_args);
    callback(ctx, runtime, &args, &mut MultipleValues::new())
}

fn keyword(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Result<Word, ObjectError> {
    let package = runtime.ensure_package(ctx, "KEYWORD")?;
    Ok(ncl_object::Package::from_word(package)
        .intern(ctx, runtime, name)?
        .0)
}

fn quoted(ctx: &mut ThreadContext, runtime: &Runtime, value: Word) -> Result<Word, ObjectError> {
    let quote = symbol(ctx, runtime, "QUOTE")?;
    list(ctx, runtime, &[quote, value])
}

fn contains_word(ctx: &mut ThreadContext, form: Word, needle: Word) -> Result<bool, ObjectError> {
    if form == needle || !form.is_cons() {
        return Ok(form == needle);
    }
    let mut cursor = form;
    loop {
        if !cursor.is_cons() {
            return if cursor == Word::NIL {
                Ok(false)
            } else {
                contains_word(ctx, cursor, needle)
            };
        }
        if contains_word(ctx, ncl_object::car(ctx, cursor)?, needle)? {
            return Ok(true);
        }
        cursor = ncl_object::cdr(ctx, cursor)?;
    }
}

#[test]
fn string_stream_macros_expand_to_stream_builtin_forms() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

    let output = symbol(&mut ctx, &runtime, "WITH-OUTPUT-TO-STRING")?;
    let let_symbol = symbol(&mut ctx, &runtime, "LET")?;
    let make_input_symbol = symbol(&mut ctx, &runtime, "MAKE-STRING-INPUT-STREAM")?;
    let stream = symbol(&mut ctx, &runtime, "S")?;
    let write = symbol(&mut ctx, &runtime, "WRITE-STRING")?;
    let text = ncl_object::make_string(&mut ctx, &runtime, &['x'])?;
    let body = list(&mut ctx, &runtime, &[write, text, stream])?;
    let output_spec = list(&mut ctx, &runtime, &[stream])?;
    let output_form = list(&mut ctx, &runtime, &[output, output_spec, body])?;
    let expansion =
        expand(&mut ctx, &runtime, "WITH-OUTPUT-TO-STRING", output_form).expect("output expansion");
    let parts = elements(&mut ctx, expansion)?;
    assert_eq!(parts[0], let_symbol);
    assert!(elements(&mut ctx, parts[1])?[0] != Word::NIL);
    let body = elements(&mut ctx, parts[2])?;
    assert_eq!(body[0], symbol(&mut ctx, &runtime, "PROGN")?);
    assert_eq!(
        elements(&mut ctx, body[2])?[0],
        symbol(&mut ctx, &runtime, "GET-OUTPUT-STREAM-STRING")?
    );

    let input = symbol(&mut ctx, &runtime, "WITH-INPUT-FROM-STRING")?;
    let text = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b', 'c']).expect("input text");
    let read = symbol(&mut ctx, &runtime, "READ-CHAR").expect("read-char");
    let keyword_package = runtime
        .ensure_package(&mut ctx, "KEYWORD")
        .expect("keyword package");
    let start = ncl_object::Package::from_word(keyword_package)
        .intern(&mut ctx, &runtime, "START")
        .expect("start keyword")
        .0;
    let body = list(&mut ctx, &runtime, &[read, stream]).expect("input body");
    let input_spec =
        list(&mut ctx, &runtime, &[stream, text, start, Word::fixnum(1)]).expect("input spec");
    let input_form = list(&mut ctx, &runtime, &[input, input_spec, body]).expect("input form");
    let expansion =
        expand(&mut ctx, &runtime, "WITH-INPUT-FROM-STRING", input_form).expect("input expansion");
    let parts = elements(&mut ctx, expansion).expect("input expansion list");
    assert_eq!(parts[0], let_symbol);
    let binding = elements(&mut ctx, parts[1]).expect("input bindings");
    let binding_head = elements(&mut ctx, binding[0]).expect("input binding head");
    assert_eq!(binding_head[0], stream);
    let binding_parts = elements(&mut ctx, binding[0]).expect("input binding");
    let stream_call = elements(&mut ctx, binding_parts[1]).expect("input stream call");
    assert_eq!(stream_call[0], make_input_symbol);
    Ok(())
}

#[test]
fn input_stream_macro_rejects_unknown_keywords_as_program_errors() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

    let input = symbol(&mut ctx, &runtime, "WITH-INPUT-FROM-STRING")?;
    let stream = symbol(&mut ctx, &runtime, "S")?;
    let text = ncl_object::make_string(&mut ctx, &runtime, &['a'])?;
    let read = symbol(&mut ctx, &runtime, "READ-CHAR")?;
    let body = list(&mut ctx, &runtime, &[read, stream])?;
    let non_keyword_start = symbol(&mut ctx, &runtime, "START")?;
    let unknown = runtime.ensure_package(&mut ctx, "KEYWORD")?;
    let unknown = ncl_object::Package::from_word(unknown)
        .intern(&mut ctx, &runtime, "UNKNOWN")?
        .0;
    let input_spec = list(
        &mut ctx,
        &runtime,
        &[
            stream,
            text,
            non_keyword_start,
            Word::TRUE,
            unknown,
            Word::TRUE,
        ],
    )?;
    let input_form = list(&mut ctx, &runtime, &[input, input_spec, body])?;

    assert_eq!(
        expand(&mut ctx, &runtime, "WITH-INPUT-FROM-STRING", input_form),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        ctx.take_pending_lisp_error(),
        Some(ncl_object::LispError::ProgramError(
            ncl_object::ProgramError::UnknownKeyword
        ))
    );
    Ok(())
}

#[test]
fn input_stream_macro_allows_unknown_keywords_with_allow_other_keys() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

    let input = symbol(&mut ctx, &runtime, "WITH-INPUT-FROM-STRING")?;
    let stream = symbol(&mut ctx, &runtime, "S")?;
    let text = ncl_object::make_string(&mut ctx, &runtime, &['a'])?;
    let read = symbol(&mut ctx, &runtime, "READ-CHAR")?;
    let body = list(&mut ctx, &runtime, &[read, stream])?;
    let keyword_package = runtime.ensure_package(&mut ctx, "KEYWORD")?;
    let unknown = ncl_object::Package::from_word(keyword_package)
        .intern(&mut ctx, &runtime, "UNKNOWN")?
        .0;
    let allow_other_keys = ncl_object::Package::from_word(keyword_package)
        .intern(&mut ctx, &runtime, "ALLOW-OTHER-KEYS")?
        .0;
    let input_spec = list(
        &mut ctx,
        &runtime,
        &[
            stream,
            text,
            unknown,
            Word::TRUE,
            allow_other_keys,
            Word::TRUE,
        ],
    )?;
    let input_form = list(&mut ctx, &runtime, &[input, input_spec, body])?;

    assert!(expand(&mut ctx, &runtime, "WITH-INPUT-FROM-STRING", input_form).is_ok());
    assert_eq!(ctx.take_pending_lisp_error(), None);
    Ok(())
}

#[test]
fn output_stream_macro_handles_element_type_and_initial_string() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

    let output = symbol(&mut ctx, &runtime, "WITH-OUTPUT-TO-STRING")?;
    let stream = symbol(&mut ctx, &runtime, "S")?;
    let body = symbol(&mut ctx, &runtime, "BODY")?;
    let element_type = keyword(&mut ctx, &runtime, "ELEMENT-TYPE")?;
    let character = symbol(&mut ctx, &runtime, "CHARACTER")?;
    let text = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b'])?;
    let quoted_character = quoted(&mut ctx, &runtime, character)?;

    let spec = list(
        &mut ctx,
        &runtime,
        &[stream, element_type, quoted_character],
    )?;
    let form = list(&mut ctx, &runtime, &[output, spec, body])?;
    assert!(expand(&mut ctx, &runtime, "WITH-OUTPUT-TO-STRING", form).is_ok());

    let invalid_spec = list(&mut ctx, &runtime, &[stream, element_type, Word::NIL])?;
    let invalid_form = list(&mut ctx, &runtime, &[output, invalid_spec, body])?;
    assert_eq!(
        expand(&mut ctx, &runtime, "WITH-OUTPUT-TO-STRING", invalid_form),
        Err(ObjectError::TypeError)
    );

    let initial_spec = list(&mut ctx, &runtime, &[stream, text, element_type, Word::NIL])?;
    let initial_form = list(&mut ctx, &runtime, &[output, initial_spec, body])?;
    let expansion = expand(&mut ctx, &runtime, "WITH-OUTPUT-TO-STRING", initial_form)?;
    let expansion_parts = elements(&mut ctx, expansion)?;
    let result = elements(&mut ctx, expansion_parts[2])?;
    assert_eq!(result[0], symbol(&mut ctx, &runtime, "PROGN")?);
    let _write_initial = elements(&mut ctx, result[1])?;
    assert!(contains_word(&mut ctx, expansion, text)?);
    Ok(())
}

#[test]
fn output_stream_macro_rejects_unknown_keywords_and_empty_forms() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

    let output = symbol(&mut ctx, &runtime, "WITH-OUTPUT-TO-STRING")?;
    let stream = symbol(&mut ctx, &runtime, "S")?;
    let body = symbol(&mut ctx, &runtime, "BODY")?;
    let unknown = keyword(&mut ctx, &runtime, "UNKNOWN")?;
    let spec = list(&mut ctx, &runtime, &[stream, unknown, Word::TRUE])?;
    let form = list(&mut ctx, &runtime, &[output, spec, body])?;
    assert_eq!(
        expand(&mut ctx, &runtime, "WITH-OUTPUT-TO-STRING", form),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        ctx.take_pending_lisp_error(),
        Some(ncl_object::LispError::ProgramError(
            ncl_object::ProgramError::UnknownKeyword
        ))
    );

    let empty = list(&mut ctx, &runtime, &[output])?;
    assert_eq!(
        expand(&mut ctx, &runtime, "WITH-OUTPUT-TO-STRING", empty),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn input_stream_macro_handles_end_index_and_invalid_keyword_arguments() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

    let input = symbol(&mut ctx, &runtime, "WITH-INPUT-FROM-STRING")?;
    let stream = symbol(&mut ctx, &runtime, "S")?;
    let index = symbol(&mut ctx, &runtime, "POSITION")?;
    let body = symbol(&mut ctx, &runtime, "BODY")?;
    let text = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b', 'c'])?;
    let end = keyword(&mut ctx, &runtime, "END")?;
    let spec = list(&mut ctx, &runtime, &[stream, text, end, Word::fixnum(2)])?;
    let form = list(&mut ctx, &runtime, &[input, spec, body])?;
    let expansion = expand(&mut ctx, &runtime, "WITH-INPUT-FROM-STRING", form)?;
    let parts = elements(&mut ctx, expansion)?;
    let bindings = elements(&mut ctx, parts[1])?;
    let stream_binding = elements(&mut ctx, bindings[0])?;
    let make_stream = elements(&mut ctx, stream_binding[1])?;
    assert_eq!(
        make_stream[0],
        symbol(&mut ctx, &runtime, "MAKE-STRING-INPUT-STREAM")?
    );
    assert_eq!(make_stream[2], Word::fixnum(0));
    assert_eq!(make_stream[3], Word::fixnum(2));

    let index_key = keyword(&mut ctx, &runtime, "INDEX")?;
    let indexed_spec = list(
        &mut ctx,
        &runtime,
        &[stream, text, index_key, index, end, Word::fixnum(2)],
    )?;
    let indexed_form = list(&mut ctx, &runtime, &[input, indexed_spec, body])?;
    let indexed_expansion = expand(&mut ctx, &runtime, "WITH-INPUT-FROM-STRING", indexed_form)?;
    let indexed_parts = elements(&mut ctx, indexed_expansion)?;
    let indexed_bindings = elements(&mut ctx, indexed_parts[1])?;
    assert_eq!(indexed_bindings.len(), 2);
    let indexed_body = elements(&mut ctx, indexed_parts[2])?;
    assert_eq!(indexed_body[0], symbol(&mut ctx, &runtime, "PROG1")?);

    let start = keyword(&mut ctx, &runtime, "START")?;
    let duplicate = list(
        &mut ctx,
        &runtime,
        &[stream, text, start, Word::fixnum(1), start, Word::fixnum(2)],
    )?;
    let duplicate_form = list(&mut ctx, &runtime, &[input, duplicate, body])?;
    assert_eq!(
        expand(&mut ctx, &runtime, "WITH-INPUT-FROM-STRING", duplicate_form),
        Err(ObjectError::TypeError)
    );

    let invalid_index = list(
        &mut ctx,
        &runtime,
        &[stream, text, index_key, Word::fixnum(1)],
    )?;
    let invalid_index_form = list(&mut ctx, &runtime, &[input, invalid_index, body])?;
    assert_eq!(
        expand(
            &mut ctx,
            &runtime,
            "WITH-INPUT-FROM-STRING",
            invalid_index_form
        ),
        Err(ObjectError::TypeError)
    );
    Ok(())
}
