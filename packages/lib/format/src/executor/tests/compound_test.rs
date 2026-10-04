#![allow(
    missing_docs,
    clippy::approx_constant,
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::unwrap_used
)]

use ncl_object::{Runtime, ThreadContext, Word, make_cons, make_string};
use ncl_printer::StringSink;

use super::{ExecutionState, execute};
use crate::parse;

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("register context");
    (runtime, ctx)
}

fn string(runtime: &Runtime, ctx: &mut ThreadContext, value: &str) -> Word {
    make_string(ctx, runtime, &value.chars().collect::<Vec<char>>()).expect("string")
}

fn run(control: &str, arguments: &[Word], ctx: &mut ThreadContext, runtime: &Runtime) -> String {
    let mut sink = StringSink::new();
    execute(
        &parse(control).expect("control"),
        arguments,
        ctx,
        runtime,
        &mut sink,
    )
    .expect("execute");
    sink.into_string()
}

#[test]
fn covers_radix_word_forms_and_plural_modifiers() {
    let (runtime, mut ctx) = context();
    assert_eq!(
        run("~R/~:R/~@R/~:@R", &[Word::fixnum(4); 4], &mut ctx, &runtime),
        "four/fourth/IV/IIII"
    );
    assert_eq!(run("~P", &[Word::fixnum(2)], &mut ctx, &runtime), "s");
    assert_eq!(run("~@P", &[Word::fixnum(1)], &mut ctx, &runtime), "y");
    assert_eq!(run("~@P", &[Word::fixnum(2)], &mut ctx, &runtime), "ies");
    assert_eq!(run("~D~:P", &[Word::fixnum(2)], &mut ctx, &runtime), "2s");
    assert_eq!(run("~R", &[Word::fixnum(0)], &mut ctx, &runtime), "zero");
    assert_eq!(
        run("~R", &[Word::fixnum(21)], &mut ctx, &runtime),
        "twenty-one"
    );
    assert_eq!(
        run("~R", &[Word::fixnum(105)], &mut ctx, &runtime),
        "one hundred five"
    );
    assert_eq!(
        run("~R", &[Word::fixnum(100)], &mut ctx, &runtime),
        "one hundred"
    );
    assert_eq!(
        run("~R", &[Word::fixnum(1_000)], &mut ctx, &runtime),
        "one thousand"
    );
    assert_eq!(
        run("~R", &[Word::fixnum(1_000_000)], &mut ctx, &runtime),
        "one million"
    );
    assert_eq!(
        run("~R", &[Word::fixnum(1_001)], &mut ctx, &runtime),
        "one thousand one"
    );
    assert_eq!(
        run("~R", &[Word::fixnum(1_000_000_000)], &mut ctx, &runtime),
        "one billion"
    );
    assert_eq!(
        run(
            "~R",
            &[Word::fixnum(1_000_000_000_000_i64)],
            &mut ctx,
            &runtime
        ),
        "one trillion"
    );
    assert_eq!(
        run("~R", &[Word::fixnum(-1)], &mut ctx, &runtime),
        "minus one"
    );
    assert_eq!(run("~:R", &[Word::fixnum(1)], &mut ctx, &runtime), "first");
    assert_eq!(run("~:R", &[Word::fixnum(2)], &mut ctx, &runtime), "second");
    assert_eq!(run("~:R", &[Word::fixnum(3)], &mut ctx, &runtime), "third");
    assert_eq!(run("~:R", &[Word::fixnum(5)], &mut ctx, &runtime), "fifth");
    assert_eq!(run("~:R", &[Word::fixnum(8)], &mut ctx, &runtime), "eighth");
    assert_eq!(run("~:R", &[Word::fixnum(9)], &mut ctx, &runtime), "ninth");
    assert_eq!(
        run("~:R", &[Word::fixnum(12)], &mut ctx, &runtime),
        "twelfth"
    );
    assert_eq!(
        run("~:R", &[Word::fixnum(20)], &mut ctx, &runtime),
        "twentieth"
    );
    assert_eq!(
        run("~:R", &[Word::fixnum(-1)], &mut ctx, &runtime),
        "minus first"
    );
    assert_eq!(run("~@R", &[Word::fixnum(0)], &mut ctx, &runtime), "N");
    assert_eq!(run("~@R", &[Word::fixnum(-4)], &mut ctx, &runtime), "-IV");
}

