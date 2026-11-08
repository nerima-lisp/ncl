//! Public formatting behavior for layout directives and named functions.

#![expect(
    clippy::expect_used,
    reason = "Test setup uses expect to keep fixture failures local."
)]

use ncl_lib_format::{FormatErrorKind, execute, parse};
use ncl_packages::PackageOperations;
use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, FunctionObject, LambdaList, MultipleValues, ObjectError, Package,
    Parameter as BuiltinParameter, ParameterType, Runtime, ThreadContext, Word, make_string,
    set_symbol_special, set_symbol_value,
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

const NAMED_FORMATTER_PARAMETERS: &[BuiltinParameter] = &[
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

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("context");
    ncl_lib_streams::register(&runtime).expect("streams");
    (runtime, ctx)
}

fn set_pretty(ctx: &mut ThreadContext, runtime: &Runtime, pretty: bool) {
    let package = runtime
        .find_package(ctx, "COMMON-LISP")
        .expect("COMMON-LISP");
    let (symbol, _) = Package::from_word(package)
        .intern(ctx, runtime, "*PRINT-PRETTY*")
        .expect("intern *PRINT-PRETTY*");
    set_symbol_special(ctx, symbol, true).expect("mark special");
    set_symbol_value(ctx, symbol, if pretty { Word::TRUE } else { Word::NIL })
        .expect("set pretty mode");
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

fn execute_text(
    control: &str,
    arguments: &[Word],
    ctx: &mut ThreadContext,
    runtime: &Runtime,
) -> String {
    let parsed = parse(control).expect("format control");
    let mut sink = StringSink::new();
    execute(&parsed, arguments, ctx, runtime, &mut sink).expect("execute");
    sink.into_string()
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

fn argument_writer(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    arguments: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = arguments.required(0)?;
    let value = arguments.required(1)?;
    let colon = arguments.required(2)? != Word::NIL;
    let at_sign = arguments.required(3)? != Word::NIL;
    let write_string = runtime
        .function(ctx, "COMMON-LISP", "WRITE-STRING")
        .and_then(|word| FunctionObject::try_from(word).ok())
        .ok_or(ObjectError::UndefinedFunction)?;
    runtime.call_builtin(ctx, write_string, &[value, stream])?;
    if colon || at_sign {
        let markers = format!(
            "{}{}",
            if colon { ":" } else { "" },
            if at_sign { "@" } else { "" }
        );
        let markers = make_string(ctx, runtime, &markers.chars().collect::<Vec<_>>())?;
        runtime.call_builtin(ctx, write_string, &[markers, stream])?;
    }
    Ok(Word::fixnum(999))
}

fn register_argument_writer(ctx: &mut ThreadContext, runtime: &Runtime) {
    let descriptor = Builtin {
        lambda_list: LambdaList::fixed(NAMED_FORMATTER_PARAMETERS),
        convention: BuiltinConvention::Direct(Arity::exact(4)),
    };
    runtime
        .register_builtin(
            ctx,
            BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("ARGUMENT-WRITER")),
            BuiltinImplementation::direct(descriptor, argument_writer),
        )
        .expect("register formatter");
}

#[test]
fn non_pretty_indent_does_not_apply_negative_adjustments() {
    let (runtime, mut ctx) = context();
    set_pretty(&mut ctx, &runtime, false);
    // CLHS 22.3.5.3 maps ~nI to PPRINT-INDENT, which has no effect on a non-pretty stream.
    assert_eq!(execute_text("a~-1I~%x", &[], &mut ctx, &runtime), "a\nx");
    // CLHS 22.3.5.3 defines :current indentation; it adds no spaces when pretty is disabled.
    assert_eq!(execute_text("a~-2:I~%x", &[], &mut ctx, &runtime), "a\nx");
}

#[test]
fn current_indent_can_move_left_without_crossing_the_line_start() {
    let (runtime, mut ctx) = context();
    set_pretty(&mut ctx, &runtime, true);
    // CLHS 22.3.5.3 maps ~n:I to :current indentation; negative n reduces the current column.
    assert_eq!(
        execute_text("abcd~-2:I~%x", &[], &mut ctx, &runtime),
        "abcd\n  x"
    );
}

#[test]
fn current_indent_clamps_negative_adjustment_at_the_line_start() {
    let (runtime, mut ctx) = context();
    set_pretty(&mut ctx, &runtime, true);
    // CLHS 22.3.5.3 maps ~n:I to :current indentation; CLHS 22.4.6 clamps it at line start.
    assert_eq!(
        execute_text("~<P>~;abcd~-7:I~%x~:>", &[Word::NIL], &mut ctx, &runtime),
        "P>abcd\nx"
    );
}

#[test] // CLHS 22.3.5.3 and 22.4.6: block-relative negative indentation clamps at line start.
fn block_indent_clamps_negative_adjustment_at_the_line_start() {
    let (runtime, mut ctx) = context();
    set_pretty(&mut ctx, &runtime, true);
    assert_eq!(
        execute_text("~<P>~;abcd~-7I~%x~:>", &[Word::NIL], &mut ctx, &runtime),
        "P>abcd\nx"
    );
}

#[test]
fn indent_is_ignored_when_the_logical_block_is_in_miser_mode() {
    let (runtime, mut ctx) = context();
    set_pretty(&mut ctx, &runtime, true);
    set_miser_width(&mut ctx, &runtime, Word::fixnum(80));
    // CLHS 22.3.5.3 maps ~n:I to PPRINT-INDENT; CLHS 22.4.6 ignores it in miser mode.
    assert_eq!(
        execute_text("~@<P>~@;~2:I~%x~:>", &[], &mut ctx, &runtime),
        "P>\nP>x"
    );
}

#[test]
fn linear_conditional_newline_keeps_short_text_on_one_line() {
    let (runtime, mut ctx) = context();
    set_pretty(&mut ctx, &runtime, true);
    // CLHS 22.3.5.1 defines ~_ as a linear conditional newline, so a short line stays unbroken.
    assert_eq!(
        execute_text("short~_tail", &[], &mut ctx, &runtime),
        "shorttail"
    );
}

#[test]
fn logical_block_reports_a_missing_list_before_rendering_its_body() {
    let (runtime, mut ctx) = context();
    // CLHS 22.3.5.2 treats the directive's argument as the logical block's list argument.
    assert_format_error(
        "~<body~:>",
        &[],
        &mut ctx,
        &runtime,
        &FormatErrorKind::MissingArgument,
    );
}

#[test]
fn three_clause_logical_block_rejects_a_directive_in_its_prefix() {
    let (runtime, mut ctx) = context();
    // CLHS 22.3.5.2 requires prefix and suffix clauses to be constant strings.
    assert_format_error(
        "~<pre~A~;body~;post~:>",
        &[Word::NIL],
        &mut ctx,
        &runtime,
        &FormatErrorKind::InvalidDirectiveUse,
    );
}

#[test]
fn justification_reports_a_missing_dynamic_width_parameter() {
    let (runtime, mut ctx) = context();
    // CLHS 22.3.6.2 uses the first parameter as the minimum field width.
    assert_format_error(
        "~V<text~>",
        &[],
        &mut ctx,
        &runtime,
        &FormatErrorKind::MissingArgument,
    );
    // CLHS 22.3.6.2 also accepts a dynamic width before the special-prefix clause.
    assert_format_error(
        "~10<pre~2,V:;body~>",
        &[],
        &mut ctx,
        &runtime,
        &FormatErrorKind::MissingArgument,
    );
}

#[test]
fn tabulation_honors_zero_increments_alignment_and_dynamic_parameters() {
    let (runtime, mut ctx) = context();
    for (control, arguments, expected) in [
        // CLHS 22.3.6.1 advances to colnum when the current column is before it.
        ("~3,0Tz", vec![], "   z"),
        // CLHS 22.3.6.1 emits no spaces if colinc is zero at or beyond colnum.
        ("abcd~3,0Tz", vec![], "abcdz"),
        // CLHS 22.3.6.1 advances to the next colnum + k*colinc stop.
        ("abc~3,4Tz", vec![], "abc    z"),
        // CLHS 22.3.6.1 uses the next absolute tab stop after the current column.
        ("abcde~3,4Tz", vec![], "abcde  z"),
        // CLHS 22.3.6.1 relative tabulation emits colrel spaces at an aligned column.
        ("abc~1,4@Tz", vec![], "abc z"),
        // CLHS 22.3.6.1 adds spaces to reach a multiple of colinc after colrel spaces.
        ("abc~2,4@Tz", vec![], "abc     z"),
        // CLHS 22.3.6.1 permits colnum to be supplied by a dynamic parameter.
        ("~V,4Tz", vec![Word::fixnum(2)], "  z"),
    ] {
        assert_eq!(
            execute_text(control, &arguments, &mut ctx, &runtime),
            expected,
            "{control}"
        );
    }
}

#[test]
fn two_clause_colon_logical_block_keeps_prefix_and_closing_parenthesis() {
    let (runtime, mut ctx) = context();
    // CLHS 22.3.5.2 keeps the supplied prefix and defaults the colon form's suffix to ")".
    assert_eq!(
        execute_text("~:<pre~;body~:>", &[Word::NIL], &mut ctx, &runtime),
        "prebody)"
    );
}

#[test]
fn two_clause_logical_block_formats_list_elements_after_its_prefix() {
    let (runtime, mut ctx) = context();
    let item = make_string(&mut ctx, &runtime, &['i', 't', 'e', 'm']).expect("item");
    let list = ncl_object::make_cons(&mut ctx, &runtime, item, Word::NIL).expect("list");
    // CLHS 22.3.5.2 uses the first clause as a prefix and the second as the body.
    assert_eq!(
        execute_text("~<pre~;~A~:>", &[list], &mut ctx, &runtime),
        "preitem"
    );
}

#[test]
fn per_line_prefix_repeats_across_literal_newlines_before_the_suffix() {
    let (runtime, mut ctx) = context();
    set_pretty(&mut ctx, &runtime, true);
    // CLHS 22.3.5.2 makes a prefix clause ended by ~@; a per-line prefix.
    assert_eq!(
        execute_text("~<P>~@;one\ntwo~;END~:>", &[Word::NIL], &mut ctx, &runtime),
        "P>one\nP>twoEND"
    );
}

#[test]
fn per_line_prefix_literal_sink_failure_keeps_the_typed_error() {
    let (runtime, mut ctx) = context();
    set_pretty(&mut ctx, &runtime, true);
    let control = parse("~<P>~@;one\ntwo~;END~:>").expect("format control");
    let mut sink = FailingSink { remaining: 6 };
    // CLHS 22.3.5.2 defines ~@; as a per-line prefix for the logical-block body.
    let error = execute(&control, &[Word::NIL], &mut ctx, &runtime, &mut sink)
        .expect_err("per-line prefix sink failure");
    assert_eq!(
        error.kind,
        FormatErrorKind::Print(PrintError::Sink("closed".to_owned()))
    );
}

#[test]
fn justification_discards_a_special_prefix_when_the_field_fits() {
    let (runtime, mut ctx) = context();
    for (control, expected) in [
        // CLHS 22.3.6.2 discards the first clause when the padded field fits with spare room.
        ("~5<pre~1,6:;x~>", "    x"),
        // CLHS 22.3.6.2 outputs the first clause when the padded field cannot fit with spare room.
        ("~5<pre~1,5:;x~>", "pre    x"),
    ] {
        assert_eq!(
            execute_text(control, &[], &mut ctx, &runtime),
            expected,
            "{control}"
        );
    }
}

#[test]
fn justification_distributes_minimum_nonspace_padding_between_all_clauses() {
    let (runtime, mut ctx) = context();
    // CLHS 22.3.6.2 applies minpad between every segment and uses padchar for that spacing.
    assert_eq!(
        execute_text("~12,1,1,'-<a~;b~;c~>", &[], &mut ctx, &runtime),
        "a-----b----c"
    );
}

#[test]
fn justification_uses_72_columns_when_string_width_is_unknown() {
    let (runtime, mut ctx) = context();
    let prefix = "x".repeat(73);
    let control = format!("{prefix}~<~%pre~:;body~>");
    // CLHS 22.3.6.2 specifies a 72-column fallback when the destination width is unknown.
    assert_eq!(
        execute_text(&control, &[], &mut ctx, &runtime),
        format!("{prefix}\nprebody")
    );
}

#[test]
fn named_formatter_rejects_an_interned_but_unbound_function_name() {
    let (runtime, mut ctx) = context();
    register_argument_writer(&mut ctx, &runtime);
    let package = runtime.find_package(&ctx, "NCL-TEST").expect("NCL-TEST");
    Package::from_word(package)
        .intern(&mut ctx, &runtime, "UNBOUND-FORMATTER")
        .expect("intern function name");
    // CLHS 22.3.5.4 looks up the named function in the indicated package before calling it.
    assert_format_error(
        "~/NCL-TEST::UNBOUND-FORMATTER/",
        &[Word::fixnum(7)],
        &mut ctx,
        &runtime,
        &FormatErrorKind::InvalidDirectiveUse,
    );
}

#[test]
fn named_formatter_ignores_function_return_values() {
    let (runtime, mut ctx) = context();
    register_argument_writer(&mut ctx, &runtime);
    let text = make_string(&mut ctx, &runtime, &['o', 'u', 't']).expect("text");
    // CLHS 22.3.5.4 says the function prints to the supplied stream and its return value is ignored.
    assert_eq!(
        execute_text("~/NCL-TEST::ARGUMENT-WRITER/", &[text], &mut ctx, &runtime,),
        "out"
    );
}

#[test]
fn named_formatter_resolves_exported_package_symbols() {
    let (runtime, mut ctx) = context();
    register_argument_writer(&mut ctx, &runtime);
    let package = runtime.find_package(&ctx, "NCL-TEST").expect("NCL-TEST");
    let name = make_string(
        &mut ctx,
        &runtime,
        &"ARGUMENT-WRITER".chars().collect::<Vec<_>>(),
    )
    .expect("formatter name");
    Package::from_word(package)
        .export(&mut ctx, &runtime, name)
        .expect("export formatter");
    let text = make_string(&mut ctx, &runtime, &['o', 'u', 't']).expect("text");
    // CLHS 22.3.5.4 uses one package colon for external lookup and passes both modifiers to the function.
    assert_eq!(
        execute_text("~:@/NCL-TEST:ARGUMENT-WRITER/", &[text], &mut ctx, &runtime),
        "out:@"
    );
}

#[test]
fn unqualified_named_formatter_uses_common_lisp_user_and_uppercases_its_name() {
    let (runtime, mut ctx) = context();
    register_argument_writer(&mut ctx, &runtime);
    let formatter_package = runtime.find_package(&ctx, "NCL-TEST").expect("NCL-TEST");
    let user_package = runtime
        .find_package(&ctx, "COMMON-LISP-USER")
        .expect("COMMON-LISP-USER");
    let mut name = make_string(
        &mut ctx,
        &runtime,
        &"ARGUMENT-WRITER".chars().collect::<Vec<_>>(),
    )
    .expect("formatter name");
    ncl_object::with_root(&mut ctx, &mut name, |ctx, name| {
        let (symbol, _) =
            Package::from_word(formatter_package).intern(ctx, &runtime, "ARGUMENT-WRITER")?;
        Package::from_word(user_package).import(ctx, &runtime, *name, symbol)
    })
    .expect("import formatter into COMMON-LISP-USER");
    let text = make_string(&mut ctx, &runtime, &['o', 'u', 't']).expect("text");
    // CLHS 22.3.5.4 looks up unqualified names in COMMON-LISP-USER and treats name characters as uppercase.
    assert_eq!(
        execute_text("~/argument-writer/", &[text], &mut ctx, &runtime),
        "out"
    );
}
