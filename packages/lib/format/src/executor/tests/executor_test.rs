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
    assert_eq!(run("~1^tail", &[], &mut ctx, &runtime), "");
    assert!(
        execute(
            &crate::parse("~:P").unwrap(),
            &[],
            &mut ctx,
            &runtime,
            &mut StringSink::new(),
        )
        .is_err()
    );
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
        ""
    );
    assert_eq!(run("~{~A,~}", &[list], &mut ctx, &runtime), "one,two,");
    assert_eq!(run("~{~A~}", &[Word::NIL], &mut ctx, &runtime), "");
    assert_eq!(run("~:(Hi~)", &[], &mut ctx, &runtime), "Hi");
    assert_eq!(run("~(Hi~)", &[], &mut ctx, &runtime), "hi");
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
    let inner = make_cons(&mut ctx, &runtime, one, Word::NIL).expect("inner");
    let outer = make_cons(&mut ctx, &runtime, inner, Word::NIL).expect("outer");
    assert_eq!(run("~{~{~A~}~}", &[outer], &mut ctx, &runtime), "one");
    let improper = make_cons(&mut ctx, &runtime, one, Word::fixnum(1)).expect("improper");
    assert!(
        execute(
            &crate::parse("~{~A~}").expect("brace"),
            &[improper],
            &mut ctx,
            &runtime,
            &mut StringSink::new(),
        )
        .is_err()
    );
    assert!(
        execute(
            &crate::parse("~?").expect("nested"),
            &[string(&runtime, &mut ctx, "~A"), improper],
            &mut ctx,
            &runtime,
            &mut StringSink::new(),
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
        "x0000/    x"
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
    assert!(
        super::parameters::parameter_width(Some(&crate::Parameter::Integer(-1)), DirectiveKind::A)
            .is_err()
    );
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
    let empty_arguments = Vec::new();
    assert!(
        parameters::repeat_count_for(
            &crate::Directive {
                parameters: vec![crate::Parameter::ArgumentCount],
                colon: false,
                at_sign: false,
                kind: DirectiveKind::Percent
            },
            &ExecutionState {
                arguments: &empty_arguments,
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

#[test]
fn covers_control_directive_variants() {
    let (runtime, mut ctx) = context();
    assert_eq!(run("~|~2I~2T", &[], &mut ctx, &runtime), "\u{c}   ");
    assert_eq!(
        run(
            "~P/~P/~P",
            &[Word::fixnum(1), Word::fixnum(2), Word::TRUE],
            &mut ctx,
            &runtime
        ),
        "/s/s"
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
    assert!(
        execute(
            &crate::parse("~C").unwrap(),
            &[Word::character(0x0011_0000)],
            &mut ctx,
            &runtime,
            &mut StringSink::new(),
        )
        .is_err()
    );
    assert!(
        execute(
            &crate::parse("~C").unwrap(),
            &[Word::fixnum(1)],
            &mut ctx,
            &runtime,
            &mut StringSink::new(),
        )
        .is_err()
    );
    assert!(crate::parse("~1,0T").is_err());
}

#[test]
fn covers_value_padding_scales_and_direct_parameter_edges() {
    let (runtime, mut ctx) = context();
    let float = make_double(&mut ctx, &runtime, 1.25).expect("float");
    assert_eq!(
        run("~8,2,1E", &[float.into()], &mut ctx, &runtime),
        "  1.25e1"
    );
    assert_eq!(
        run("~8,2,1F", &[float.into()], &mut ctx, &runtime),
        "   12.50"
    );
    assert_eq!(
        run("~8,2,,,'0$", &[float.into()], &mut ctx, &runtime),
        "00001.25"
    );
    assert_eq!(
        run("~8,2@F", &[float.into()], &mut ctx, &runtime),
        "   +1.25"
    );
    assert_eq!(run("~16R", &[Word::fixnum(255)], &mut ctx, &runtime), "FF");
    assert_eq!(
        run(
            "~8A/~8@S",
            &[string(&runtime, &mut ctx, "x"); 2],
            &mut ctx,
            &runtime
        ),
        "x       /     \"x\""
    );
    assert!(
        execute(
            &crate::parse("~A").unwrap(),
            &[Word::fixnum(1)],
            &mut ctx,
            &runtime,
            &mut StringSink::new(),
        )
        .is_ok()
    );
    assert!(
        super::parameters::parameter_width(
            Some(&crate::Parameter::Character('x')),
            DirectiveKind::A
        )
        .is_err()
    );
    assert!(
        parameters::parameter_usize(Some(&crate::Parameter::Relative), DirectiveKind::F).is_err()
    );
    assert_eq!(parameters::parameter_i64(None), None);
    assert_eq!(
        parameters::parameter_i64(Some(&crate::Parameter::Integer(3))),
        Some(3)
    );
    assert_eq!(
        parameters::parameter_i64(Some(&crate::Parameter::Unsupplied)),
        None
    );
}

#[test]
fn covers_internal_dispatch_fallbacks_and_currency_alignment() {
    let (runtime, mut ctx) = context();
    let float = make_double(&mut ctx, &runtime, 1.25).expect("float");
    assert_eq!(run("a~/", &[], &mut ctx, &runtime), "a\n");
    assert_eq!(
        run("~8,2@$", &[float.into()], &mut ctx, &runtime),
        "    1.25"
    );

    let mut sink = StringSink::new();
    let mut argument_index = 0;
    let mut line_start = true;
    let float_word: Word = float.into();
    let float_arguments = vec![float_word];
    let mut state = ExecutionState {
        arguments: &float_arguments,
        argument_index: &mut argument_index,
        ctx: &mut ctx,
        runtime: &runtime,
        sink: &mut sink,
        line_start: &mut line_start,
    };
    let fallback = crate::Directive {
        parameters: Vec::new(),
        colon: false,
        at_sign: false,
        kind: DirectiveKind::A,
    };
    assert!(control::execute_control_kind(&fallback, &mut state).is_err());
    assert!(
        value::execute_value_kind(
            &crate::Directive {
                kind: DirectiveKind::Percent,
                ..fallback
            },
            &mut state
        )
        .is_err()
    );

    let scaled = crate::Directive {
        parameters: vec![
            crate::Parameter::Integer(0),
            crate::Parameter::Integer(2),
            crate::Parameter::Integer(i64::MAX),
        ],
        colon: false,
        at_sign: false,
        kind: DirectiveKind::F,
    };
    *state.argument_index = 0;
    assert!(value::execute_value_kind(&scaled, &mut state).is_err());

    let invalid_radix = crate::Directive {
        parameters: vec![crate::Parameter::Integer(1)],
        colon: false,
        at_sign: false,
        kind: DirectiveKind::R,
    };
    *state.argument_index = 0;
    assert!(value::execute_value_kind(&invalid_radix, &mut state).is_err());

    let padded = crate::Directive {
        parameters: vec![
            crate::Parameter::Integer(8),
            crate::Parameter::Integer(2),
            crate::Parameter::Unsupplied,
            crate::Parameter::Unsupplied,
            crate::Parameter::Integer(1),
        ],
        colon: false,
        at_sign: false,
        kind: DirectiveKind::F,
    };
    *state.argument_index = 0;
    assert!(value::execute_value_kind(&padded, &mut state).is_ok());

    let nested = crate::parse("~{~{~A~}~}").expect("nested control");
    assert_eq!(
        matching(
            &nested.parts,
            0,
            nested.parts.len(),
            DirectiveKind::BraceOpen,
            DirectiveKind::BraceClose,
        ),
        Ok(nested.parts.len() - 1)
    );
}
