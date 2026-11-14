#![expect(
    clippy::expect_used,
    reason = "tests assert on registered FORMAT builtin results from valid fixtures"
)]
//! Tests for registered FORMAT builtins and their runtime behavior.

use ncl_conditions::{ConditionIdentifier, condition_class_of};
use ncl_lib_format::register;
use ncl_object::{
    FunctionObject, HeapRootedWord, Instance, ObjectError, Package, Runtime, ThreadContext, Word,
    make_cons, make_string, make_symbol, set_symbol_value, slot_ref, string_ref, symbol_value,
};
use ncl_printer::{PrintOptions, StringSink, write};

fn string(runtime: &Runtime, ctx: &mut ThreadContext, text: &str) -> Word {
    make_string(ctx, runtime, &text.chars().collect::<Vec<_>>()).expect("string")
}

fn read_string(ctx: &ThreadContext, value: Word) -> String {
    let length = ncl_object::string_length(ctx, value).expect("length");
    (0..length)
        .map(|index| string_ref(ctx, value, index).expect("character"))
        .collect()
}

fn assert_pending_simple_error(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    format_control: Word,
    expected_arguments: &str,
) {
    let condition = ctx
        .take_pending_condition()
        .expect("FORMAT failure signals a condition");
    let condition = HeapRootedWord::new(ctx, condition).expect("root condition");
    let actual_class = condition_class_of(ctx, condition.get()).expect("condition class");
    let expected_class = ConditionIdentifier::SimpleError
        .class(ctx, runtime)
        .expect("SIMPLE-ERROR class");
    assert_eq!(actual_class.as_word(), expected_class.as_word());
    assert_eq!(
        slot_ref(ctx, Instance::from_word(condition.get()), 0).expect("format-control slot"),
        format_control,
    );

    let arguments =
        slot_ref(ctx, Instance::from_word(condition.get()), 1).expect("format-arguments slot");
    let mut sink = StringSink::new();
    write(ctx, runtime, arguments, &mut sink, &PrintOptions::default())
        .expect("print format arguments");
    assert_eq!(sink.into_string(), expected_arguments);
}

fn assert_pending_stream_error(ctx: &mut ThreadContext, runtime: &Runtime, stream: Word) {
    let condition = ctx
        .take_pending_condition()
        .expect("stream failure signals a condition");
    let condition = HeapRootedWord::new(ctx, condition).expect("root condition");
    let actual_class = condition_class_of(ctx, condition.get()).expect("condition class");
    let expected_class = ConditionIdentifier::StreamError
        .class(ctx, runtime)
        .expect("STREAM-ERROR class");
    assert_eq!(actual_class.as_word(), expected_class.as_word());
    assert_eq!(
        slot_ref(ctx, Instance::from_word(condition.get()), 0).expect("stream slot"),
        stream,
    );
}

fn setup() -> (Runtime, ThreadContext, FunctionObject) {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("context");
    ncl_conditions::register(&runtime).expect("conditions");
    ncl_lib_streams::register(&runtime).expect("streams");
    register(&runtime).expect("format");
    let function = runtime
        .function(&mut ctx, "COMMON-LISP", "FORMAT")
        .expect("FORMAT");
    let function = FunctionObject::try_from(function).expect("function");
    (runtime, ctx, function)
}

#[test] // CLHS 22.3, 22.3.4.1/.2, 22.3.1.2/.3
fn format_is_callable_and_executes_value_and_line_directives() {
    let (runtime, mut ctx, function) = setup();
    let control = string(&runtime, &mut ctx, "~A/~S/~D~%~&");
    let first = string(&runtime, &mut ctx, "x");
    let second = string(&runtime, &mut ctx, "y");
    let result = runtime
        .call_builtin(
            &mut ctx,
            function,
            &[Word::NIL, control, first, second, Word::fixnum(12)],
        )
        .expect("format result");

    assert_eq!(read_string(&ctx, result), "x/\"y\"/12\n");
}

