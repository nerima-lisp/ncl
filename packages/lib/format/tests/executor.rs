#![expect(
    clippy::expect_used,
    reason = "tests assert on valid FORMAT controls and the expected printer output"
)]
//! Tests for FORMAT directive execution and printer output.

use std::error::Error;

use ncl_lib_format::{FormatError, FormatErrorKind, execute, parse};
use ncl_object::{Runtime, ThreadContext, Word, make_bignum_from_i128, make_string};
use ncl_printer::{CharSink, PrintError, StringSink};

struct FailingSink {
    remaining: usize,
}

impl CharSink for FailingSink {
    fn write_char(&mut self, _character: char) -> Result<(), PrintError> {
        if self.remaining == 0 {
            Err(PrintError::Sink("closed".to_owned()))
        } else {
            self.remaining -= 1;
            Ok(())
        }
    }
}

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("register context");
    (runtime, ctx)
}

fn string(runtime: &Runtime, ctx: &mut ThreadContext, text: &str) -> Word {
    make_string(ctx, runtime, &text.chars().collect::<Vec<char>>()).expect("string")
}

#[test] // CLHS 22.3.4.1/.2, 22.3.1.1
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

#[test] // CLHS 22.3.2, 22.3.1.2/.3
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

#[test] // CLHS 22.3.2, 22.3.4.1
fn rejects_missing_arguments_and_formats_non_integer_radix_arguments_as_aesthetic() {
    let (runtime, mut ctx) = context();
    let mut sink = StringSink::new();
    let missing = parse("~A").expect("control");
    let error =
        execute(&missing, &[], &mut ctx, &runtime, &mut sink).expect_err("missing argument");
    assert_eq!(error.position, 0);
    assert_eq!(error.directive, Some('A'));
    assert_eq!(error.kind, FormatErrorKind::MissingArgument);
    let non_integer = parse("~D").expect("control");
    let value = string(&runtime, &mut ctx, "not an integer");
    let consumed = execute(&non_integer, &[value], &mut ctx, &runtime, &mut sink)
        .expect("non-integer radix arguments use aesthetic output");
    assert_eq!(consumed, 1);
    assert_eq!(sink.into_string(), "not an integer");
}

#[test] // CLHS 22.3.1.3
fn ampersand_does_not_add_a_second_newline_at_line_start() {
    let (runtime, mut ctx) = context();
    let control = parse("a~%~&b").expect("control");
    let mut sink = StringSink::new();
    execute(&control, &[], &mut ctx, &runtime, &mut sink).expect("execute");
    assert_eq!(sink.into_string(), "a\nb");
}

#[test] // CLHS 22.3.1.3
fn ampersand_preserves_output_sink_errors() {
    let (runtime, mut ctx) = context();
    let control = parse("a~&").expect("control");
    let mut sink = FailingSink { remaining: 1 };
    let error = execute(&control, &[], &mut ctx, &runtime, &mut sink)
        .expect_err("ampersand newline write fails");

    assert_eq!(
        error.kind,
        FormatErrorKind::Print(PrintError::Sink("closed".to_owned()))
    );
}

#[test] // CLHS 22.3.4.1
fn directive_sink_errors_keep_the_directive_location() {
    let (runtime, mut ctx) = context();
    let control = parse("pre~A").expect("control");
    let value = string(&runtime, &mut ctx, "value");
    let mut sink = FailingSink { remaining: 3 };
    let error =
        execute(&control, &[value], &mut ctx, &runtime, &mut sink).expect_err("sink failure");
    assert_eq!(error.position, 3);
    assert_eq!(error.directive, Some('A'));
    assert_eq!(
        error.kind,
        FormatErrorKind::Print(PrintError::Sink("closed".to_owned()))
    );
}

