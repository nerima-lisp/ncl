#![allow(clippy::expect_used, clippy::unwrap_used)]

use super::*;
use ncl_object::{Runtime, ThreadContext, Word, make_cons, make_double, make_string};
use ncl_printer::StringSink;

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("context");
    (runtime, ctx)
}

fn string(runtime: &Runtime, ctx: &mut ThreadContext, value: &str) -> Word {
    make_string(ctx, runtime, &value.chars().collect::<Vec<_>>()).expect("string")
}

fn run(control: &str, arguments: &[Word], ctx: &mut ThreadContext, runtime: &Runtime) -> String {
    let mut sink = StringSink::new();
    execute(
        &crate::parse(control).expect("control"),
        arguments,
        ctx,
        runtime,
        &mut sink,
    )
    .unwrap_or_else(|error| panic!("{control}: {error:?}")); // check-added-lines: allow(panic)
    sink.into_string()
}

#[test]
fn covers_control_parameter_edges() {
    let (runtime, mut ctx) = context();
    assert_eq!(run("~3%x", &[], &mut ctx, &runtime), "\n\n\nx");
    assert_eq!(run("a~&b", &[], &mut ctx, &runtime), "a\nb");
    assert_eq!(run("~2I~2_", &[], &mut ctx, &runtime), "    ");
    assert_eq!(run("~3,2T", &[], &mut ctx, &runtime), "  ");
    assert_eq!(
        run(
            "~P/~P",
            &[Word::fixnum(1), Word::fixnum(2)],
            &mut ctx,
            &runtime
        ),
        "/s"
    );
    assert_eq!(
        run(
            "~#%",
            &[Word::fixnum(1), Word::fixnum(2)],
            &mut ctx,
            &runtime
        ),
        "\n\n"
    );
    assert_eq!(
        run(
            "~2*~A",
            &[Word::fixnum(1), Word::fixnum(2), Word::fixnum(3)],
            &mut ctx,
            &runtime
        ),
        "3"
    );
    assert_eq!(
        run(
            "~-2*~A",
            &[Word::fixnum(1), Word::fixnum(2)],
            &mut ctx,
            &runtime
        ),
        "1"
    );
    assert_eq!(run("~^tail", &[], &mut ctx, &runtime), "");
    assert_eq!(
        run("~C", &[Word::character(u32::from('x'))], &mut ctx, &runtime),
        "x"
    );
}

#[test]
fn covers_compound_edges_and_argument_errors() {
    let (runtime, mut ctx) = context();
    let one = string(&runtime, &mut ctx, "one");
    let two = string(&runtime, &mut ctx, "two");
    let tail = make_cons(&mut ctx, &runtime, two, Word::NIL).expect("tail");
    let list = make_cons(&mut ctx, &runtime, one, tail).expect("list");
    assert_eq!(
        run("~[zero~;one~]", &[Word::fixnum(1)], &mut ctx, &runtime),
        "one"
    );
    assert_eq!(
        run("~[zero~;one~]", &[Word::fixnum(9)], &mut ctx, &runtime),
        "one"
    );
    assert_eq!(run("~{~A,~}", &[list], &mut ctx, &runtime), "one,two,");
    assert_eq!(run("~{~A~}", &[Word::NIL], &mut ctx, &runtime), "");
    assert_eq!(run("~:(Hi~)", &[], &mut ctx, &runtime), "hi");
    assert_eq!(run("~(Hi~)", &[], &mut ctx, &runtime), "HI");
    assert_eq!(run("~10<ok~>", &[], &mut ctx, &runtime), "        ok");
    assert_eq!(run("~:>ok", &[], &mut ctx, &runtime), "ok");
    assert_eq!(
        run(
            "~?",
            &[string(&runtime, &mut ctx, "~A"), list],
            &mut ctx,
            &runtime
        ),
        "one"
    );
    assert!(
        execute(
            &crate::parse("~A").unwrap(),
            &[],
            &mut ctx,
            &runtime,
            &mut StringSink::new()
        )
        .is_err()
    );
    assert!(
        execute(
            &crate::parse("~[").unwrap(),
            &[],
            &mut ctx,
            &runtime,
            &mut StringSink::new()
        )
        .is_err()
    );
    assert!(
        execute(
            &crate::parse("~{").unwrap(),
            &[],
            &mut ctx,
            &runtime,
            &mut StringSink::new()
        )
        .is_err()
    );
}