#[test] // CLHS 22.3.7.6
fn format_preserves_undefined_function_from_recursive_function_control() {
    let (runtime, mut ctx, format) = setup();
    let control = string(&runtime, &mut ctx, "~?");
    let control = HeapRootedWord::new(&ctx, control).expect("root control");
    let common_lisp = runtime
        .find_package(&ctx, "COMMON-LISP")
        .expect("COMMON-LISP package");
    let (missing_formatter, _) = Package::from_word(common_lisp)
        .intern(&mut ctx, &runtime, "MISSING-FORMATTER")
        .expect("formatter symbol");
    let missing_formatter =
        HeapRootedWord::new(&ctx, missing_formatter).expect("root formatter symbol");

    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            format,
            &[Word::NIL, control.get(), missing_formatter.get(), Word::NIL],
        ),
        Err(ObjectError::UndefinedFunction)
    );
}

#[test] // CLHS 22.3
fn format_accepts_symbol_and_character_controls() {
    let (runtime, mut ctx, function) = setup();
    let symbol_name = string(&runtime, &mut ctx, "~A");
    let symbol = make_symbol(&mut ctx, &runtime, symbol_name).expect("symbol");
    let argument = string(&runtime, &mut ctx, "value");

    let symbol_result = runtime
        .call_builtin(&mut ctx, function, &[Word::NIL, symbol, argument])
        .expect("symbol control");
    assert_eq!(read_string(&ctx, symbol_result), "value");

    let character_result = runtime
        .call_builtin(
            &mut ctx,
            function,
            &[Word::NIL, Word::character(u32::from('x'))],
        )
        .expect("character control");
    assert_eq!(read_string(&ctx, character_result), "x");
}

#[test] // CLHS 22.3.7.4/.5
fn format_runs_nested_iteration_over_a_list() {
    let (runtime, mut ctx, function) = setup();
    let control = string(&runtime, &mut ctx, "~{~A~}");
    let list = Word::NIL;
    let rooted_list = ncl_object::HeapRootedWord::new(&ctx, list).expect("root list");
    for value in [3, 2, 1] {
        let cons =
            make_cons(&mut ctx, &runtime, Word::fixnum(value), rooted_list.get()).expect("cons");
        rooted_list.set(cons);
    }

    let result = runtime
        .call_builtin(&mut ctx, function, &[Word::NIL, control, rooted_list.get()])
        .expect("format iteration");
    assert_eq!(read_string(&ctx, result), "123");
}

#[test] // CLHS 22.3.7.4/.5
fn format_maps_improper_iteration_arguments_to_type_error() {
    let (runtime, mut ctx, function) = setup();
    let control = string(&runtime, &mut ctx, "~{~A~}");
    let control = HeapRootedWord::new(&ctx, control).expect("root control");
    let dotted = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2))
        .expect("dotted iteration arguments");
    let dotted = HeapRootedWord::new(&ctx, dotted).expect("root dotted arguments");

    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            function,
            &[Word::NIL, control.get(), dotted.get()]
        ),
        Err(ObjectError::TypeError)
    );
    assert_pending_simple_error(&mut ctx, &runtime, control.get(), "((1 . 2))");
}

