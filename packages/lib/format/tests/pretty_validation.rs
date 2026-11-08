//! Boundary behavior for pretty layout and justification directives.

#![expect(
    clippy::expect_used,
    reason = "Test setup uses expect and unwrap to keep fixture failures local."
)]

use ncl_lib_format::{FormatErrorKind, execute, parse};
use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, FunctionObject, LambdaList, MultipleValues, ObjectError, Package,
    Parameter as BuiltinParameter, ParameterType, Runtime, ThreadContext, Word,
    make_bignum_from_i128, make_cons, make_string, set_symbol_special, set_symbol_value,
};
use ncl_printer::{CharSink, PrintError, StringSink};

const FORMATTER_PARAMETERS: &[BuiltinParameter] = &[
    BuiltinParameter {
        name: BuiltinName::new("STREAM"),
        ty: ParameterType::Any,
    },
    BuiltinParameter {
        name: BuiltinName::new("VALUE"),
        ty: ParameterType::Any,
    },
    BuiltinParameter {
        name: BuiltinName::new("COLON"),
        ty: ParameterType::Any,
    },
    BuiltinParameter {
        name: BuiltinName::new("AT-SIGN"),
        ty: ParameterType::Any,
    },
];

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
    ctx.register(&runtime).expect("context");
    ncl_lib_streams::register(&runtime).expect("streams");
    (runtime, ctx)
}

fn string(runtime: &Runtime, ctx: &mut ThreadContext, text: &str) -> Word {
    make_string(ctx, runtime, &text.chars().collect::<Vec<_>>()).expect("string")
}

fn formatter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    arguments: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = arguments.required(0)?;
    let value = arguments.required(1)?;
    let write_string = runtime
        .function(ctx, "COMMON-LISP", "WRITE-STRING")
        .and_then(|word| FunctionObject::try_from(word).ok())
        .ok_or(ObjectError::UndefinedFunction)?;
    runtime.call_builtin(ctx, write_string, &[value, stream])?;
    Ok(value)
}

const fn failing_formatter(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    _arguments: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Err(ObjectError::TypeError)
}

fn register_formatter(ctx: &mut ThreadContext, runtime: &Runtime) {
    let descriptor = Builtin {
        lambda_list: LambdaList::fixed(FORMATTER_PARAMETERS),
        convention: BuiltinConvention::Direct(Arity::exact(4)),
    };
    runtime
        .register_builtin(
            ctx,
            BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("FORMATTER")),
            BuiltinImplementation::direct(descriptor, formatter),
        )
        .expect("register formatter");
}

fn register_failing_formatter(ctx: &mut ThreadContext, runtime: &Runtime) {
    let descriptor = Builtin {
        lambda_list: LambdaList::fixed(FORMATTER_PARAMETERS),
        convention: BuiltinConvention::Direct(Arity::exact(4)),
    };
    runtime
        .register_builtin(
            ctx,
            BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("FAIL-FORMATTER")),
            BuiltinImplementation::direct(descriptor, failing_formatter),
        )
        .expect("register failing formatter");
}

fn execute_text(
    control: &str,
    arguments: &[Word],
    ctx: &mut ThreadContext,
    runtime: &Runtime,
) -> (String, usize) {
    let control = parse(control).expect("format control");
    let mut sink = StringSink::new();
    let consumed = execute(&control, arguments, ctx, runtime, &mut sink).expect("execute");
    (sink.into_string(), consumed)
}

fn assert_format_error(
    control: &str,
    arguments: &[Word],
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    expected: &FormatErrorKind,
) {
    let parsed = parse(control).expect("format control");
    let mut sink = StringSink::new();
    let error = execute(&parsed, arguments, ctx, runtime, &mut sink).expect_err(control);
    assert_eq!(&error.kind, expected, "{control}");
}

