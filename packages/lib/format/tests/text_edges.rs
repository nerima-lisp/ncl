//! Boundary behavior for FORMAT text and printer directives.
#![expect(
    clippy::expect_used,
    reason = "Test setup uses expect and unwrap to keep fixture failures local."
)]

use std::error::Error;

use ncl_lib_format::{FormatErrorKind, execute, parse};
use ncl_object::{Runtime, ThreadContext, Word, make_bignum_from_i128, make_instance, make_string};
use ncl_printer::{CharSink, PrintError, StringSink};

struct FailingSink;

impl CharSink for FailingSink {
    fn write_char(&mut self, _character: char) -> Result<(), PrintError> {
        Err(PrintError::Sink("closed".to_owned()))
    }
}

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("register context");
    (runtime, ctx)
}

fn run(control: &str, arguments: &[Word], ctx: &mut ThreadContext, runtime: &Runtime) -> String {
    let parsed = parse(control).expect("control");
    let mut sink = StringSink::new();
    execute(&parsed, arguments, ctx, runtime, &mut sink).expect("execute");
    sink.into_string()
}

fn string(runtime: &Runtime, ctx: &mut ThreadContext, value: &str) -> Word {
    make_string(ctx, runtime, &value.chars().collect::<Vec<_>>()).expect("string")
}

#[test] // CLHS 22.3.1.2/.3/.4/.5, 22.3.9.3
fn line_control_directives_honor_zero_counts_and_repeat_ampersands() {
    let (runtime, mut ctx) = context();
    assert_eq!(run("~0%~0|~0~x", &[], &mut ctx, &runtime), "x");
    assert_eq!(run("a~3&b", &[], &mut ctx, &runtime), "a\n\n\nb");
    assert_eq!(run("a~0&b", &[], &mut ctx, &runtime), "ab");
    assert_eq!(run("~&x~3~", &[], &mut ctx, &runtime), "x~~~");
    assert_eq!(run("a~@\n  b", &[], &mut ctx, &runtime), "a\nb");

    let parsed = parse("~v%").expect("control");
    let mut sink = StringSink::new();
    let error = execute(
        &parsed,
        &[Word::character(u32::from('2'))],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect_err("character cannot be a line count");
    assert_eq!(error.kind, FormatErrorKind::InvalidParameter);
}

#[test]
fn line_directives_keep_their_location_when_the_output_sink_fails() {
    let (runtime, mut ctx) = context();
    let cases = [
        // CLHS 22.3.1.3: Fresh Line emits the remaining newlines for this count.
        ("~3&", '&'),
        // CLHS 22.3.1.5: each tilde emits one literal tilde.
        ("~3~", '~'),
        // CLHS 22.3.9.3: at-sign makes the newline directive emit a newline.
        ("~@\n", '\n'),
    ];

    for (control, directive) in cases {
        let parsed = parse(control).expect("control");
        let mut sink = FailingSink;
        let error = execute(&parsed, &[], &mut ctx, &runtime, &mut sink)
            .expect_err("line directive output should preserve the sink error");

        assert_eq!(error.position, 0, "{control}");
        assert_eq!(error.directive, Some(directive), "{control}");
        assert_eq!(
            error.kind,
            FormatErrorKind::Print(PrintError::Sink("closed".to_owned())),
            "{control}"
        );
    }
}

#[test] // CLHS 22.3.1.1
fn character_controls_reject_non_scalars_and_parameters() {
    let (runtime, mut ctx) = context();
    for (control, argument, expected) in [
        (
            "~C",
            Word::character(0xd800),
            FormatErrorKind::WrongArgumentType,
        ),
        (
            "~1C",
            Word::character(u32::from('x')),
            FormatErrorKind::InvalidParameter,
        ),
    ] {
        let parsed = parse(control).expect("control");
        let mut sink = StringSink::new();
        let error = execute(&parsed, &[argument], &mut ctx, &runtime, &mut sink)
            .expect_err("invalid character input");
        assert_eq!(error.kind, expected, "{control}");
    }
}

#[test] // CLHS 22.3.4.2
fn unreadable_object_errors_keep_the_directive_location() {
    let (runtime, mut ctx) = context();
    let instance = make_instance(&mut ctx, &runtime, Word::NIL, &[])
        .map(Into::into)
        .expect("instance");
    let parsed = parse("pre~S").expect("control");
    let mut sink = StringSink::new();
    let error = execute(&parsed, &[instance], &mut ctx, &runtime, &mut sink)
        .expect_err("an instance has no readable syntax");
    assert_eq!(error.position, 3);
    assert_eq!(error.directive, Some('S'));
    assert_eq!(error.kind, FormatErrorKind::Print(PrintError::NotReadable));
    assert_eq!(
        error.to_string(),
        "format error at offset 3 (~S): Print(NotReadable)"
    );
    assert!(Error::source(&error).is_some());
}

#[test] // CLHS 22.3.8.3
fn plural_directives_report_missing_arguments_and_reject_parameters() {
    let (runtime, mut ctx) = context();
    for (control, arguments, expected) in [
        ("~P", vec![], FormatErrorKind::MissingArgument),
        ("~:P", vec![], FormatErrorKind::MissingArgument),
        (
            "~1P",
            vec![Word::fixnum(1)],
            FormatErrorKind::InvalidParameter,
        ),
    ] {
        let parsed = parse(control).expect("control");
        let mut sink = StringSink::new();
        let error = execute(&parsed, &arguments, &mut ctx, &runtime, &mut sink)
            .expect_err("invalid plural input");
        assert_eq!(error.kind, expected, "{control}");
    }
}

#[test] // CLHS 22.3.4.1/.2/.3
fn value_directive_parameter_errors_keep_their_type_and_location() {
    let (runtime, mut ctx) = context();
    let value = string(&runtime, &mut ctx, "x");
    let wrong_parameter = string(&runtime, &mut ctx, "not an integer");
    let too_large = make_bignum_from_i128(&mut ctx, &runtime, i128::MAX)
        .map(Into::into)
        .expect("large relative parameter");

    for (control, arguments, expected) in [
        (
            "~vA",
            vec![wrong_parameter, value],
            FormatErrorKind::WrongArgumentType,
        ),
        ("~1W", vec![value], FormatErrorKind::InvalidParameter),
        ("~-1A", vec![value], FormatErrorKind::InvalidParameter),
        ("~'xA", vec![value], FormatErrorKind::InvalidParameter),
        ("~1,0,0A", vec![value], FormatErrorKind::InvalidParameter),
        ("~1,1,0,2A", vec![value], FormatErrorKind::InvalidParameter),
        ("~1,1,'xA", vec![value], FormatErrorKind::InvalidParameter),
        (
            "~vA",
            vec![too_large, value],
            FormatErrorKind::InvalidParameter,
        ),
    ] {
        let parsed = parse(control).expect("control");
        let mut sink = StringSink::new();
        let error = execute(&parsed, &arguments, &mut ctx, &runtime, &mut sink)
            .expect_err("invalid value directive parameter");
        assert_eq!(error.position, 0, "{control}");
        assert_eq!(
            error.directive,
            Some(control.chars().last().expect("directive"))
        );
        assert_eq!(error.kind, expected, "{control}");
        assert!(Error::source(&error).is_none(), "{control}");
    }
}
