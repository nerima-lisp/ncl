#![allow(missing_docs)]
#![allow(clippy::unwrap_used, reason = "tests assert on builtin behavior")]

use ncl_object::{
    FunctionObject, Runtime, ThreadContext, Word, pop_root, push_root, simple_vector_ref,
    stream_state, string_length, string_ref,
};

fn setup(gc_stress: bool) -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_streams::register(&runtime).unwrap();
    ctx.set_gc_stress(gc_stress);
    ctx.set_strict_forwarding(true);
    (runtime, ctx)
}

fn function(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap()
}

fn string(ctx: &ThreadContext, value: Word) -> String {
    (0..string_length(ctx, value).unwrap())
        .map(|index| string_ref(ctx, value, index).unwrap())
        .collect()
}

fn make_string(ctx: &mut ThreadContext, runtime: &Runtime, value: &str) -> Word {
    ncl_object::make_string(ctx, runtime, &value.chars().collect::<Vec<_>>()).unwrap()
}

#[test]
fn input_stream_honors_bounds_peek_unread_and_read_line() {
    let (runtime, mut ctx) = setup(false);
    let make_input = function(&runtime, &mut ctx, "MAKE-STRING-INPUT-STREAM");
    let read_char = function(&runtime, &mut ctx, "READ-CHAR");
    let peek_char = function(&runtime, &mut ctx, "PEEK-CHAR");
    let unread_char = function(&runtime, &mut ctx, "UNREAD-CHAR");
    let read_line = function(&runtime, &mut ctx, "READ-LINE");
    let source = make_string(&mut ctx, &runtime, "zero\none\ntwo");
    let stream = runtime
        .call_builtin(
            &mut ctx,
            make_input,
            &[source, Word::fixnum(5), Word::fixnum(8)],
        )
        .unwrap();

    assert_eq!(
        runtime.call_builtin(&mut ctx, peek_char, &[stream]),
        Ok(Word::character(u32::from('o')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[stream]),
        Ok(Word::character(u32::from('o')))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            unread_char,
            &[Word::character(u32::from('o')), stream]
        ),
        Ok(Word::character(u32::from('o')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[stream]),
        Ok(Word::character(u32::from('o')))
    );
    let line = runtime
        .call_builtin(&mut ctx, read_line, &[stream])
        .unwrap();
    assert_eq!(string(&ctx, line), "ne");
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[stream]),
        Ok(Word::NIL)
    );
}

#[test]
fn output_stream_round_trips_write_operations_and_resets() {
    let (runtime, mut ctx) = setup(false);
    let make_output = function(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let write_char = function(&runtime, &mut ctx, "WRITE-CHAR");
    let write_string = function(&runtime, &mut ctx, "WRITE-STRING");
    let write_line = function(&runtime, &mut ctx, "WRITE-LINE");
    let get_output = function(&runtime, &mut ctx, "GET-OUTPUT-STREAM-STRING");
    let stream = runtime.call_builtin(&mut ctx, make_output, &[]).unwrap();
    let text = make_string(&mut ctx, &runtime, "bcde");
    let line = make_string(&mut ctx, &runtime, "f");

    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_char,
            &[Word::character(u32::from('a')), stream]
        ),
        Ok(Word::character(u32::from('a')))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_string,
            &[text, stream, Word::fixnum(1), Word::fixnum(3)]
        ),
        Ok(text)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_line, &[line, stream]),
        Ok(line)
    );
    let output = runtime
        .call_builtin(&mut ctx, get_output, &[stream])
        .unwrap();
    assert_eq!(string(&ctx, output), "acdf\n");
    let empty = runtime
        .call_builtin(&mut ctx, get_output, &[stream])
        .unwrap();
    assert_eq!(string(&ctx, empty), "");
}

#[test]
fn output_stream_survives_gc_stress_and_strict_forwarding() {
    let (runtime, mut ctx) = setup(false);
    let mut make_output = function(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM").as_word();
    let make_output_root = push_root(&mut ctx, &mut make_output);
    let mut write_char = function(&runtime, &mut ctx, "WRITE-CHAR").as_word();
    let write_char_root = push_root(&mut ctx, &mut write_char);
    let mut write_string = function(&runtime, &mut ctx, "WRITE-STRING").as_word();
    let write_string_root = push_root(&mut ctx, &mut write_string);
    let mut write_line = function(&runtime, &mut ctx, "WRITE-LINE").as_word();
    let write_line_root = push_root(&mut ctx, &mut write_line);
    let stream = runtime
        .call_builtin(
            &mut ctx,
            FunctionObject::try_from(make_output).unwrap(),
            &[],
        )
        .unwrap();
    let mut stream = stream;
    let stream_root = push_root(&mut ctx, &mut stream);
    ctx.set_gc_stress(true);
    let result = runtime.call_builtin(
        &mut ctx,
        FunctionObject::try_from(write_char).unwrap(),
        &[Word::character(u32::from('g')), stream],
    );
    assert_eq!(result, Ok(Word::character(u32::from('g'))));
    let text = make_string(&mut ctx, &runtime, "hi");
    let written = runtime.call_builtin(
        &mut ctx,
        FunctionObject::try_from(write_string).unwrap(),
        &[text, stream],
    );
    assert_eq!(string(&ctx, written.unwrap()), "hi");
    let line = make_string(&mut ctx, &runtime, "x");
    let written_line = runtime.call_builtin(
        &mut ctx,
        FunctionObject::try_from(write_line).unwrap(),
        &[line, stream],
    );
    assert_eq!(string(&ctx, written_line.unwrap()), "x");
    let state = stream_state(&ctx, ncl_object::Stream::from_word(stream)).unwrap();
    assert_eq!(simple_vector_ref(&ctx, state, 1), Ok(Word::fixnum(5)));
    assert!(pop_root(&mut ctx, stream_root));
    assert!(pop_root(&mut ctx, write_line_root));
    assert!(pop_root(&mut ctx, write_string_root));
    assert!(pop_root(&mut ctx, write_char_root));
    assert!(pop_root(&mut ctx, make_output_root));
}