#[test]
fn covers_conditional_iteration_indirection_and_case_forms() {
    let (runtime, mut ctx) = context();
    let value = string(&runtime, &mut ctx, "x");
    let one = string(&runtime, &mut ctx, "one");
    let two = string(&runtime, &mut ctx, "two");
    let tail = make_cons(&mut ctx, &runtime, two, Word::NIL).expect("tail");
    let list = make_cons(&mut ctx, &runtime, one, tail).expect("list");
    let lists = make_cons(&mut ctx, &runtime, list, Word::NIL).expect("lists");
    assert_eq!(
        run(
            "~[zero~;one~:;other~]",
            &[Word::fixnum(9)],
            &mut ctx,
            &runtime
        ),
        "other"
    );
    assert_eq!(
        run("~:[false~;true~]", &[Word::NIL], &mut ctx, &runtime),
        "false"
    );
    assert_eq!(
        run("~:[false~;true~]", &[Word::TRUE], &mut ctx, &runtime),
        "true"
    );
    assert_eq!(run("~@{~A,~}", &[one, two], &mut ctx, &runtime), "one,two,");
    assert_eq!(run("~{~A~^,~}", &[list], &mut ctx, &runtime), "one,two");
    assert_eq!(run("~@{~A~:^,~}", &[one, two], &mut ctx, &runtime), "one,two");
    assert_eq!(run("~:{~A,~}", &[lists], &mut ctx, &runtime), "one,");
    assert_eq!(
        run(
            "~@?",
            &[string(&runtime, &mut ctx, "~A"), value],
            &mut ctx,
            &runtime
        ),
        "x"
    );
    assert_eq!(
        run("~(Hello WORLD~)", &[], &mut ctx, &runtime),
        "hello world"
    );
    assert_eq!(
        run("~:(hello WORLD~)", &[], &mut ctx, &runtime),
        "Hello World"
    );
    assert_eq!(
        run("~@(hello WORLD~)", &[], &mut ctx, &runtime),
        "Hello world"
    );
    assert_eq!(
        run("~:@(hello world~)", &[], &mut ctx, &runtime),
        "HELLO WORLD"
    );
}

#[test]
fn covers_conditional_and_iteration_edge_paths() {
    let (runtime, mut ctx) = context();
    assert_eq!(run("~1[zero~;one~]", &[], &mut ctx, &runtime), "one");
    assert_eq!(
        run("~:[false~;true~]", &[Word::TRUE], &mut ctx, &runtime),
        "true"
    );
    assert_eq!(run("~@[yes~]", &[Word::TRUE], &mut ctx, &runtime), "yes");
    assert_eq!(run("~@[yes~]", &[Word::NIL], &mut ctx, &runtime), "");
    let a = string(&runtime, &mut ctx, "a");
    let b = string(&runtime, &mut ctx, "b");
    let tail = make_cons(&mut ctx, &runtime, b, Word::NIL).expect("tail");
    let items = make_cons(&mut ctx, &runtime, a, tail).expect("items");
    assert_eq!(run("~1{~A~}", &[items], &mut ctx, &runtime), "a");
    assert_eq!(
        run(
            "~@{~A~}",
            &[Word::fixnum(1), Word::fixnum(2)],
            &mut ctx,
            &runtime
        ),
        "12"
    );
    assert_eq!(
        run("~@{literal~}", &[Word::fixnum(1)], &mut ctx, &runtime),
        "literal"
    );
    assert_eq!(
        run("before~1^after", &[Word::fixnum(1)], &mut ctx, &runtime),
        "beforeafter"
    );
    assert_eq!(
        run(
            "before~1^after~A",
            &[Word::fixnum(1), Word::fixnum(2)],
            &mut ctx,
            &runtime
        ),
        "beforeafter1"
    );

    let bad_item =
        make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2)).expect("bad item");
    let bad_outer = make_cons(&mut ctx, &runtime, bad_item, Word::NIL).expect("bad outer");
    let mut sink = StringSink::new();
    assert!(
        execute(
            &parse("~:{~A~}").expect("control"),
            &[bad_outer],
            &mut ctx,
            &runtime,
            &mut sink
        )
        .is_err()
    );
    assert!(
        execute(
            &parse("~@{~}").expect("control"),
            &[Word::fixnum(1)],
            &mut ctx,
            &runtime,
            &mut sink
        )
        .is_ok()
    );
    assert!(
        execute(
            &parse("~?").expect("control"),
            &[Word::NIL, Word::NIL],
            &mut ctx,
            &runtime,
            &mut sink
        )
        .is_err()
    );
    let improper =
        make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2)).expect("improper");
    assert!(
        execute(
            &parse("~?").expect("control"),
            &[string(&runtime, &mut ctx, "~A"), improper],
            &mut ctx,
            &runtime,
            &mut sink
        )
        .is_err()
    );
    let nested_tail =
        make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2)).expect("nested tail");
    let nested_outer = make_cons(&mut ctx, &runtime, nested_tail, Word::NIL).expect("nested outer");
    assert!(
        execute(
            &parse("~:{~A~}").expect("control"),
            &[nested_outer],
            &mut ctx,
            &runtime,
            &mut sink
        )
        .is_err()
    );
    let brace_improper =
        make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2)).expect("brace improper");
    assert!(
        execute(
            &parse("~{~A~}").expect("control"),
            &[brace_improper],
            &mut ctx,
            &runtime,
            &mut sink
        )
        .is_err()
    );
}

