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
