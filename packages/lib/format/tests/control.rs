//! Expected FORMAT output for argument and control directives.

#![expect(
    clippy::expect_used,
    reason = "Test setup uses expect and unwrap to keep fixture failures local."
)]

use ncl_lib_format::{FormatErrorKind, execute, parse};
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

fn run(
    control: &str,
    arguments: &[Word],
    ctx: &mut ThreadContext,
    runtime: &Runtime,
) -> (String, usize) {
    let parsed = parse(control).expect("control");
    let mut sink = StringSink::new();
    let consumed = execute(&parsed, arguments, ctx, runtime, &mut sink).expect("execution");
    (sink.into_string(), consumed)
}

#[test]
fn selects_conditional_clauses_by_index_and_default_clause() {
    let (runtime, mut ctx) = context();
    for (selector, expected) in [(0, "zero"), (1, "one"), (2, "many")] {
        assert_eq!(
            run(
                "~[zero~;one~;many~]",
                &[Word::fixnum(selector)],
                &mut ctx,
                &runtime
            ),
            (expected.to_owned(), 1),
        );
    }
    assert_eq!(
        run(
            "~[zero~;one~;~:;many~]",
            &[Word::fixnum(8)],
            &mut ctx,
            &runtime
        ),
        ("many".to_owned(), 1),
    );
    assert_eq!(
        run("~1[zero~;one~]", &[], &mut ctx, &runtime),
        ("one".to_owned(), 0),
    );
}

#[test]
fn selects_boolean_and_at_sign_conditionals() {
    let (runtime, mut ctx) = context();
    assert_eq!(
        run("~:[false~;true~]", &[Word::NIL], &mut ctx, &runtime),
        ("false".to_owned(), 1),
    );
    assert_eq!(
        run("~:[false~;true~]", &[Word::TRUE], &mut ctx, &runtime),
        ("true".to_owned(), 1),
    );
    let value = string(&runtime, &mut ctx, "kept");
    assert_eq!(
        run("~@[~A~]", &[value], &mut ctx, &runtime),
        ("kept".to_owned(), 1),
    );
    assert_eq!(
        run("~@[unused~]", &[Word::NIL], &mut ctx, &runtime),
        (String::new(), 1),
    );
}

#[test]
fn iterates_over_a_list_and_remaining_arguments() {
    let (runtime, mut ctx) = context();
    let first = string(&runtime, &mut ctx, "a");
    let second = string(&runtime, &mut ctx, "b");
    let third = string(&runtime, &mut ctx, "c");
    let values = list(&runtime, &mut ctx, &[first, second, third]);
    assert_eq!(
        run("~{~A~}", &[values], &mut ctx, &runtime),
        ("abc".to_owned(), 1),
    );
    assert_eq!(
        run("~@{~A~}", &[first, second, third], &mut ctx, &runtime),
        ("abc".to_owned(), 3),
    );
    assert_eq!(
        run("~1@{~A~}", &[first, second], &mut ctx, &runtime),
        ("a".to_owned(), 1),
    );
}

#[test]
fn iterates_over_sublist_arguments_in_both_modes() {
    let (runtime, mut ctx) = context();
    let a = string(&runtime, &mut ctx, "a");
    let b = string(&runtime, &mut ctx, "b");
    let c = string(&runtime, &mut ctx, "c");
    let d = string(&runtime, &mut ctx, "d");
    let left = list(&runtime, &mut ctx, &[a, b]);
    let right = list(&runtime, &mut ctx, &[c, d]);
    let rows = list(&runtime, &mut ctx, &[left, right]);
    assert_eq!(
        run("~:{~A~A~}", &[rows], &mut ctx, &runtime),
        ("abcd".to_owned(), 1),
    );
    assert_eq!(
        run("~:@{(~A)~}", &[left, right], &mut ctx, &runtime),
        ("(a)(c)".to_owned(), 2),
    );
}

#[test]
fn closing_colon_forces_one_iteration() {
    let (runtime, mut ctx) = context();
    assert_eq!(
        run("~{x~:}", &[Word::NIL], &mut ctx, &runtime),
        ("x".to_owned(), 1),
    );
    assert_eq!(run("~@{x~:}", &[], &mut ctx, &runtime), ("x".to_owned(), 0),);
}

