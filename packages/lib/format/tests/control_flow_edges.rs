//! Error locations across nested FORMAT control flow.
#![expect(
    clippy::expect_used,
    reason = "Test setup uses expect to keep fixture failures local."
)]

use ncl_lib_format::{FormatErrorKind, ParseErrorKind, execute, parse};
use ncl_object::{Runtime, ThreadContext, Word, make_cons, make_string};
use ncl_printer::StringSink;

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("register context");
    (runtime, ctx)
}

fn string(runtime: &Runtime, ctx: &mut ThreadContext, value: &str) -> Word {
    make_string(ctx, runtime, &value.chars().collect::<Vec<_>>()).expect("string")
}

fn list(runtime: &Runtime, ctx: &mut ThreadContext, values: &[Word]) -> Word {
    values.iter().rev().copied().fold(Word::NIL, |tail, value| {
        make_cons(ctx, runtime, value, tail).expect("cons")
    })
}

#[test] // CLHS 22.3.7.2
fn colon_conditional_without_a_selector_reports_its_location() {
    let (runtime, mut ctx) = context();
    let control = parse("~:[true~;false~]").expect("conditional");
    let mut sink = StringSink::new();
    let error = execute(&control, &[], &mut ctx, &runtime, &mut sink)
        .expect_err("colon conditional requires a selector");

    assert_eq!(error.position, 0);
    assert_eq!(error.directive, Some('['));
    assert_eq!(error.kind, FormatErrorKind::MissingArgument);
}

#[test] // CLHS 22.3.7.4/.5, 22.3.4.1
fn empty_sublist_iteration_locates_a_missing_nested_argument() {
    let (runtime, mut ctx) = context();
    let control = parse("~@:{~A~}").expect("sublist iteration");
    let mut sink = StringSink::new();
    let error = execute(&control, &[Word::NIL], &mut ctx, &runtime, &mut sink)
        .expect_err("empty sublist has no argument for aesthetic printing");

    assert_eq!(error.position, 4);
    assert_eq!(error.directive, Some('A'));
    assert_eq!(error.kind, FormatErrorKind::MissingArgument);
}

#[test] // CLHS 22.3.7.6
fn recursive_string_control_preserves_nested_parse_error_location() {
    let (runtime, mut ctx) = context();
    let nested_control = string(&runtime, &mut ctx, "prefix~Q");
    let nested_arguments = list(&runtime, &mut ctx, &[]);
    let control = parse("~?").expect("recursive directive");
    let mut sink = StringSink::new();
    let error = execute(
        &control,
        &[nested_control, nested_arguments],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect_err("nested control has an unknown directive");

    assert_eq!(error.position, 6);
    assert_eq!(error.directive, Some('Q'));
    assert_eq!(
        error.kind,
        FormatErrorKind::Parse(ParseErrorKind::UnknownDirective)
    );
}

#[test] // CLHS 22.3.7.4/.5, 22.3.1.1
fn rest_iteration_preserves_body_argument_errors() {
    let (runtime, mut ctx) = context();
    let control = parse("~@{~C~}").expect("rest iteration");
    let mut sink = StringSink::new();
    let error = execute(&control, &[Word::fixnum(65)], &mut ctx, &runtime, &mut sink)
        .expect_err("character directive requires a character");

    assert_eq!(error.position, 3);
    assert_eq!(error.directive, Some('C'));
    assert_eq!(error.kind, FormatErrorKind::WrongArgumentType);
}

#[test] // CLHS 22.3.7.4/.5
fn sublist_iteration_rejects_an_improper_outer_list() {
    let (runtime, mut ctx) = context();
    let improper =
        make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2)).expect("dotted outer list");
    let control = parse("~:{~A~}").expect("sublist iteration");
    let mut sink = StringSink::new();
    let error = execute(&control, &[improper], &mut ctx, &runtime, &mut sink)
        .expect_err("sublist iteration requires a proper outer list");

    assert_eq!(error.position, 0);
    assert_eq!(error.directive, Some('{'));
    assert_eq!(error.kind, FormatErrorKind::ImproperList);
}

#[test] // CLHS 22.3.7.6, 22.3.4.1
fn at_sign_recursive_control_preserves_nested_argument_errors() {
    let (runtime, mut ctx) = context();
    let nested_control = string(&runtime, &mut ctx, "~A");
    let control = parse("~@?").expect("recursive directive");
    let mut sink = StringSink::new();
    let error = execute(&control, &[nested_control], &mut ctx, &runtime, &mut sink)
        .expect_err("nested aesthetic directive requires a caller argument");

    assert_eq!(error.position, 0);
    assert_eq!(error.directive, Some('A'));
    assert_eq!(error.kind, FormatErrorKind::MissingArgument);
}
