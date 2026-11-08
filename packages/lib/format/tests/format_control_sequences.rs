//! Public FORMAT control-sequence behavior.
#![expect(
    clippy::expect_used,
    reason = "Test setup uses expect to keep fixture failures local."
)]

use ncl_object::{
    FunctionObject, HeapRootedWord, Runtime, ThreadContext, Word, make_string, string_length,
    string_ref,
};

fn context() -> (Runtime, ThreadContext, FunctionObject) {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("register context");
    ncl_lib_format::register(&runtime).expect("register FORMAT");
    let format = runtime
        .function(&mut ctx, "COMMON-LISP", "FORMAT")
        .and_then(|word| FunctionObject::try_from(word).ok())
        .expect("FORMAT function");
    (runtime, ctx, format)
}

fn format_nil(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    format: FunctionObject,
    control: &str,
    arguments: &[Word],
) -> String {
    let argument_roots = arguments
        .iter()
        .map(|argument| HeapRootedWord::new(ctx, *argument))
        .collect::<Result<Vec<_>, _>>()
        .expect("root arguments");
    let control_word =
        make_string(ctx, runtime, &control.chars().collect::<Vec<_>>()).expect("control string");
    let control = HeapRootedWord::new(ctx, control_word).expect("root control string");
    let mut call_arguments = Vec::with_capacity(arguments.len() + 2);
    call_arguments.extend([Word::NIL, control.get()]);
    call_arguments.extend(argument_roots.iter().map(HeapRootedWord::get));
    let result = runtime
        .call_builtin(ctx, format, &call_arguments)
        .expect("FORMAT result");
    (0..string_length(ctx, result).expect("result length"))
        .map(|index| string_ref(ctx, result, index).expect("result character"))
        .collect()
}

#[test]
fn public_format_uses_remaining_arguments_in_up_arrow_condition() {
    let (runtime, mut ctx, format) = context();

    // CLHS 22.3 and 22.3.9.2: # is zero here, so ~^ terminates before the literal text.
    assert_eq!(format_nil(&runtime, &mut ctx, format, "~#^tail", &[]), "");
    // CLHS 22.3 and 22.3.9.2: # is nonzero, so ~^ continues and emits the literal text.
    assert_eq!(
        format_nil(&runtime, &mut ctx, format, "~#^tail", &[Word::fixnum(7)]),
        "tail"
    );
}
