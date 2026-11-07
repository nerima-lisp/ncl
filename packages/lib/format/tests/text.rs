//! Expected FORMAT output for text and character directives.

#![expect(
    clippy::expect_used,
    reason = "Test setup uses expect and unwrap to keep fixture failures local."
)]

use ncl_lib_format::{FormatErrorKind, execute, parse};
use ncl_object::{
    Package, Runtime, ThreadContext, Word, make_cons, make_string, set_symbol_special,
    set_symbol_value,
};
use ncl_printer::StringSink;

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("register context");
    ncl_lib_streams::register(&runtime).expect("register streams");
    (runtime, ctx)
}

fn string(runtime: &Runtime, ctx: &mut ThreadContext, text: &str) -> Word {
    make_string(ctx, runtime, &text.chars().collect::<Vec<char>>()).expect("string")
}

fn run(control: &str, arguments: &[Word], ctx: &mut ThreadContext, runtime: &Runtime) -> String {
    let parsed = parse(control).expect("control");
    let mut sink = StringSink::new();
    execute(&parsed, arguments, ctx, runtime, &mut sink).expect("execute");
    sink.into_string()
}

fn set_print_special(ctx: &mut ThreadContext, runtime: &Runtime, name: &str, value: Word) {
    let package = runtime
        .find_package(ctx, "COMMON-LISP")
        .expect("COMMON-LISP package");
    let (symbol, _) = Package::from_word(package)
        .intern(ctx, runtime, name)
        .expect("print special");
    set_symbol_special(ctx, symbol, true).expect("mark print special");
    set_symbol_value(ctx, symbol, value).expect("set print special");
}

#[test]
fn aesthetic_and_standard_directives_apply_field_padding() {
    let (runtime, mut ctx) = context();
    let value = string(&runtime, &mut ctx, "x");
    assert_eq!(
        run("~5A|~5@A", &[value, value], &mut ctx, &runtime),
        "x    |    x"
    );
    assert_eq!(run("~5,2,1,'-A", &[value], &mut ctx, &runtime), "x----");
}

#[test]
fn character_directive_emits_literal_and_named_characters() {
    let (runtime, mut ctx) = context();
    let input = [
        Word::character(u32::from('a')),
        Word::character(u32::from(' ')),
        Word::character(u32::from('\n')),
    ];
    assert_eq!(
        run("~C/~:C/~@C", &input, &mut ctx, &runtime),
        "a/Space/#\\Newline"
    );
}

#[test]
fn plural_directives_use_and_back_up_the_last_argument() {
    let (runtime, mut ctx) = context();
    assert_eq!(run("~D~P", &[Word::fixnum(2)], &mut ctx, &runtime), "2s");
    assert_eq!(
        run("~D~:P~A", &[Word::fixnum(2)], &mut ctx, &runtime),
        "2s2"
    );
    assert_eq!(run("~@P", &[Word::fixnum(1)], &mut ctx, &runtime), "y");
    assert_eq!(
        run("~D~:@P~A", &[Word::fixnum(3)], &mut ctx, &runtime),
        "3ies3"
    );
}

#[test]
fn line_controls_and_ignored_newline_follow_modifiers() {
    let (runtime, mut ctx) = context();
    assert_eq!(run("a~2%~&b~~", &[], &mut ctx, &runtime), "a\n\nb~");
    assert_eq!(run("a~:\n  b", &[], &mut ctx, &runtime), "a  b");
    assert_eq!(run("a~@\n  b", &[], &mut ctx, &runtime), "a\nb");
}

#[test]
fn wrong_character_arguments_keep_the_directive_location() {
    let (runtime, mut ctx) = context();
    let value = string(&runtime, &mut ctx, "not a character");
    let parsed = parse("pre~C").expect("control");
    let mut sink = StringSink::new();
    let error = execute(&parsed, &[value], &mut ctx, &runtime, &mut sink)
        .expect_err("character type check");
    assert_eq!(error.position, 3);
    assert_eq!(error.directive, Some('C'));
    assert_eq!(error.kind, FormatErrorKind::WrongArgumentType);
}

#[test]
fn write_directive_uses_ambient_printer_specials() {
    let (runtime, mut ctx) = context();
    let value = string(&runtime, &mut ctx, "x");
    set_print_special(&mut ctx, &runtime, "*PRINT-ESCAPE*", Word::NIL);
    set_print_special(&mut ctx, &runtime, "*PRINT-BASE*", Word::fixnum(16));

    assert_eq!(
        run("~W|~W", &[value, Word::fixnum(31)], &mut ctx, &runtime),
        "x|1F"
    );
}

#[test]
fn colon_write_directive_enables_pretty_printing() {
    let (runtime, mut ctx) = context();
    set_print_special(&mut ctx, &runtime, "*PRINT-PRETTY*", Word::NIL);
    set_print_special(&mut ctx, &runtime, "*PRINT-BASE*", Word::fixnum(16));
    let text = "x".repeat(80);
    let text_value = string(&runtime, &mut ctx, &text);
    let list = make_cons(&mut ctx, &runtime, Word::fixnum(31), Word::NIL).expect("tail");
    let list = make_cons(&mut ctx, &runtime, text_value, list).expect("list");

    assert_eq!(
        run("~:W", &[list], &mut ctx, &runtime),
        format!("(\"{text}\"\n 1F)")
    );
}

#[test]
fn at_sign_write_directive_clears_level_and_length_limits() {
    let (runtime, mut ctx) = context();
    set_print_special(&mut ctx, &runtime, "*PRINT-LEVEL*", Word::fixnum(1));
    set_print_special(&mut ctx, &runtime, "*PRINT-LENGTH*", Word::fixnum(1));
    set_print_special(&mut ctx, &runtime, "*PRINT-BASE*", Word::fixnum(16));
    let inner_tail = make_cons(&mut ctx, &runtime, Word::fixnum(32), Word::NIL).expect("tail");
    let inner = make_cons(&mut ctx, &runtime, Word::fixnum(31), inner_tail).expect("inner");
    let outer_tail = make_cons(&mut ctx, &runtime, Word::fixnum(34), Word::NIL).expect("tail");
    let outer_tail = make_cons(&mut ctx, &runtime, Word::fixnum(33), outer_tail).expect("tail");
    let outer = make_cons(&mut ctx, &runtime, inner, outer_tail).expect("outer");

    assert_eq!(
        run("~W|~@W", &[outer, outer], &mut ctx, &runtime),
        "(# ...)|((1F 20) 21 22)"
    );
}