#[test] // CLHS 22.3
fn format_writes_to_a_stream_and_the_bound_standard_output() {
    let (runtime, mut ctx, format) = setup();
    let make_output = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "MAKE-STRING-OUTPUT-STREAM")
            .expect("make output stream"),
    )
    .expect("make output function");
    let get_output = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "GET-OUTPUT-STREAM-STRING")
            .expect("get output string"),
    )
    .expect("get output function");
    let stream = runtime
        .call_builtin(&mut ctx, make_output, &[])
        .expect("output stream");
    let stream = HeapRootedWord::new(&ctx, stream).expect("root output stream");
    let control = string(&runtime, &mut ctx, "~A");
    let control = HeapRootedWord::new(&ctx, control).expect("root control");
    let value = string(&runtime, &mut ctx, "stream output");
    let value = HeapRootedWord::new(&ctx, value).expect("root value");

    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            format,
            &[stream.get(), control.get(), value.get()]
        ),
        Ok(Word::NIL)
    );
    let output = runtime
        .call_builtin(&mut ctx, get_output, &[stream.get()])
        .expect("stream contents");
    assert_eq!(read_string(&ctx, output), "stream output");

    let package = runtime
        .find_package(&ctx, "COMMON-LISP")
        .expect("COMMON-LISP package");
    let (output_symbol, _) = Package::from_word(package)
        .intern(&mut ctx, &runtime, "*STANDARD-OUTPUT*")
        .expect("standard output symbol");
    let previous = symbol_value(&ctx, output_symbol).expect("previous standard output");
    let previous = HeapRootedWord::new(&ctx, previous).expect("root previous output");
    set_symbol_value(&mut ctx, output_symbol, stream.get()).expect("bind output stream");

    let control = string(&runtime, &mut ctx, "~A");
    let control = HeapRootedWord::new(&ctx, control).expect("root control");
    let value = string(&runtime, &mut ctx, "standard output");
    let value = HeapRootedWord::new(&ctx, value).expect("root value");
    assert_eq!(
        runtime.call_builtin(&mut ctx, format, &[Word::TRUE, control.get(), value.get()]),
        Ok(Word::NIL)
    );
    let output = runtime
        .call_builtin(&mut ctx, get_output, &[stream.get()])
        .expect("standard output contents");
    assert_eq!(read_string(&ctx, output), "standard output");
    set_symbol_value(&mut ctx, output_symbol, previous.get()).expect("restore standard output");
}

#[test] // CLHS 22.3
fn format_maps_invalid_controls_arguments_destinations_and_stream_writes() {
    let (runtime, mut ctx, format) = setup();
    let valid_control = string(&runtime, &mut ctx, "~A");
    let valid_control = HeapRootedWord::new(&ctx, valid_control).expect("root valid control");
    let non_string = Word::fixnum(1);
    assert_eq!(
        runtime.call_builtin(&mut ctx, format, &[non_string, valid_control.get()]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, format, &[Word::NIL, non_string]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, format, &[Word::fixnum(1), valid_control.get()]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, format, &[Word::NIL, Word::character(0x11_0000)]),
        Err(ObjectError::TypeError)
    );

    let malformed = string(&runtime, &mut ctx, "~z");
    let malformed = HeapRootedWord::new(&ctx, malformed).expect("root malformed control");
    assert_eq!(
        runtime.call_builtin(&mut ctx, format, &[Word::NIL, malformed.get()]),
        Err(ObjectError::TypeError)
    );
    assert_pending_simple_error(&mut ctx, &runtime, malformed.get(), "NIL");

    let make_output = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "MAKE-STRING-OUTPUT-STREAM")
            .expect("make output stream"),
    )
    .expect("make output function");
    let stream = runtime
        .call_builtin(&mut ctx, make_output, &[])
        .expect("output stream");
    let stream = HeapRootedWord::new(&ctx, stream).expect("root output stream");
    assert_eq!(
        runtime.call_builtin(&mut ctx, format, &[stream.get(), malformed.get()]),
        Err(ObjectError::TypeError)
    );
    assert_pending_simple_error(&mut ctx, &runtime, malformed.get(), "NIL");

    let common_lisp = runtime
        .find_package(&ctx, "COMMON-LISP")
        .expect("COMMON-LISP package");
    let (output_symbol, _) = Package::from_word(common_lisp)
        .intern(&mut ctx, &runtime, "*STANDARD-OUTPUT*")
        .expect("standard output symbol");
    let previous_output = symbol_value(&ctx, output_symbol).expect("previous standard output");
    let previous_output = HeapRootedWord::new(&ctx, previous_output).expect("root previous output");
    set_symbol_value(&mut ctx, output_symbol, stream.get()).expect("bind output stream");
    assert_eq!(
        runtime.call_builtin(&mut ctx, format, &[Word::TRUE, malformed.get()]),
        Err(ObjectError::TypeError)
    );
    assert_pending_simple_error(&mut ctx, &runtime, malformed.get(), "NIL");
    set_symbol_value(&mut ctx, output_symbol, previous_output.get())
        .expect("restore standard output");

    let missing_argument = string(&runtime, &mut ctx, "~A");
    assert_eq!(
        runtime.call_builtin(&mut ctx, format, &[Word::NIL, missing_argument]),
        Err(ObjectError::TypeError)
    );
    assert_pending_simple_error(&mut ctx, &runtime, missing_argument, "NIL");
    let wrong_type = string(&runtime, &mut ctx, "~C");
    let wrong_type = HeapRootedWord::new(&ctx, wrong_type).expect("root control");
    let text = string(&runtime, &mut ctx, "not a character");
    let text = HeapRootedWord::new(&ctx, text).expect("root text");
    assert_eq!(
        runtime.call_builtin(&mut ctx, format, &[Word::NIL, wrong_type.get(), text.get()]),
        Err(ObjectError::TypeError)
    );
    assert_pending_simple_error(
        &mut ctx,
        &runtime,
        wrong_type.get(),
        "(\"not a character\")",
    );

    let make_input = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "MAKE-STRING-INPUT-STREAM")
            .expect("make input stream"),
    )
    .expect("make input function");
    let input_text = string(&runtime, &mut ctx, "input only");
    let input_text = HeapRootedWord::new(&ctx, input_text).expect("root input text");
    let input = runtime
        .call_builtin(&mut ctx, make_input, &[input_text.get()])
        .expect("input stream");
    let input = HeapRootedWord::new(&ctx, input).expect("root input stream");
    let control = string(&runtime, &mut ctx, "text");
    let control = HeapRootedWord::new(&ctx, control).expect("root argument");
    let output = string(&runtime, &mut ctx, "~A");
    let output = HeapRootedWord::new(&ctx, output).expect("root control");
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            format,
            &[input.get(), output.get(), control.get()]
        ),
        Err(ObjectError::TypeError)
    );

    let package = runtime
        .find_package(&ctx, "COMMON-LISP")
        .expect("COMMON-LISP package");
    let (output_symbol, _) = Package::from_word(package)
        .intern(&mut ctx, &runtime, "*STANDARD-OUTPUT*")
        .expect("standard output symbol");
    let previous = symbol_value(&ctx, output_symbol).expect("previous standard output");
    let previous = HeapRootedWord::new(&ctx, previous).expect("root previous output");
    set_symbol_value(&mut ctx, output_symbol, Word::fixnum(1)).expect("set invalid output");
    let output_control = string(&runtime, &mut ctx, "~A");
    let output_control = HeapRootedWord::new(&ctx, output_control).expect("root output control");
    let value = string(&runtime, &mut ctx, "output");
    let value = HeapRootedWord::new(&ctx, value).expect("root output value");
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            format,
            &[Word::TRUE, output_control.get(), value.get()]
        ),
        Err(ObjectError::TypeError)
    );
    set_symbol_value(&mut ctx, output_symbol, previous.get()).expect("restore standard output");
}