#[test]
fn covers_justification_and_parameterized_float_edges() {
    let (runtime, mut ctx) = context();
    assert_eq!(run("~10<foo~;bar~>", &[], &mut ctx, &runtime), "foo    bar");
    assert_eq!(run("~10@<foo~>", &[], &mut ctx, &runtime), "foo       ");
    assert_eq!(
        run("~10:<foo~;bar~>", &[], &mut ctx, &runtime),
        "foo    bar"
    );
    assert_eq!(
        run("~10@<foo~;bar~>", &[], &mut ctx, &runtime),
        "foo    bar"
    );
    assert_eq!(run("~5,4<foo~;bar~>", &[], &mut ctx, &runtime), "foo  bar");
    assert_eq!(
        run("~10,1,1,'*<foo~;bar~>", &[], &mut ctx, &runtime),
        "foo****bar"
    );
    assert_eq!(
        run(
            "~10<~[foo~;bar~]~;x~>",
            &[Word::fixnum(1)],
            &mut ctx,
            &runtime
        ),
        "bar      x"
    );
    let float = ncl_object::make_double(&mut ctx, &runtime, 3.14159).expect("float");
    assert_eq!(
        run("~9,2,1,,'*F", &[float.into()], &mut ctx, &runtime),
        "****31.42"
    );
    assert_eq!(
        run("~9,2,,1,,'*E", &[float.into()], &mut ctx, &runtime),
        "***3.14e1"
    );
    assert_eq!(run("~F", &[float.into()], &mut ctx, &runtime), "3.14159");
    assert_eq!(
        run(
            "~8,1A",
            &[string(&runtime, &mut ctx, "x")],
            &mut ctx,
            &runtime
        ),
        "x       "
    );
    assert_eq!(run("x~/", &[], &mut ctx, &runtime), "x\n");
}

#[test]
fn covers_internal_state_fallbacks() {
    let (runtime, mut ctx) = context();
    let mut index = 0;
    let mut line_start = true;
    let mut sink = StringSink::new();
    let empty_arguments = Vec::new();
    let mut escape = None;
    let mut state = ExecutionState {
        arguments: &empty_arguments,
        argument_index: &mut index,
        ctx: &mut ctx,
        runtime: &runtime,
        sink: &mut sink,
        line_start: &mut line_start,
        column: 0,
        escape: &mut escape,
        remaining_override: None,
    };
    super::execute_parts(&parse("literal").expect("control").parts, 0, 1, &mut state)
        .expect("execute");
    assert_eq!(sink.into_string(), "literal");
    let mut words = String::new();
    super::value::append_words(&mut words, "one");
    super::value::append_words(&mut words, "two");
    assert_eq!(words, "one two");
    let nested = parse("~<~[a~;b~]~;c~>").expect("nested control");
    assert_eq!(
        super::compound::split_justification(&nested.parts, 1, nested.parts.len() - 1).len(),
        2
    );
}