#[test]
fn covers_value_formats_and_invalid_inputs() {
    let (runtime, mut ctx) = context();
    let value = string(&runtime, &mut ctx, "x");
    assert_eq!(
        run("~5,'0A/~5@A", &[value, value], &mut ctx, &runtime),
        "    x/x    "
    );
    assert_eq!(
        run(
            "~D/~B/~O/~X/~16R",
            &[Word::fixnum(255); 5],
            &mut ctx,
            &runtime
        ),
        "255/11111111/377/FF/FF"
    );
    let float = make_double(&mut ctx, &runtime, 12.345)
        .expect("float")
        .into();
    assert_eq!(
        run("~8,2F/~10,2E/~8,1G/~8,3$", &[float; 4], &mut ctx, &runtime),
        "   12.35/    1.23e1/    12.3/  12.345"
    );
    assert!(
        execute(
            &crate::parse("~D").unwrap(),
            &[value],
            &mut ctx,
            &runtime,
            &mut StringSink::new()
        )
        .is_err()
    );
    assert!(
        execute(
            &crate::parse("~R").unwrap(),
            &[value],
            &mut ctx,
            &runtime,
            &mut StringSink::new()
        )
        .is_err()
    );
    assert!(
        execute(
            &crate::parse("~F").unwrap(),
            &[value],
            &mut ctx,
            &runtime,
            &mut StringSink::new()
        )
        .is_err()
    );
    assert!(
        execute(
            &crate::parse("~$").unwrap(),
            &[value],
            &mut ctx,
            &runtime,
            &mut StringSink::new()
        )
        .is_err()
    );
    assert!(
        execute(
            &crate::parse("~C").unwrap(),
            &[value],
            &mut ctx,
            &runtime,
            &mut StringSink::new()
        )
        .is_err()
    );
}

#[test]
fn covers_parameter_and_printer_helpers() {
    let (runtime, mut ctx) = context();
    let value = string(&runtime, &mut ctx, "x");
    assert_eq!(object_string(&ctx, value).as_deref(), Some("x"));
    assert_eq!(object_string(&ctx, Word::fixnum(1)), None);
    assert!(parameter_width(Some(&crate::Parameter::Integer(-1)), DirectiveKind::A).is_err());
    assert!(
        parameters::parameter_usize(Some(&crate::Parameter::Integer(-1)), DirectiveKind::F)
            .is_err()
    );
    assert!(
        parameters::repeat_count(&crate::Directive {
            parameters: vec![crate::Parameter::Character('x')],
            colon: false,
            at_sign: false,
            kind: DirectiveKind::Percent
        })
        .is_err()
    );
    assert!(
        parameters::repeat_count(&crate::Directive {
            parameters: vec![crate::Parameter::Relative],
            colon: false,
            at_sign: false,
            kind: DirectiveKind::Percent
        })
        .is_err()
    );
    assert!(
        parameters::repeat_count_for(
            &crate::Directive {
                parameters: vec![crate::Parameter::ArgumentCount],
                colon: false,
                at_sign: false,
                kind: DirectiveKind::Percent
            },
            &ExecutionState {
                arguments: &[],
                argument_index: &mut 0,
                ctx: &mut ctx,
                runtime: &runtime,
                sink: &mut StringSink::new(),
                line_start: &mut true
            }
        )
        .is_ok()
    );
}