#[test] // CLHS 22.3
fn format_propagates_errors_while_formatting_to_a_stream() {
    let (runtime, mut ctx, format) = setup();
    let make_output = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "MAKE-STRING-OUTPUT-STREAM")
            .expect("make output stream"),
    )
    .expect("make output function");
    let close = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "CLOSE")
            .expect("close stream"),
    )
    .expect("close function");
    let stream = runtime
        .call_builtin(&mut ctx, make_output, &[])
        .expect("output stream");
    let stream = HeapRootedWord::new(&ctx, stream).expect("root output stream");
    let control = string(&runtime, &mut ctx, "~A");
    let control = HeapRootedWord::new(&ctx, control).expect("root control");
    assert_eq!(
        runtime.call_builtin(&mut ctx, format, &[stream.get(), control.get()]),
        Err(ObjectError::TypeError)
    );
    assert_pending_simple_error(&mut ctx, &runtime, control.get(), "NIL");

    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[stream.get()]),
        Ok(Word::TRUE)
    );
    let value = string(&runtime, &mut ctx, "closed");
    let value = HeapRootedWord::new(&ctx, value).expect("root value");
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            format,
            &[stream.get(), control.get(), value.get()]
        ),
        Err(ObjectError::TypeError)
    );
    assert_pending_stream_error(&mut ctx, &runtime, stream.get());
}
