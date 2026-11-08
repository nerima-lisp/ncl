//! Boundary behavior for FORMAT control execution.

#![expect(
    clippy::expect_used,
    reason = "Test setup uses expect and unwrap to keep fixture failures local."
)]

use ncl_lib_format::{
    ControlPart, ControlSegment, DirectiveBody, DirectiveModifiers, FormatControl, FormatError,
    FormatErrorKind, execute, parse,
};
use ncl_object::{
    Package, Runtime, ThreadContext, Word, make_bignum_from_i128, make_cons, make_instance,
    make_string, set_symbol_special, set_symbol_value,
};
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

fn string(runtime: &Runtime, ctx: &mut ThreadContext, value: &str) -> Word {
    make_string(ctx, runtime, &value.chars().collect::<Vec<_>>()).expect("string")
}

fn list(runtime: &Runtime, ctx: &mut ThreadContext, values: &[Word]) -> Word {
    values.iter().rev().copied().fold(Word::NIL, |tail, value| {
        make_cons(ctx, runtime, value, tail).expect("cons")
    })
}

fn set_print_readably(ctx: &mut ThreadContext, runtime: &Runtime) {
    let package = runtime
        .find_package(ctx, "COMMON-LISP")
        .expect("COMMON-LISP package");
    let (symbol, _) = Package::from_word(package)
        .intern(ctx, runtime, "*PRINT-READABLY*")
        .expect("print special");
    set_symbol_special(ctx, symbol, true).expect("mark print special");
    set_symbol_value(ctx, symbol, Word::TRUE).expect("set print special");
}

fn run(
    control: &str,
    arguments: &[Word],
    ctx: &mut ThreadContext,
    runtime: &Runtime,
) -> (String, usize) {
    let parsed = parse(control).expect("control");
    let mut sink = StringSink::new();
    let consumed = execute(&parsed, arguments, ctx, runtime, &mut sink).expect("execute");
    (sink.into_string(), consumed)
}

fn assert_error(
    control: &str,
    arguments: &[Word],
    expected: &FormatErrorKind,
    ctx: &mut ThreadContext,
    runtime: &Runtime,
) {
    let parsed = parse(control).expect("control");
    let mut sink = StringSink::new();
    let error = execute(&parsed, arguments, ctx, runtime, &mut sink).expect_err("execute");
    assert_eq!(&error.kind, expected, "{control}");
}

#[test] // CLHS 22.3.7.1, 22.3.9.2
fn argument_pointer_and_up_arrow_follow_parameter_decisions() {
    let (runtime, mut ctx) = context();
    let a = string(&runtime, &mut ctx, "a");
    assert_error(
        "~1,2*",
        &[a, a],
        &FormatErrorKind::InvalidParameter,
        &mut ctx,
        &runtime,
    );
    for (control, expected) in [
        ("~0^x", (String::new(), 0)),
        ("~1^x", ("x".to_owned(), 0)),
        ("~1,1^x", (String::new(), 0)),
        ("~1,2^x", ("x".to_owned(), 0)),
        ("~1,2,3^x", (String::new(), 0)),
        ("~2,1,3^x", ("x".to_owned(), 0)),
    ] {
        assert_eq!(run(control, &[], &mut ctx, &runtime), expected, "{control}");
    }
}

#[test] // CLHS 22.3, 22.3.9.2
fn relative_integer_parameters_accept_signed_bignum_boundaries() {
    let (runtime, mut ctx) = context();
    for integer in [-3_i128, i128::from(i64::MIN), i128::from(i64::MAX)] {
        let parameter = make_bignum_from_i128(&mut ctx, &runtime, integer)
            .map(Into::into)
            .expect("bignum parameter");
        assert_eq!(
            run("~v^x", &[parameter, Word::NIL], &mut ctx, &runtime),
            ("x".to_owned(), 1),
            "{integer}"
        );
    }
    let too_large = make_bignum_from_i128(&mut ctx, &runtime, i128::MAX)
        .map(Into::into)
        .expect("large bignum parameter");
    assert_error(
        "~v^",
        &[too_large],
        &FormatErrorKind::InvalidParameter,
        &mut ctx,
        &runtime,
    );
}

