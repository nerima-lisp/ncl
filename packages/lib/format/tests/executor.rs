#![allow(missing_docs, clippy::expect_used, clippy::unwrap_used)]

use ncl_lib_format::{FormatError, execute, parse};
use ncl_object::{Runtime, ThreadContext, Word, make_cons, make_double, make_string};
use ncl_printer::{PrintError, StringSink};

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("register context");
    (runtime, ctx)
}

fn string(runtime: &Runtime, ctx: &mut ThreadContext, text: &str) -> Word {
    make_string(ctx, runtime, &text.chars().collect::<Vec<char>>()).expect("string")
}

#[test]
fn executes_literals_and_basic_value_directives() {
    let (runtime, mut ctx) = context();
    let control = parse("x=~A s=~S").expect("control");
    let value = string(&runtime, &mut ctx, "hello");
    let mut sink = StringSink::new();
    assert_eq!(
        execute(&control, &[value, value], &mut ctx, &runtime, &mut sink),
        Ok(2)
    );
    assert_eq!(sink.into_string(), "x=hello s=\"hello\"");
}

#[test]
fn executes_integer_radices_and_line_controls() {
    let (runtime, mut ctx) = context();
    let control = parse("~D ~B ~O ~X~2%tail~&done~~").expect("control");
    let mut sink = StringSink::new();
    assert_eq!(
        execute(
            &control,
            &[Word::fixnum(255); 4],
            &mut ctx,
            &runtime,
            &mut sink,
        ),
        Ok(4)
    );
    assert_eq!(sink.into_string(), "255 11111111 377 FF\n\ntail\ndone~");
}

#[test]
fn executes_zero_and_multiple_tilde_repeats_with_expected_line_state() {
    let (runtime, mut ctx) = context();
    let control = parse("head~3~~0~~&tail").expect("control");
    let mut sink = StringSink::new();

    assert_eq!(execute(&control, &[], &mut ctx, &runtime, &mut sink), Ok(0));
    assert_eq!(sink.into_string(), "head~~~\ntail");
}

#[test]
fn rejects_missing_and_non_integer_arguments() {
    let (runtime, mut ctx) = context();
    let mut sink = StringSink::new();
    let missing = parse("~A").expect("control");
    assert_eq!(
        execute(&missing, &[], &mut ctx, &runtime, &mut sink),
        Err(FormatError::MissingArgument {
            directive: ncl_lib_format::DirectiveKind::A,
        })
    );
    let non_integer = parse("~D").expect("control");
    let value = string(&runtime, &mut ctx, "not an integer");
    assert_eq!(
        execute(&non_integer, &[value], &mut ctx, &runtime, &mut sink),
        Err(FormatError::NonInteger {
            directive: ncl_lib_format::DirectiveKind::D,
        })
    );
}

#[test]
fn ampersand_does_not_add_a_second_newline_at_line_start() {
    let (runtime, mut ctx) = context();
    let control = parse("a~%~&b").expect("control");
    let mut sink = StringSink::new();
    execute(&control, &[], &mut ctx, &runtime, &mut sink).expect("execute");
    assert_eq!(sink.into_string(), "a\nb");
}

#[test]
fn rejects_invalid_repeat_parameters_and_preserves_consumed_argument_count() {
    let (runtime, mut ctx) = context();
    let mut sink = StringSink::new();
    assert!(parse("~-1%").is_err());
    assert_eq!(
        execute(
            &parse("~0%~A").expect("control"),
            &[Word::fixnum(7)],
            &mut ctx,
            &runtime,
            &mut sink
        ),
        Ok(1)
    );
    assert_eq!(sink.into_string(), "7");
}