#[test]
fn recursive_format_uses_a_list_or_the_current_arguments() {
    let (runtime, mut ctx) = context();
    let control = string(&runtime, &mut ctx, "<~A>");
    let value = string(&runtime, &mut ctx, "item");
    let arguments = list(&runtime, &mut ctx, &[value]);
    assert_eq!(
        run("~?", &[control, arguments], &mut ctx, &runtime),
        ("<item>".to_owned(), 2),
    );
    assert_eq!(
        run("~@?", &[control, value], &mut ctx, &runtime),
        ("<item>".to_owned(), 2),
    );
    let escaping_control = string(&runtime, &mut ctx, "before~^after");
    assert_eq!(
        run("~@?tail", &[escaping_control], &mut ctx, &runtime),
        ("beforetail".to_owned(), 1),
    );
}

#[test]
fn recursive_format_accepts_function_controls_only_in_list_mode() {
    let (runtime, mut ctx) = context();
    ncl_lib_streams::register(&runtime).expect("streams");
    ncl_lib_format::register(&runtime).expect("format");
    let formatter = runtime
        .function(&mut ctx, "COMMON-LISP", "FORMAT")
        .expect("FORMAT function");
    let nested_control = string(&runtime, &mut ctx, "<~A>");
    let value = string(&runtime, &mut ctx, "item");
    let arguments = list(&runtime, &mut ctx, &[nested_control, value]);
    assert_eq!(
        run("~?", &[formatter, arguments], &mut ctx, &runtime),
        ("<item>".to_owned(), 2),
    );

    for (control, arguments) in [
        ("~@?", vec![formatter]),
        ("~?", vec![Word::NIL, Word::NIL]),
        ("~@?", vec![Word::character(u32::from('x'))]),
    ] {
        let parsed = parse(control).expect("control");
        let mut sink = StringSink::new();
        let error = execute(&parsed, &arguments, &mut ctx, &runtime, &mut sink)
            .expect_err("non-string recursive control");
        assert_eq!(error.kind, FormatErrorKind::WrongArgumentType);
    }
}

#[test]
fn case_conversion_applies_to_captured_body_output() {
    let (runtime, mut ctx) = context();
    let value = string(&runtime, &mut ctx, "hELLO wOrLD");
    for (control, expected) in [
        ("~(~A~)", "hello world"),
        ("~:(~A~)", "Hello World"),
        ("~@(~A~)", "Hello world"),
        ("~:@(~A~)", "HELLO WORLD"),
    ] {
        assert_eq!(
            run(control, &[value], &mut ctx, &runtime),
            (expected.to_owned(), 1),
        );
    }
}

#[test]
fn star_moves_the_argument_pointer_in_all_modes() {
    let (runtime, mut ctx) = context();
    let a = string(&runtime, &mut ctx, "a");
    let b = string(&runtime, &mut ctx, "b");
    let c = string(&runtime, &mut ctx, "c");
    assert_eq!(
        run("~*~A", &[a, b], &mut ctx, &runtime),
        ("b".to_owned(), 2),
    );
    assert_eq!(
        run("~2*~A", &[a, b, c], &mut ctx, &runtime),
        ("c".to_owned(), 3),
    );
    assert_eq!(run("~@*~A", &[a], &mut ctx, &runtime), ("a".to_owned(), 1),);
    assert_eq!(
        run("~A~:*~A", &[a], &mut ctx, &runtime),
        ("aa".to_owned(), 1),
    );
}

#[test]
fn up_arrow_escapes_at_top_level_iteration_and_conditional() {
    let (runtime, mut ctx) = context();
    assert_eq!(
        run("start~^end", &[], &mut ctx, &runtime),
        ("start".to_owned(), 0),
    );
    let a = string(&runtime, &mut ctx, "a");
    let values = list(&runtime, &mut ctx, &[a]);
    assert_eq!(
        run("~{~A~^x~}z", &[values], &mut ctx, &runtime),
        ("az".to_owned(), 1),
    );
    let b = string(&runtime, &mut ctx, "b");
    let left = list(&runtime, &mut ctx, &[a]);
    let right = list(&runtime, &mut ctx, &[b]);
    let rows = list(&runtime, &mut ctx, &[left, right]);
    assert_eq!(
        run("~:{~A~^x~}z", &[rows], &mut ctx, &runtime),
        ("abz".to_owned(), 1),
    );
    assert_eq!(
        run(
            "~[before~^after~;other~]tail",
            &[Word::fixnum(0)],
            &mut ctx,
            &runtime
        ),
        ("before".to_owned(), 1),
    );
}

