//! Public-API edge cases for pretty FORMAT directives.

#![expect(
    clippy::expect_used,
    reason = "Test setup uses expect and unwrap to keep fixture failures local."
)]

use ncl_lib_format::{FormatErrorKind, execute, parse};
use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, LambdaList, MultipleValues, ObjectError, Package,
    Parameter as BuiltinParameter, ParameterType, Runtime, ThreadContext, Word, make_cons, rplacd,
    set_symbol_special, set_symbol_value,
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

fn execute_text(
    control: &str,
    arguments: &[Word],
    ctx: &mut ThreadContext,
    runtime: &Runtime,
) -> (String, usize) {
    let parsed = parse(control).expect("format control");
    let mut sink = StringSink::new();
    let consumed = execute(&parsed, arguments, ctx, runtime, &mut sink).expect("execute");
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

fn formatter(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    arguments: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    arguments.required(1)
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

#[test] // CLHS 22.3.5.3 and 22.3.6.1: layout directives accept dynamic parameters.
fn layout_reports_missing_dynamic_parameters_and_sink_failures() {
    let (runtime, mut ctx) = context();
    enable_pretty(&mut ctx, &runtime);
    assert_format_error(
        "~VI",
        &[],
        &mut ctx,
        &runtime,
        &FormatErrorKind::MissingArgument,
    );
    assert_format_error(
        "~VT",
        &[],
        &mut ctx,
        &runtime,
        &FormatErrorKind::MissingArgument,
    );
    assert_format_error(
        "~1,-1T",
        &[],
        &mut ctx,
        &runtime,
        &FormatErrorKind::InvalidParameter,
    );
    assert_sink_error("~1T", &[], 0, &mut ctx, &runtime);
}

#[test] // CLHS 22.3.5.2: logical blocks detect circular list arguments.
fn logical_block_rejects_cyclic_lists() {
    let (runtime, mut ctx) = context();
    let list = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL).expect("list");
    rplacd(&mut ctx, list, list).expect("make cycle");
    assert_format_error(
        "~<~A~:>",
        &[list],
        &mut ctx,
        &runtime,
        &FormatErrorKind::ImproperList,
    );
}

#[test] // CLHS 22.3.5.2: the logical-block body processes list or remaining arguments.
fn logical_block_bodies_propagate_errors_for_list_and_remaining_arguments() {
    let (runtime, mut ctx) = context();
    let item = make_cons(&mut ctx, &runtime, Word::fixnum(7), Word::NIL).expect("list");
    assert_format_error(
        "~<~A~A~:>",
        &[item],
        &mut ctx,
        &runtime,
        &FormatErrorKind::MissingArgument,
    );
    assert_format_error(
        "~@<~A~A~:>",
        &[Word::fixnum(7)],
        &mut ctx,
        &runtime,
        &FormatErrorKind::MissingArgument,
    );
}

#[test] // CLHS 22.3.6.2: special prefixes and clauses participate in field justification.
fn justification_wraps_special_prefixes_and_rejects_rendering_failures() {
    let (runtime, mut ctx) = context();
    assert_eq!(
        execute_text("~10<pre~2,11:;body~>", &[], &mut ctx, &runtime).0,
        "pre      body"
    );
    assert_format_error(
        "~<prefix~A~:;body~>",
        &[],
        &mut ctx,
        &runtime,
        &FormatErrorKind::MissingArgument,
    );
    assert_format_error(
        "~10<pre~2,V:;body~>",
        &[],
        &mut ctx,
        &runtime,
        &FormatErrorKind::MissingArgument,
    );
    assert_format_error(
        "~10<pre~V,2:;body~>",
        &[Word::character(u32::from('x'))],
        &mut ctx,
        &runtime,
        &FormatErrorKind::InvalidParameter,
    );
    for control in [
        "~0,1,9223372036854775807<one~;two~;three~>",
        "~0,1,9223372036854775807<one~;two~;three~;four~>",
        "~0,9223372036854775807,9223372036854775807<a~;bb~>",
        "~1,3,9223372036854775807<~;~;~>",
    ] {
        assert_format_error(
            control,
            &[],
            &mut ctx,
            &runtime,
            &FormatErrorKind::InvalidParameter,
        );
    }
}

#[test] // CLHS 22.3.6.2: spacing is distributed among clauses using their rendered widths.
fn justification_distributes_remainders_and_measures_multiline_segments() {
    let (runtime, mut ctx) = context();
    for (control, expected) in [
        ("~8:<a~;b~;c~>", "  a  b c"),
        ("~8@<a~;b~;c~>", "a  b  c "),
        ("~8:@<a~;b~;c~>", "  a b c "),
        ("~8<ab~%c~;d~>", "ab\nc      d"),
    ] {
        assert_eq!(
            execute_text(control, &[], &mut ctx, &runtime).0,
            expected,
            "{control}"
        );
    }
}

#[test] // CLHS 22.3.6.2: justification writes the special prefix and padded field.
fn justification_propagates_sink_errors_from_prefix_and_field_writes() {
    let (runtime, mut ctx) = context();
    assert_sink_error("~10<pre~2,11:;body~>", &[], 0, &mut ctx, &runtime);
    assert_sink_error("~5<abc~>", &[], 0, &mut ctx, &runtime);
}

#[test] // CLHS 22.3.5.2 and 22.3.5.3: fill prefixes persist across wrapped and indented lines.
fn prefixed_fill_and_directives_keep_prefixes_on_each_line() {
    let (runtime, mut ctx) = context();
    enable_pretty(&mut ctx, &runtime);
    assert_eq!(
        execute_text("~@<P>~@;short word~:@>", &[], &mut ctx, &runtime).0,
        "P>short word"
    );
    let first = "a".repeat(75);
    let second = "b".repeat(10);
    let control = format!("~@<P>~@;{first} {second}~:@>");
    assert_eq!(
        execute_text(&control, &[], &mut ctx, &runtime).0,
        format!("P>{first}\nP>{second}")
    );
    assert_eq!(
        execute_text("~@<P>~@;~%~A~:@>", &[Word::fixnum(7)], &mut ctx, &runtime).0,
        "P>\nP>7"
    );
}

#[test] // CLHS 22.3.5.2: per-line prefixes preserve literal spaces in fill text.
fn prefixed_fill_preserves_repeated_and_trailing_spaces() {
    let (runtime, mut ctx) = context();
    enable_pretty(&mut ctx, &runtime);
    for (body, expected) in [("a  b", "P>a  b"), ("a ", "P>a ")] {
        let control = format!("~@<P>~@;{body}~:@>");
        assert_eq!(execute_text(&control, &[], &mut ctx, &runtime).0, expected);
    }
}

#[test] // CLHS 22.3.5.2: logical-block prefix, suffix, and fill writes go to the output stream.
fn logical_blocks_propagate_sink_errors_from_prefix_suffix_and_fill_writes() {
    let (runtime, mut ctx) = context();
    enable_pretty(&mut ctx, &runtime);
    assert_sink_error("~<P~;body~:>", &[Word::NIL], 0, &mut ctx, &runtime);
    assert_sink_error("~@<P>~@;body~:@>", &[], 0, &mut ctx, &runtime);
    assert_sink_error(
        "~<P>~@;one\ntwo~;END~:>",
        &[Word::NIL],
        0,
        &mut ctx,
        &runtime,
    );
    assert_sink_error("~<pre~;body~;post~:>", &[Word::NIL], 4, &mut ctx, &runtime);
    assert_sink_error("~<pre~;body~;post~:>", &[Word::NIL], 7, &mut ctx, &runtime);
    assert_sink_error("~@<P>~@;word next~:@>", &[], 2, &mut ctx, &runtime);
    assert_sink_error("~@<P>~@;word~:@>", &[], 2, &mut ctx, &runtime);
    assert_sink_error("~@<P>~@;a b~:@>", &[], 3, &mut ctx, &runtime);
    let first = "a".repeat(75);
    let second = "b".repeat(10);
    let control = format!("~@<P>~@;{first} {second}~:@>");
    assert_sink_error(&control, &[], 2 + first.len(), &mut ctx, &runtime);
    assert_sink_error(
        "~@<P>~@;~%~A~:@>",
        &[Word::fixnum(7)],
        3,
        &mut ctx,
        &runtime,
    );
    assert_sink_error("~@<word next~:@>", &[], 0, &mut ctx, &runtime);
    assert_sink_error("~@<word~:@>", &[], 0, &mut ctx, &runtime);
    assert_sink_error("~@<a b~:@>", &[], 1, &mut ctx, &runtime);
    let control = format!("~@<{first} {second}~:@>");
    assert_sink_error(&control, &[], first.len(), &mut ctx, &runtime);
}

#[test] // CLHS 22.3.5.1: ~:@_ is a mandatory newline when pretty printing is enabled.
fn underscore_break_reports_sink_failure() {
    let (runtime, mut ctx) = context();
    enable_pretty(&mut ctx, &runtime);
    assert_sink_error("a~:@_b", &[], 1, &mut ctx, &runtime);
}

#[test] // CLHS 22.3.5.4: named format functions consume dynamic parameters and a value.
fn named_formatter_reports_missing_dynamic_parameters() {
    let (runtime, mut ctx) = context();
    register_formatter(&mut ctx, &runtime);
    assert_format_error(
        "~V/NCL-TEST::FORMATTER/",
        &[],
        &mut ctx,
        &runtime,
        &FormatErrorKind::MissingArgument,
    );
    assert_format_error(
        "~/NCL-TEST::FORMATTER/",
        &[],
        &mut ctx,
        &runtime,
        &FormatErrorKind::MissingArgument,
    );
    for control in ["~//", "~/:FORMATTER/", "~/NCL-TEST:/"] {
        assert_format_error(
            control,
            &[Word::fixnum(7)],
            &mut ctx,
            &runtime,
            &FormatErrorKind::InvalidDirectiveUse,
        );
    }
}

#[test] // CLHS 22.3.5.4: named functions write through the supplied string output stream.
fn named_formatter_reports_missing_string_stream_support() {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("context");
    register_formatter(&mut ctx, &runtime);
    assert_format_error(
        "~/NCL-TEST::FORMATTER/",
        &[Word::fixnum(7)],
        &mut ctx,
        &runtime,
        &FormatErrorKind::InvalidDirectiveUse,
    );
}