#[test] // CLHS 22.3.7.2
fn conditional_modifiers_select_and_consume_their_arguments() {
    let (runtime, mut ctx) = context();
    for (control, arguments, expected) in [
        (
            "~@[selected~]",
            vec![Word::TRUE],
            ("selected".to_owned(), 0),
        ),
        ("~@[selected~]", vec![Word::NIL], (String::new(), 1)),
        ("~:[false~;true~]", vec![Word::NIL], ("false".to_owned(), 1)),
        ("~:[false~;true~]", vec![Word::TRUE], ("true".to_owned(), 1)),
        (
            "~v[zero~;one~]",
            vec![Word::fixnum(1)],
            ("one".to_owned(), 1),
        ),
    ] {
        assert_eq!(
            run(control, &arguments, &mut ctx, &runtime),
            expected,
            "{control}"
        );
    }
    assert_error(
        "~@[selected~]",
        &[],
        &FormatErrorKind::MissingArgument,
        &mut ctx,
        &runtime,
    );

    let parsed = parse("~@[selected~]").expect("control");
    let mut directive = parsed
        .parts
        .into_iter()
        .find_map(|part| match part {
            ControlPart::Directive(directive) => Some(directive),
            ControlPart::Literal(_) => None,
        })
        .expect("conditional directive");
    directive.body = DirectiveBody::Clauses(vec![ControlSegment {
        separator: Some(DirectiveModifiers {
            parameters: vec![],
            colon: false,
            at_sign: true,
            position: 1,
        }),
        control: FormatControl {
            parts: vec![ControlPart::Literal("selected".to_owned())],
        },
    }]);
    let malformed = FormatControl {
        parts: vec![ControlPart::Directive(directive)],
    };
    let mut sink = StringSink::new();
    let error = execute(&malformed, &[Word::TRUE], &mut ctx, &runtime, &mut sink)
        .expect_err("at-sign clauses reject decorated separators");
    assert_eq!(error.kind, FormatErrorKind::InvalidDirectiveUse);

    let mut directive = parse("~:[false~;true~]")
        .expect("colon conditional")
        .parts
        .into_iter()
        .find_map(|part| match part {
            ControlPart::Directive(directive) => Some(directive),
            ControlPart::Literal(_) => None,
        })
        .expect("colon directive");
    let separator = match &mut directive.body {
        DirectiveBody::None | DirectiveBody::Sequence(_) => None,
        DirectiveBody::Clauses(clauses) => clauses[1].separator.as_mut(),
    }
    .expect("conditional separator");
    separator.at_sign = true;
    let malformed = FormatControl {
        parts: vec![ControlPart::Directive(directive)],
    };
    let error = execute(&malformed, &[Word::NIL], &mut ctx, &runtime, &mut sink)
        .expect_err("colon clauses reject decorated separators");
    assert_eq!(error.kind, FormatErrorKind::InvalidDirectiveUse);
}

#[test] // CLHS 22.3.7.4/.5, 22.3.9.2
fn iterations_report_missing_lists_and_stop_on_each_escape_flow() {
    let (runtime, mut ctx) = context();
    for (control, arguments) in [
        ("~{~A~}", vec![]),
        ("~:{~A~}", vec![]),
        ("~:{~A~:}", vec![Word::NIL]),
        ("~:@{~A~:}", vec![]),
    ] {
        assert_error(
            control,
            &arguments,
            &FormatErrorKind::MissingArgument,
            &mut ctx,
            &runtime,
        );
    }

    let value = string(&runtime, &mut ctx, "x");
    assert_eq!(
        run("~@{~0^skip~A~}", &[value], &mut ctx, &runtime),
        (String::new(), 0)
    );

    let a = string(&runtime, &mut ctx, "a");
    let b = string(&runtime, &mut ctx, "b");
    let row_a = list(&runtime, &mut ctx, &[a]);
    let row_b = list(&runtime, &mut ctx, &[b]);
    assert_eq!(
        run("~:@{~A~:^skip~}done", &[row_a, row_b], &mut ctx, &runtime),
        ("askipbdone".to_owned(), 2)
    );

    let improper = make_cons(&mut ctx, &runtime, a, Word::fixnum(9)).expect("improper row");
    assert_error(
        "~:@{~A~}",
        &[improper],
        &FormatErrorKind::ImproperList,
        &mut ctx,
        &runtime,
    );
    let empty_row = list(&runtime, &mut ctx, &[Word::NIL]);
    assert_error(
        "~:{~A~}",
        &[empty_row],
        &FormatErrorKind::MissingArgument,
        &mut ctx,
        &runtime,
    );

    assert_eq!(
        run("~{x~:}", &[Word::NIL], &mut ctx, &runtime),
        ("x".to_owned(), 1)
    );
    assert_eq!(run("~@{x~:}", &[], &mut ctx, &runtime), ("x".to_owned(), 0));

    for (control, arguments) in [("~{literal~}", vec![empty_row]), ("~@{literal~}", vec![a])] {
        assert_error(
            control,
            &arguments,
            &FormatErrorKind::InvalidDirectiveUse,
            &mut ctx,
            &runtime,
        );
    }
}