#[test] // CLHS 22.3
fn dynamic_and_remaining_parameters_consume_the_expected_arguments() {
    let (runtime, mut ctx) = context();
    let value = string(&runtime, &mut ctx, "x");
    for (control, arguments, expected, consumed) in [
        ("~vA", vec![Word::fixnum(4), value], "x   ", 2),
        ("~v@A", vec![Word::fixnum(4), value], "   x", 2),
        ("~vA", vec![Word::NIL, value], "x", 2),
        ("~#A", vec![value, value], "x ", 1),
    ] {
        let parsed = parse(control).expect("control");
        let mut sink = StringSink::new();
        assert_eq!(
            execute(&parsed, &arguments, &mut ctx, &runtime, &mut sink),
            Ok(consumed),
            "{control} consumed count",
        );
        assert_eq!(sink.into_string(), expected, "{control} output");
    }

    let character_parameter = parse("~'xA").expect("control");
    let mut sink = StringSink::new();
    let error = execute(
        &character_parameter,
        &[value],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect_err("a character cannot be used as a minimum column count");
    assert_eq!(error.kind, FormatErrorKind::InvalidParameter);

    let wrong_type_parameter = parse("~vA").expect("control");
    let wrong_type = string(&runtime, &mut ctx, "not an integer parameter");
    let error = execute(
        &wrong_type_parameter,
        &[wrong_type, value],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect_err("relative numeric parameter type check");
    assert_eq!(error.kind, FormatErrorKind::WrongArgumentType);

    for parameter in [Word::character(u32::from('4')), Word::character(0xd800)] {
        let error = execute(
            &wrong_type_parameter,
            &[parameter, value],
            &mut ctx,
            &runtime,
            &mut sink,
        )
        .expect_err("character relative parameter");
        assert_eq!(
            error.kind,
            if parameter == Word::character(0xd800) {
                FormatErrorKind::WrongArgumentType
            } else {
                FormatErrorKind::InvalidParameter
            }
        );
    }
}

#[test] // CLHS 22.3
fn relative_bignum_parameters_preserve_signed_i64_boundaries() {
    let (runtime, mut ctx) = context();
    let large_positive = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 62)
        .map(Into::into)
        .expect("large positive bignum");
    let i64_min = make_bignum_from_i128(&mut ctx, &runtime, i128::from(i64::MIN))
        .map(Into::into)
        .expect("i64 minimum bignum");
    let too_large = make_bignum_from_i128(&mut ctx, &runtime, i128::from(i64::MAX) + 1)
        .map(Into::into)
        .expect("out-of-range bignum");
    let beyond_i64_limbs = make_bignum_from_i128(&mut ctx, &runtime, i128::MAX)
        .map(Into::into)
        .expect("bignum beyond two i64 limbs");

    for value in [large_positive, i64_min] {
        assert_eq!(run_parameter(&runtime, &mut ctx, value), "x");
    }

    let parsed = parse("~v^").expect("control");
    for value in [too_large, beyond_i64_limbs] {
        let mut sink = StringSink::new();
        let error = execute(&parsed, &[value], &mut ctx, &runtime, &mut sink)
            .expect_err("relative parameter must fit in i64");
        assert_eq!(error.kind, FormatErrorKind::InvalidParameter);
    }
}

fn run_parameter(runtime: &Runtime, ctx: &mut ThreadContext, value: Word) -> String {
    let parsed = parse("~v^x").expect("control");
    let mut sink = StringSink::new();
    assert_eq!(
        execute(&parsed, &[value, Word::NIL], ctx, runtime, &mut sink),
        Ok(1)
    );
    sink.into_string()
}

#[test] // CLHS 22.3.4.1
fn format_errors_display_locations_and_expose_print_sources() {
    let (runtime, mut ctx) = context();
    let missing = parse("pre~A").expect("control");
    let mut sink = StringSink::new();
    let error =
        execute(&missing, &[], &mut ctx, &runtime, &mut sink).expect_err("missing argument");
    assert_eq!(
        error.to_string(),
        "format error at offset 3 (~A): MissingArgument"
    );
    assert!(Error::source(&error).is_none());

    let no_directive = FormatError {
        position: 7,
        directive: None,
        kind: FormatErrorKind::InvalidParameter,
    };
    assert_eq!(
        no_directive.to_string(),
        "format error at offset 7: InvalidParameter"
    );

    let control = parse("~A").expect("control");
    let value = string(&runtime, &mut ctx, "x");
    let mut sink = FailingSink { remaining: 0 };
    let error =
        execute(&control, &[value], &mut ctx, &runtime, &mut sink).expect_err("printer sink error");
    assert!(Error::source(&error).is_some());
}

#[test] // CLHS 22.3.4.1, 22.3.5.2
fn sink_failures_keep_or_gain_a_directive_location() {
    let (runtime, mut ctx) = context();
    let value = string(&runtime, &mut ctx, "x");
    for (control, directive, arguments) in [
        ("literal", None, vec![]),
        ("~%", Some('%'), vec![]),
        ("~(x~)", Some('('), vec![]),
        ("~:<~A~:>", Some('<'), vec![Word::fixnum(7)]),
        ("~A", Some('A'), vec![value]),
    ] {
        let parsed = parse(control).expect("control");
        let mut sink = FailingSink { remaining: 0 };
        let error = execute(&parsed, &arguments, &mut ctx, &runtime, &mut sink)
            .expect_err("sink is closed");
        assert_eq!(error.position, 0, "{control}");
        assert_eq!(error.directive, directive, "{control}");
        assert_eq!(
            error.kind,
            FormatErrorKind::Print(PrintError::Sink("closed".to_owned()))
        );
    }
}

#[test] // CLHS 22.3.5.2
fn logical_block_non_list_values_use_the_printer_output_path() {
    let (runtime, mut ctx) = context();
    let parsed = parse("~:<~A~:>").expect("control");
    let mut sink = StringSink::new();
    assert_eq!(
        execute(&parsed, &[Word::fixnum(7)], &mut ctx, &runtime, &mut sink,),
        Ok(1)
    );
    assert_eq!(sink.into_string(), "7");
}
