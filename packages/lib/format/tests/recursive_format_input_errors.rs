//! Public FORMAT argument errors in recursive control processing.
#![expect(
    clippy::expect_used,
    reason = "Test setup uses expect to keep fixture failures local."
)]

use ncl_object::{
    FunctionObject, HeapRootedWord, ObjectError, Runtime, ThreadContext, Word, make_cons,
    make_string,
};

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("register context");
    ncl_lib_format::register(&runtime).expect("register FORMAT");
    (runtime, ctx)
}

fn format_function(runtime: &Runtime, ctx: &mut ThreadContext) -> FunctionObject {
    runtime
        .function(ctx, "COMMON-LISP", "FORMAT")
        .and_then(|word| FunctionObject::try_from(word).ok())
        .expect("FORMAT function")
}

fn string(runtime: &Runtime, ctx: &mut ThreadContext, value: &str) -> Word {
    make_string(ctx, runtime, &value.chars().collect::<Vec<_>>()).expect("string")
}

fn call_format(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    format: FunctionObject,
    destination: Word,
    control: &str,
    arguments: &[Word],
) -> Result<Word, ObjectError> {
    let destination = HeapRootedWord::new(ctx, destination)?;
    let argument_roots = arguments
        .iter()
        .map(|argument| HeapRootedWord::new(ctx, *argument))
        .collect::<Result<Vec<_>, _>>()?;
    let control_word = string(runtime, ctx, control);
    let control = HeapRootedWord::new(ctx, control_word)?;
    let mut call_arguments = Vec::with_capacity(arguments.len() + 2);
    call_arguments.extend([destination.get(), control.get()]);
    call_arguments.extend(argument_roots.iter().map(HeapRootedWord::get));
    runtime.call_builtin(ctx, format, &call_arguments)
}

#[test]
fn public_format_requires_destination_and_control_arguments() {
    let (runtime, mut ctx) = context();
    let format = format_function(&runtime, &mut ctx);

    // CLHS 22.3 defines FORMAT with both destination and control-string arguments.
    assert_eq!(
        runtime.call_builtin(&mut ctx, format, &[]),
        Err(ObjectError::TypeError)
    );
    // CLHS 22.3 requires control-string even when the destination is NIL.
    assert_eq!(
        runtime.call_builtin(&mut ctx, format, &[Word::NIL]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn public_format_rejects_a_non_format_control_for_recursive_question_mark() {
    let (runtime, mut ctx) = context();
    let format = format_function(&runtime, &mut ctx);

    // CLHS 22.3.7.6 requires the next ~? argument to be a format control.
    assert_eq!(
        call_format(
            &runtime,
            &mut ctx,
            format,
            Word::NIL,
            "~?",
            &[Word::fixnum(7), Word::NIL],
        ),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn public_format_requires_a_list_after_recursive_question_mark() {
    let (runtime, mut ctx) = context();
    let format = format_function(&runtime, &mut ctx);
    let nested_control = string(&runtime, &mut ctx, "~A");
    let nested_control = HeapRootedWord::new(&ctx, nested_control).expect("root nested control");
    let not_a_list_word = string(&runtime, &mut ctx, "not a list");
    let not_a_list = HeapRootedWord::new(&ctx, not_a_list_word).expect("root invalid list");

    // CLHS 22.3.7.6 requires a list after the recursive format control.
    assert_eq!(
        call_format(
            &runtime,
            &mut ctx,
            format,
            Word::NIL,
            "~?",
            &[nested_control.get(), not_a_list.get()],
        ),
        Err(ObjectError::TypeError)
    );

    let dotted_list_word = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2))
        .expect("dotted recursive argument list");
    let dotted_list = HeapRootedWord::new(&ctx, dotted_list_word).expect("root dotted list");
    // CLHS 22.3.7.6 requires a list; a dotted pair is not a list of format arguments.
    assert_eq!(
        call_format(
            &runtime,
            &mut ctx,
            format,
            Word::NIL,
            "~?",
            &[nested_control.get(), dotted_list.get()],
        ),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn public_format_rejects_missing_recursive_question_mark_arguments() {
    let (runtime, mut ctx) = context();
    let format = format_function(&runtime, &mut ctx);
    let nested_control_word = string(&runtime, &mut ctx, "~A");
    let nested_control = HeapRootedWord::new(&ctx, nested_control_word).expect("root control");

    // CLHS 22.3.7.6 requires a format control as the next ~? argument.
    assert_eq!(
        call_format(&runtime, &mut ctx, format, Word::NIL, "~?", &[]),
        Err(ObjectError::TypeError)
    );
    // CLHS 22.3.7.6 requires an argument list immediately after the nested control string.
    assert_eq!(
        call_format(
            &runtime,
            &mut ctx,
            format,
            Word::NIL,
            "~?",
            &[nested_control.get()],
        ),
        Err(ObjectError::TypeError)
    );
    // CLHS 22.3.7.6 supplies the nested list to its control; CLHS 22.3.4.1 makes ~A consume an arg.
    assert_eq!(
        call_format(
            &runtime,
            &mut ctx,
            format,
            Word::NIL,
            "~?",
            &[nested_control.get(), Word::NIL],
        ),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn public_format_parses_the_nested_recursive_control_string() {
    let (runtime, mut ctx) = context();
    let format = format_function(&runtime, &mut ctx);
    let malformed_word = string(&runtime, &mut ctx, "prefix~Q");
    let malformed = HeapRootedWord::new(&ctx, malformed_word).expect("root malformed control");

    // CLHS 22.3.7.6 recursively processes the supplied control; unknown directives are invalid.
    assert_eq!(
        call_format(
            &runtime,
            &mut ctx,
            format,
            Word::NIL,
            "~?",
            &[malformed.get(), Word::NIL],
        ),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn public_format_requires_a_string_for_at_sign_recursive_processing() {
    let (runtime, mut ctx) = context();
    let format = format_function(&runtime, &mut ctx);
    let format_word = runtime
        .function(&mut ctx, "COMMON-LISP", "FORMAT")
        .expect("FORMAT function word");
    let format_word = HeapRootedWord::new(&ctx, format_word).expect("root FORMAT function");

    // CLHS 22.3.7.6 requires the direct ~@? argument to be a string, not a function control.
    assert_eq!(
        call_format(
            &runtime,
            &mut ctx,
            format,
            Word::NIL,
            "~@?",
            &[format_word.get()],
        ),
        Err(ObjectError::TypeError)
    );
}