#[test]
fn colon_up_arrow_continues_until_the_last_sublist() {
    let (runtime, mut ctx) = context();
    let a = string(&runtime, &mut ctx, "a");
    let b = string(&runtime, &mut ctx, "b");
    let c = string(&runtime, &mut ctx, "c");
    let d = string(&runtime, &mut ctx, "d");
    let left = list(&runtime, &mut ctx, &[a]);
    let right = list(&runtime, &mut ctx, &[b]);
    let rows = list(&runtime, &mut ctx, &[left, right]);
    let left_with_remaining = list(&runtime, &mut ctx, &[a, b]);
    let right_with_remaining = list(&runtime, &mut ctx, &[c, d]);
    let rows_with_remaining = list(
        &runtime,
        &mut ctx,
        &[left_with_remaining, right_with_remaining],
    );
    assert_eq!(
        run("~:{~A~:^x~}z", &[rows], &mut ctx, &runtime),
        ("axbz".to_owned(), 1),
    );
    assert_eq!(
        run("~:@{~A~:^x~}z", &[left, right], &mut ctx, &runtime),
        ("axbz".to_owned(), 2),
    );
    assert_eq!(
        run("~:{~A~:^x~A~}z", &[rows_with_remaining], &mut ctx, &runtime),
        ("axbcz".to_owned(), 1),
    );
    assert_eq!(
        run(
            "~:{~A~#:^x~A~}z",
            &[rows_with_remaining],
            &mut ctx,
            &runtime
        ),
        ("axbcxdz".to_owned(), 1),
    );
    assert_eq!(
        run(
            "~:@{~A~:^x~A~}z",
            &[left_with_remaining, right_with_remaining],
            &mut ctx,
            &runtime
        ),
        ("axbcz".to_owned(), 2),
    );
}

#[test]
fn rejects_colon_up_arrow_without_colon_iteration() {
    let (runtime, mut ctx) = context();
    let control = parse("~:^ ").expect("control");
    let mut sink = StringSink::new();
    let error = execute(&control, &[], &mut ctx, &runtime, &mut sink).expect_err("invalid use");
    assert_eq!(error.kind, FormatErrorKind::InvalidDirectiveUse);
}

#[test]
fn colon_up_arrow_requires_the_nearest_iteration_to_be_colon_mode() {
    let (runtime, mut ctx) = context();
    let nested = parse("~:{~{~:^~}~}").expect("control");
    let mut sink = StringSink::new();
    let item = string(&runtime, &mut ctx, "item");
    let inner = list(&runtime, &mut ctx, &[item]);
    let row = list(&runtime, &mut ctx, &[inner]);
    let rows = list(&runtime, &mut ctx, &[row]);
    let error = execute(&nested, &[rows], &mut ctx, &runtime, &mut sink)
        .expect_err("ordinary iteration shadows colon iteration");
    assert_eq!(error.kind, FormatErrorKind::InvalidDirectiveUse);
}

#[test]
fn colon_up_arrow_escapes_the_local_logical_block_body() {
    let (runtime, mut ctx) = context();
    let a = string(&runtime, &mut ctx, "a");
    let b = string(&runtime, &mut ctx, "b");
    let left_values = list(&runtime, &mut ctx, &[a]);
    let right_values = list(&runtime, &mut ctx, &[b]);
    let left = list(&runtime, &mut ctx, &[left_values]);
    let right = list(&runtime, &mut ctx, &[right_values]);
    let rows = list(&runtime, &mut ctx, &[left, right]);
    assert_eq!(
        run("~:{~<~A~:^x~:>!~}Q", &[rows], &mut ctx, &runtime),
        ("a!b!Q".to_owned(), 1),
    );
}