#[test] // CLHS 22.3.8.1
fn case_conversion_propagates_body_errors() {
    let (runtime, mut ctx) = context();
    assert_error(
        "~(~A~)",
        &[],
        &FormatErrorKind::MissingArgument,
        &mut ctx,
        &runtime,
    );
    assert_error(
        "~1(~A~)",
        &[Word::fixnum(1)],
        &FormatErrorKind::InvalidDirectiveUse,
        &mut ctx,
        &runtime,
    );
    let wrong_parameter = string(&runtime, &mut ctx, "not an integer");
    assert_error(
        "~v(~A~)",
        &[wrong_parameter],
        &FormatErrorKind::WrongArgumentType,
        &mut ctx,
        &runtime,
    );
}

#[test] // CLHS 22.3.7.6
fn recursive_function_control_requires_its_argument_list() {
    let (runtime, mut ctx) = context();
    ncl_lib_format::register(&runtime).expect("FORMAT");
    let formatter = runtime
        .function(&mut ctx, "COMMON-LISP", "FORMAT")
        .expect("FORMAT function");
    assert_error(
        "~?",
        &[formatter],
        &FormatErrorKind::MissingArgument,
        &mut ctx,
        &runtime,
    );
}

#[test] // CLHS 22.3.7.6, 22.3.9.2
fn recursive_string_control_executes_nested_and_escape_paths() {
    let (runtime, mut ctx) = context();
    let nested_control = string(&runtime, &mut ctx, "~A");
    let value = string(&runtime, &mut ctx, "nested");
    let nested_arguments = list(&runtime, &mut ctx, &[value]);
    assert_eq!(
        run(
            "~?",
            &[nested_control, nested_arguments],
            &mut ctx,
            &runtime
        ),
        ("nested".to_owned(), 2)
    );
    assert_eq!(
        run("~@?", &[nested_control, value], &mut ctx, &runtime),
        ("nested".to_owned(), 2)
    );

    let escape_control = string(&runtime, &mut ctx, "~0^hidden");
    let empty_arguments = list(&runtime, &mut ctx, &[]);
    assert_eq!(
        run(
            "~?after",
            &[escape_control, empty_arguments],
            &mut ctx,
            &runtime
        ),
        ("after".to_owned(), 2)
    );
}