fn enable_pretty(ctx: &mut ThreadContext, runtime: &Runtime) {
    let package = runtime
        .find_package(ctx, "COMMON-LISP")
        .expect("COMMON-LISP");
    let (symbol, _) = Package::from_word(package)
        .intern(ctx, runtime, "*PRINT-PRETTY*")
        .expect("intern *PRINT-PRETTY*");
    set_symbol_special(ctx, symbol, true).expect("mark special");
    set_symbol_value(ctx, symbol, Word::TRUE).expect("set pretty mode");
}

fn set_miser_width(ctx: &mut ThreadContext, runtime: &Runtime, width: Word) {
    let package = runtime
        .find_package(ctx, "COMMON-LISP")
        .expect("COMMON-LISP");
    let (symbol, _) = Package::from_word(package)
        .intern(ctx, runtime, "*PRINT-MISER-WIDTH*")
        .expect("intern *PRINT-MISER-WIDTH*");
    set_symbol_special(ctx, symbol, true).expect("mark miser width special");
    set_symbol_value(ctx, symbol, width).expect("set miser width");
}

fn assert_sink_error(
    control: &str,
    arguments: &[Word],
    remaining: usize,
    ctx: &mut ThreadContext,
    runtime: &Runtime,
) {
    let parsed = parse(control).expect("format control");
    let mut sink = FailingSink { remaining };
    let error = execute(&parsed, arguments, ctx, runtime, &mut sink).expect_err("sink failure");
    assert_eq!(
        error.kind,
        FormatErrorKind::Print(PrintError::Sink("closed".to_owned()))
    );
}

#[test] // CLHS 22.3.5.4: named formatter names use package and symbol syntax.
fn named_formatter_handles_name_edges_and_missing_arguments() {
    let (runtime, mut ctx) = context();
    register_formatter(&mut ctx, &runtime);
    let value = string(&runtime, &mut ctx, "x");
    for control in [
        "~/NCL-TEST::MISSING/",
        "~/NCL-TEST:FORMATTER/",
        "~/NO-SUCH-PACKAGE::FORMATTER/",
        "~/FORMATTER/",
        "~/:FORMATTER/",
        "~/NCL-TEST:/",
    ] {
        assert_format_error(
            control,
            &[value],
            &mut ctx,
            &runtime,
            &FormatErrorKind::InvalidDirectiveUse,
        );
    }
    assert_format_error(
        "~/NCL-TEST::FORMATTER/",
        &[],
        &mut ctx,
        &runtime,
        &FormatErrorKind::MissingArgument,
    );
}

#[test] // CLHS 22.3.5.4: the named directive invokes its selected formatter function.
fn named_formatter_callback_errors_become_invalid_directive_use() {
    let (runtime, mut ctx) = context();
    register_failing_formatter(&mut ctx, &runtime);
    assert_format_error(
        "~/NCL-TEST::FAIL-FORMATTER/",
        &[Word::NIL],
        &mut ctx,
        &runtime,
        &FormatErrorKind::InvalidDirectiveUse,
    );
}

#[test] // CLHS 22.3.5.4: named formatter output is copied from its string stream.
fn named_formatter_preserves_sink_errors() {
    let (runtime, mut ctx) = context();
    register_formatter(&mut ctx, &runtime);
    let value = string(&runtime, &mut ctx, "x");
    let control = parse("~/NCL-TEST::FORMATTER/").expect("format control");
    let mut sink = FailingSink { remaining: 0 };
    let error = execute(&control, &[value], &mut ctx, &runtime, &mut sink)
        .expect_err("named formatter sink failure");
    assert_eq!(error.directive, Some('/'));
    assert_eq!(
        error.kind,
        FormatErrorKind::Print(PrintError::Sink("closed".to_owned()))
    );
}

