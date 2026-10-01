#![allow(clippy::unwrap_used, reason = "tests assert on printer output")]

//! Second-wave tests for builtin routing and less common object renderings.

use ncl_object::{
    Package, Runtime, ThreadContext, Word, make_cons, make_simple_vector, make_string,
};
use ncl_printer::{PrintCase, PrintOptions, StringSink, write};

fn context_with_streams() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_printer::register(&mut ctx, &runtime).unwrap();
    ncl_lib_streams::register(&runtime).unwrap();
    (runtime, ctx)
}

fn intern(runtime: &Runtime, ctx: &mut ThreadContext, package: &str, name: &str) -> Word {
    let package = runtime.find_package(ctx, package).unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> Word {
    let function = runtime.function(ctx, "COMMON-LISP", name).unwrap();
    let function = ncl_object::FunctionObject::try_from(function).unwrap();
    runtime.call_builtin(ctx, function, args).unwrap()
}

fn read_string(ctx: &ThreadContext, string: Word) -> String {
    let length = ncl_object::string_length(ctx, string).unwrap();
    (0..length)
        .map(|index| ncl_object::string_ref(ctx, string, index).unwrap())
        .collect()
}

fn output_stream_value(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    variable: &str,
    value: Word,
    supplied_stream: Word,
) -> String {
    let stream = call(runtime, ctx, "MAKE-STRING-OUTPUT-STREAM", &[]);
    let variable = intern(runtime, ctx, "COMMON-LISP", variable);
    ncl_object::set_symbol_value(ctx, variable, stream).unwrap();
    call(runtime, ctx, "PRINC", &[value, supplied_stream]);
    let result = call(runtime, ctx, "GET-OUTPUT-STREAM-STRING", &[stream]);
    read_string(ctx, result)
}

#[test]
fn print_builtins_route_nil_and_true_to_their_stream_variables() {
    let (runtime, mut ctx) = context_with_streams();
    let value = make_string(&mut ctx, &runtime, &['o', 'k']).unwrap();

    assert_eq!(
        output_stream_value(&runtime, &mut ctx, "*STANDARD-OUTPUT*", value, Word::NIL,),
        "ok"
    );
    assert_eq!(
        output_stream_value(&runtime, &mut ctx, "*TERMINAL-IO*", value, Word::TRUE,),
        "ok"
    );
}

#[test]
fn character_printing_names_controls_and_rejects_invalid_unicode() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let cases = [
        ('\n', "#\\Newline"),
        ('\t', "#\\Tab"),
        ('\r', "#\\Return"),
        ('\u{8}', "#\\Backspace"),
        ('\u{c}', "#\\Page"),
        ('\u{7f}', "#\\Rubout"),
        ('\0', "#\\Null"),
    ];
    for (character, expected) in cases {
        let mut sink = StringSink::new();
        write(
            &mut ctx,
            &runtime,
            Word::character(u32::from(character)),
            &mut sink,
            &PrintOptions::new(),
        )
        .unwrap();
        assert_eq!(sink.into_string(), expected, "character {character:?}");
    }
}

#[test]
fn symbol_rendering_covers_capitalize_and_literal_escaping() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let package = runtime.find_package(&ctx, "COMMON-LISP-USER").unwrap();
    let package = Package::from_word(package);
    for (name, expected) in [
        ("hello-world", "COMMON-LISP-USER:|hello-world|"),
        ("Hello world", "COMMON-LISP-USER:|Hello world|"),
        ("A|B\\C", "COMMON-LISP-USER:|A\\|B\\\\C|"),
        (".", "COMMON-LISP-USER:|.|"),
    ] {
        let symbol = package.intern(&mut ctx, &runtime, name).unwrap().0;
        let mut sink = StringSink::new();
        write(&mut ctx, &runtime, symbol, &mut sink, &PrintOptions::new()).unwrap();
        assert_eq!(sink.into_string(), expected, "symbol {name:?}");
    }
    let symbol = package.intern(&mut ctx, &runtime, "hello world").unwrap().0;
    let mut sink = StringSink::new();
    write(
        &mut ctx,
        &runtime,
        symbol,
        &mut sink,
        &PrintOptions::new().with_case(PrintCase::Capitalize),
    )
    .unwrap();
    assert_eq!(sink.into_string(), "COMMON-LISP-USER:|hello world|");
}

#[test]
fn circle_scan_labels_shared_vectors_but_not_single_occurrences() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let shared = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(7)]).unwrap();
    let tail = make_cons(&mut ctx, &runtime, shared, Word::NIL).unwrap();
    let outer = make_cons(&mut ctx, &runtime, shared, tail).unwrap();
    let mut sink = StringSink::new();
    write(
        &mut ctx,
        &runtime,
        outer,
        &mut sink,
        &PrintOptions::new().with_circle(true),
    )
    .unwrap();
    assert_eq!(sink.into_string(), "(#1=#(7) #1#)");

    let mut sink = StringSink::new();
    write(
        &mut ctx,
        &runtime,
        shared,
        &mut sink,
        &PrintOptions::new().with_circle(true),
    )
    .unwrap();
    assert_eq!(sink.into_string(), "#(7)");
}