#[test] // CLHS 22.3.7.6
fn recursive_function_control_maps_a_missing_stream_constructor() {
    let (runtime, mut ctx) = context();
    ncl_lib_format::register(&runtime).expect("FORMAT");
    let formatter = runtime
        .function(&mut ctx, "COMMON-LISP", "FORMAT")
        .expect("FORMAT function");
    let arguments = list(&runtime, &mut ctx, &[]);
    let parsed = parse("~?").expect("control");
    let mut sink = StringSink::new();
    let error = execute(
        &parsed,
        &[formatter, arguments],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect_err("missing stream constructor");
    assert_eq!(
        error.kind,
        FormatErrorKind::Print(PrintError::Object(
            ncl_object::ObjectError::UndefinedFunction
        ))
    );
}

#[test] // CLHS 22.3.7.6
fn recursive_function_control_maps_an_undefined_function_designator() {
    let (runtime, mut ctx) = context();
    ncl_lib_streams::register(&runtime).expect("streams");
    let common_lisp = runtime
        .find_package(&ctx, "COMMON-LISP")
        .expect("COMMON-LISP package");
    let (missing_formatter, _) = Package::from_word(common_lisp)
        .intern(&mut ctx, &runtime, "MISSING-FORMATTER")
        .expect("formatter symbol");
    let arguments = list(&runtime, &mut ctx, &[]);
    let parsed = parse("~?").expect("control");
    let mut sink = StringSink::new();
    let error = execute(
        &parsed,
        &[missing_formatter, arguments],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect_err("undefined formatter");
    assert_eq!(
        error.kind,
        FormatErrorKind::Print(PrintError::Object(
            ncl_object::ObjectError::UndefinedFunction
        ))
    );
}

#[test] // CLHS 22.3.7.6
fn recursive_function_output_errors_keep_the_recursive_directive_location() {
    let (runtime, mut ctx) = context();
    ncl_lib_streams::register(&runtime).expect("streams");
    ncl_lib_format::register(&runtime).expect("FORMAT");
    let formatter = runtime
        .function(&mut ctx, "COMMON-LISP", "FORMAT")
        .expect("FORMAT function");
    let control = string(&runtime, &mut ctx, "text");
    let arguments = list(&runtime, &mut ctx, &[control]);
    let parsed = parse("~?").expect("control");
    let mut sink = FailingSink { remaining: 0 };
    let error = execute(
        &parsed,
        &[formatter, arguments],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect_err("recursive output sink failure");
    assert_eq!(error.position, 0);
    assert_eq!(error.directive, Some('?'));
    assert_eq!(
        error.kind,
        FormatErrorKind::Print(PrintError::Sink("closed".to_owned()))
    );
}

#[test] // CLHS 22.3.5.2
fn logical_block_printing_maps_render_and_indentation_sink_failures() {
    let (runtime, mut ctx) = context();
    let non_list = parse("~:<~A~:>").expect("control");
    let mut sink = StringSink::new();
    assert_eq!(
        execute(&non_list, &[Word::fixnum(7)], &mut ctx, &runtime, &mut sink,),
        Ok(1)
    );
    assert_eq!(sink.into_string(), "7");

    let mut sink = FailingSink { remaining: 0 };
    let error = execute(&non_list, &[Word::fixnum(7)], &mut ctx, &runtime, &mut sink)
        .expect_err("logical block sink failure");
    assert_eq!(error.position, 0);
    assert_eq!(error.directive, Some('<'));
    assert_eq!(
        error.kind,
        FormatErrorKind::Print(PrintError::Sink("closed".to_owned()))
    );

    set_print_readably(&mut ctx, &runtime);
    let instance = make_instance(&mut ctx, &runtime, Word::NIL, &[])
        .map(Into::into)
        .expect("instance");
    let error = execute(&non_list, &[instance], &mut ctx, &runtime, &mut sink)
        .expect_err("logical block value has no readable syntax");
    assert_eq!(error.position, 0);
    assert_eq!(error.directive, Some('<'));
    assert_eq!(error.kind, FormatErrorKind::Print(PrintError::NotReadable));

    let item = string(&runtime, &mut ctx, "x");
    let values = list(&runtime, &mut ctx, &[item]);
    let indented = parse("prefix~:<~%~A~:>").expect("control");
    let mut sink = FailingSink { remaining: 8 };
    let error = execute(&indented, &[values], &mut ctx, &runtime, &mut sink)
        .expect_err("indentation sink failure");
    assert_eq!(error.position, 11);
    assert_eq!(error.directive, Some('A'));
    assert_eq!(
        error.kind,
        FormatErrorKind::Print(PrintError::Sink("closed".to_owned()))
    );

    let no_directive = FormatError {
        position: 4,
        directive: None,
        kind: FormatErrorKind::InvalidParameter,
    };
    assert_eq!(
        no_directive.to_string(),
        "format error at offset 4: InvalidParameter"
    );
}