#[test] // CLHS 22.3.5.2: non-list arguments are printed and malformed lists signal errors.
fn logical_block_prints_non_lists_and_rejects_dotted_lists() {
    let (runtime, mut ctx) = context();
    assert_eq!(
        execute_text("~<pre~;~A~;post~:>", &[Word::fixnum(1)], &mut ctx, &runtime),
        ("1".into(), 1)
    );
    let control = parse("~<~A~:>").expect("format control");
    let dotted =
        make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2)).expect("dotted list");
    let mut sink = StringSink::new();
    let error = execute(&control, &[dotted], &mut ctx, &runtime, &mut sink)
        .expect_err("dotted block argument");
    assert_eq!(error.kind, FormatErrorKind::ImproperList);
    assert_eq!(
        execute_text("~:<~:>", &[Word::NIL], &mut ctx, &runtime),
        ("()".into(), 1)
    );
}

#[test] // CLHS 22.3.5.3 and 22.3.6.1: indentation and tab parameters have defaults.
fn indent_and_tab_directives_use_defaults_and_handle_zero_increments() {
    let (runtime, mut ctx) = context();
    for control in ["a~I~%x", "a~,I~%x"] {
        assert_eq!(execute_text(control, &[], &mut ctx, &runtime).0, "a\nx");
    }
    enable_pretty(&mut ctx, &runtime);
    assert_eq!(execute_text("abc~2,0Tz", &[], &mut ctx, &runtime).0, "abcz");
    assert_eq!(
        execute_text("abc~3,0@Tz", &[], &mut ctx, &runtime).0,
        "abc   z"
    );
}

#[test] // CLHS 22.3.5.1: the @ conditional newline uses *PRINT-MISER-WIDTH*.
fn underscore_miser_width_accepts_and_ignores_signed_bignums() {
    let (runtime, mut ctx) = context();
    enable_pretty(&mut ctx, &runtime);
    let prefix = "p".repeat(40);
    let content = "x".repeat(41);
    let control = format!("~@<{prefix}~;{content}~@_tail~:>");
    let wide = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 62)
        .map(Into::into)
        .expect("wide miser width");
    set_miser_width(&mut ctx, &runtime, wide);
    assert_eq!(
        execute_text(&control, &[], &mut ctx, &runtime).0,
        format!("{prefix}{content}\n{}tail", " ".repeat(40))
    );
    let negative = make_bignum_from_i128(&mut ctx, &runtime, -(1_i128 << 62))
        .map(Into::into)
        .expect("negative miser width");
    set_miser_width(&mut ctx, &runtime, negative);
    assert_eq!(
        execute_text(&control, &[], &mut ctx, &runtime).0,
        format!("{prefix}{content}tail")
    );
}

#[test] // CLHS 22.3.5.1, 22.3.5.2, 22.3.5.3, 22.3.6.1, and 22.3.6.2 validate directive use.
fn layout_and_justification_reject_invalid_parameter_shapes() {
    let (runtime, mut ctx) = context();
    enable_pretty(&mut ctx, &runtime);
    for (control, arguments) in [
        ("~@I", vec![]),
        ("~1,2I", vec![]),
        ("~1,2,3T", vec![]),
        ("~2<~A~:>", vec![]),
        ("~<~A~;~B~;~C~;~D~:>", vec![Word::NIL]),
        ("~<prefix~A~;~A~:>", vec![Word::NIL]),
        ("~<pre~;body~;post~A~:>", vec![Word::NIL]),
    ] {
        assert_format_error(
            control,
            &arguments,
            &mut ctx,
            &runtime,
            &FormatErrorKind::InvalidDirectiveUse,
        );
    }
    for (control, arguments) in [
        ("~VI", vec![Word::character(u32::from('x'))]),
        ("~-1T", vec![]),
        ("~VT", vec![Word::character(u32::from('x'))]),
        ("~1,-1<abc~>", vec![]),
        ("~1,1,-1<abc~>", vec![]),
        ("~0,1,9223372036854775807<one~;two~;three~;four~>", vec![]),
        ("~5,1,0,32<abc~>", vec![]),
        ("~10<pre~2,V:;body~>", vec![Word::character(u32::from('x'))]),
        (
            "~V<~A~>",
            vec![Word::character(u32::from('x')), Word::fixnum(1)],
        ),
        ("~-1<~A~>", vec![Word::fixnum(1)]),
        ("~1,2,3,4,5<~A~>", vec![Word::fixnum(1)]),
    ] {
        assert_format_error(
            control,
            &arguments,
            &mut ctx,
            &runtime,
            &FormatErrorKind::InvalidParameter,
        );
    }
    for control in [
        "~1_",
        "~<a~@;b~>",
        "~<a~1;b~>",
        "~<a~:;b~:;c~>",
        "~<pre~1,2,3:;body~>",
    ] {
        assert_format_error(
            control,
            &[],
            &mut ctx,
            &runtime,
            &FormatErrorKind::InvalidDirectiveUse,
        );
    }
}