#[test]
fn rejects_character_and_relative_repeat_parameters() {
    let (runtime, mut ctx) = context();
    let mut sink = StringSink::new();
    assert_eq!(
        execute(
            &parse("~'x%").expect("control"),
            &[],
            &mut ctx,
            &runtime,
            &mut sink,
        ),
        Err(FormatError::InvalidParameter {
            directive: ncl_lib_format::DirectiveKind::Percent,
        })
    );
    assert_eq!(
        execute(
            &parse("~v~").expect("control"),
            &[],
            &mut ctx,
            &runtime,
            &mut sink,
        ),
        Err(FormatError::InvalidParameter {
            directive: ncl_lib_format::DirectiveKind::Tilde,
        })
    );
}
#[test]
fn executes_character_radix_float_width_and_printer_directives() {
    let (runtime, mut ctx) = context();
    let float = make_double(&mut ctx, &runtime, 1.25).expect("float");
    let control = parse("~C/~16R/~F/~E/~G/~$/~5A/~5@A/~W").expect("control");
    let mut sink = StringSink::new();
    let value = string(&runtime, &mut ctx, "x");
    execute(
        &control,
        &[
            Word::character(u32::from('a')),
            Word::fixnum(255),
            float.into(),
            float.into(),
            float.into(),
            float.into(),
            value,
            value,
            value,
        ],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect("execute");
    assert_eq!(
        sink.into_string(),
        "a/FF/1.25/1.25/1.25/1.25/    x/x    /\"x\""
    );
}

#[test]
fn executes_compound_control_directives() {
    let (runtime, mut ctx) = context();
    let one = string(&runtime, &mut ctx, "one");
    let two = string(&runtime, &mut ctx, "two");
    let tail = make_cons(&mut ctx, &runtime, two, Word::NIL).expect("tail");
    let list = make_cons(&mut ctx, &runtime, one, tail).expect("list");
    let controls = [
        ("~[zero~;one~]", vec![Word::fixnum(1)], "one"),
        ("~{~A,~}", vec![list], "one,two,"),
        ("~@(hello~)", vec![], "HELLO"),
        ("~10<ok~>", vec![], "        ok"),
        ("~?", vec![string(&runtime, &mut ctx, "~A"), list], "one"),
    ];
    for (control, arguments, expected) in controls {
        let mut sink = StringSink::new();
        execute(
            &parse(control).expect("control"),
            &arguments,
            &mut ctx,
            &runtime,
            &mut sink,
        )
        .expect("execute");
        assert_eq!(sink.into_string(), expected, "{control}");
    }
}

#[test]
fn executes_parameter_count_and_argument_navigation() {
    let (runtime, mut ctx) = context();
    let mut sink = StringSink::new();
    execute(
        &parse("~#_~A~-1*~A").expect("control"),
        &[Word::fixnum(1), Word::fixnum(2)],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect("execute");
    assert_eq!(sink.into_string(), "  11");
}

#[test]
fn executes_parameterized_float_formats_and_rejects_invalid_values() {
    let (runtime, mut ctx) = context();
    let float = make_double(&mut ctx, &runtime, 12.345).expect("float");
    let mut sink = StringSink::new();
    assert_eq!(
        parse("~8,2F").expect("parse").parts[0],
        ncl_lib_format::ControlPart::Directive(ncl_lib_format::Directive {
            parameters: vec![
                ncl_lib_format::Parameter::Integer(8),
                ncl_lib_format::Parameter::Integer(2)
            ],
            colon: false,
            at_sign: false,
            kind: ncl_lib_format::DirectiveKind::F
        })
    );
    execute(
        &parse("~8,2F/~10,2E/~8,1G/~8,3$").expect("control"),
        &[float.into(), float.into(), float.into(), float.into()],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect("execute");
    assert_eq!(sink.into_string(), "   12.35/    1.23e1/    12.3/  12.345");

    let mut sink = StringSink::new();
    assert!(
        execute(
            &parse("~F").expect("control"),
            &[Word::fixnum(1)],
            &mut ctx,
            &runtime,
            &mut sink,
        )
        .is_err()
    );
    assert!(
        execute(
            &parse("~8,2F").expect("control"),
            &[float.into()],
            &mut ctx,
            &runtime,
            &mut sink,
        )
        .is_ok()
    );
}

#[test]
fn exercises_format_error_contract_and_malformed_compounds() {
    let (runtime, mut ctx) = context();
    let mut sink = StringSink::new();
    let cases = ["~[", "~{", "~(", "~<", "~?", "~16R", "~'x%"];
    for control in cases {
        let parsed = parse(control).expect("parse");
        assert!(
            execute(&parsed, &[], &mut ctx, &runtime, &mut sink).is_err(),
            "{control}"
        );
    }
    let errors = [
        FormatError::MissingArgument {
            directive: ncl_lib_format::DirectiveKind::A,
        },
        FormatError::InvalidParameter {
            directive: ncl_lib_format::DirectiveKind::A,
        },
        FormatError::NonInteger {
            directive: ncl_lib_format::DirectiveKind::D,
        },
    ];
    for error in errors {
        assert!(!error.to_string().is_empty());
        assert!(std::error::Error::source(&error).is_none());
    }
    let printed = FormatError::from(PrintError::Sink("closed".to_owned()));
    assert!(printed.to_string().contains("sink"));
    assert!(std::error::Error::source(&printed).is_some());
}

#[test]
fn exercises_early_termination_case_variants_and_parameter_errors() {
    let (runtime, mut ctx) = context();
    let mut sink = StringSink::new();
    execute(
        &parse("~^tail").expect("control"),
        &[],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect("execute");
    assert_eq!(sink.into_string(), "");
    let mut sink = StringSink::new();
    execute(
        &parse("~[zero~;one~]").expect("control"),
        &[Word::fixnum(9)],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect("execute");
    assert_eq!(sink.into_string(), "one");
    let mut sink = StringSink::new();
    execute(
        &parse("~[zero~;one~]").expect("control"),
        &[Word::TRUE],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect("execute");
    assert_eq!(sink.into_string(), "zero");
    let mut sink = StringSink::new();
    execute(
        &parse("~:(Hello~)").expect("control"),
        &[],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect("execute");
    assert_eq!(sink.into_string(), "hello");
    assert!(parse("~2,0T").is_err());
    assert!(parse("~-1A").is_err());
}

#[test]
fn exercises_invalid_lists_nested_formats_and_float_padding_parameters() {
    let (runtime, mut ctx) = context();
    let mut sink = StringSink::new();
    assert!(
        execute(
            &parse("~{~A~}").expect("control"),
            &[Word::fixnum(1)],
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
            &parse("~{~A~}").expect("control"),
            &[improper],
            &mut ctx,
            &runtime,
            &mut sink
        )
        .is_err()
    );
    assert!(
        execute(
            &parse("~?").expect("control"),
            &[Word::fixnum(1), Word::NIL],
            &mut ctx,
            &runtime,
            &mut sink
        )
        .is_err()
    );
    let invalid_control = string(&runtime, &mut ctx, "~");
    assert!(
        execute(
            &parse("~?").expect("control"),
            &[invalid_control, Word::NIL],
            &mut ctx,
            &runtime,
            &mut sink
        )
        .is_err()
    );
    let float = make_double(&mut ctx, &runtime, 1.2).expect("float");
    execute(
        &parse("~8,2,,,'0F").expect("control"),
        &[float.into()],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect("execute");
    assert!(sink.into_string().contains('0'));
}

#[test]
fn executes_remaining_directive_edges_and_reports_typed_parameter_errors() {
    let (runtime, mut ctx) = context();
    let mut sink = StringSink::new();

    execute(
        &parse("before~:>after~/tail~|~:_(x~)").expect("control"),
        &[],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect("control directives");
    assert!(sink.into_string().contains("before\nafter"));

    let value = string(&runtime, &mut ctx, "value");
    let improper = make_cons(&mut ctx, &runtime, value, Word::fixnum(1)).expect("improper");
    assert!(
        execute(
            &parse("~{~A~}").expect("brace"),
            &[improper],
            &mut ctx,
            &runtime,
            &mut StringSink::new(),
        )
        .is_err()
    );
    assert!(
        execute(
            &parse("~?").expect("nested"),
            &[Word::fixnum(1), Word::NIL],
            &mut ctx,
            &runtime,
            &mut StringSink::new(),
        )
        .is_err()
    );
    assert!(parse("~1R").is_err());
    assert!(parse("~37R").is_err());

    let integer = Word::fixnum(1);
    assert!(
        execute(
            &parse("~F").expect("float"),
            &[integer],
            &mut ctx,
            &runtime,
            &mut StringSink::new(),
        )
        .is_err()
    );
    assert!(
        execute(
            &parse("~$").expect("currency"),
            &[integer],
            &mut ctx,
            &runtime,
            &mut StringSink::new(),
        )
        .is_err()
    );
}
#[test]
fn format_errors_expose_specific_messages_and_sources() {
    use std::error::Error;

    let cases = [
        (
            FormatError::MissingArgument {
                directive: ncl_lib_format::DirectiveKind::A,
            },
            "format: missing argument for ~A",
        ),
        (
            FormatError::InvalidParameter {
                directive: ncl_lib_format::DirectiveKind::Percent,
            },
            "format: invalid parameter for ~Percent",
        ),
        (
            FormatError::NonInteger {
                directive: ncl_lib_format::DirectiveKind::D,
            },
            "format: expected integer for ~D",
        ),
    ];
    for (error, message) in cases {
        assert_eq!(error.to_string(), message);
        assert!(error.source().is_none());
    }

    let print_error = FormatError::from(PrintError::Sink("closed".to_owned()));
    assert_eq!(print_error.to_string(), "format: print: sink error: closed");
    assert_eq!(
        print_error.source().map(ToString::to_string),
        Some("print: sink error: closed".to_owned())
    );
}