#[test] // CLHS 22.3.5.2 and 22.3.5.3: fill prefixes and indentation compose across lines.
fn fill_prefix_wraps_at_words_and_indent_survives_prefixed_lines() {
    let (runtime, mut ctx) = context();
    enable_pretty(&mut ctx, &runtime);
    let first = "a".repeat(75);
    let second = "b".repeat(10);
    let control = format!("~@<P>~@;{first} {second}~:@>");
    let expected = format!("P>{first}\nP>{second}");
    assert_eq!(execute_text(&control, &[], &mut ctx, &runtime).0, expected);
    assert_eq!(
        execute_text("~@<P>~@;short word~:@>", &[], &mut ctx, &runtime).0,
        "P>short word"
    );
    assert_eq!(
        execute_text("~@<P>~@;~2:I~%x~:@>", &[], &mut ctx, &runtime).0,
        "P>\nP>  x"
    );
}

#[test] // CLHS 22.3.6.2: justification rounds field width and honors prefix spare parameters.
fn justification_rounds_width_and_uses_prefix_spare_parameters() {
    let (runtime, mut ctx) = context();
    let current_line = "x".repeat(78);
    for (control, expected) in [("~2,4<12345~>", " 12345"), ("~4,0<abc~>", " abc")] {
        assert_eq!(execute_text(control, &[], &mut ctx, &runtime).0, expected);
    }
    for (control, expected) in [
        ("~5,1,0,'-<abc~>", "--abc"),
        ("~10<pre~2:;body~>", "      body"),
        ("~10<first~;body~^ignored~;tail~>", "first body"),
    ] {
        assert_eq!(execute_text(control, &[], &mut ctx, &runtime).0, expected);
    }
    assert_eq!(
        execute_text(
            "~10<pre~2,V:;body~>",
            &[Word::fixnum(15)],
            &mut ctx,
            &runtime
        )
        .0,
        "      body"
    );
    let no_wrap = format!("{current_line}~10<pre~2,90:;body~>");
    assert_eq!(
        execute_text(&no_wrap, &[], &mut ctx, &runtime).0,
        format!("{current_line}      body")
    );
    let wrap = format!("{current_line}~10<pre~2,89:;body~>");
    assert_eq!(
        execute_text(&wrap, &[], &mut ctx, &runtime).0,
        format!("{current_line}pre      body")
    );
}

#[test] // CLHS 22.3.5.2 and 22.3.5.3: prefixed logical blocks execute layout directives.
fn prefixed_layout_propagates_sink_errors_from_directives_and_wrapped_lines() {
    let (runtime, mut ctx) = context();
    enable_pretty(&mut ctx, &runtime);
    assert_sink_error("~@<P>~@;~A~:@>", &[Word::fixnum(7)], 2, &mut ctx, &runtime);

    let long_word = "x".repeat(80);
    let control = format!("~@<P>~@;first {long_word}~:@>");
    assert_sink_error(&control, &[], 8, &mut ctx, &runtime);

    assert_sink_error("~@<P>~@;~2:I~%x~:@>", &[], 6, &mut ctx, &runtime);
}
