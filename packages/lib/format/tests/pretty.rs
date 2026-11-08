//! Expected FORMAT output for pretty and layout directives.

#![expect(
    clippy::expect_used,
    reason = "Test setup uses expect and unwrap to keep fixture failures local."
)]

use ncl_lib_format::{FormatErrorKind, execute, parse};
use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, FunctionObject, LambdaList, MultipleValues, ObjectError,
    ObjectRef, Package, Parameter as BuiltinParameter, ParameterType, Runtime, ThreadContext, Word,
    classify_object, make_cons, make_string, set_symbol_special, set_symbol_value,
};
use ncl_printer::StringSink;

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
    BuiltinParameter {
        name: BuiltinName::new("PARAMETER"),
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

fn string(runtime: &Runtime, ctx: &mut ThreadContext, text: &str) -> Word {
    make_string(ctx, runtime, &text.chars().collect::<Vec<_>>()).expect("string")
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
    let error = execute(&parsed, arguments, ctx, runtime, &mut sink).expect_err("format error");
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

#[test] // CLHS 22.3.5.1: conditional newline modes.
fn underscore_modes_distinguish_linear_fill_miser_and_mandatory_breaks() {
    let (runtime, mut ctx) = context();
    assert_eq!(execute_text("a~:@_b", &[], &mut ctx, &runtime).0, "ab");
    enable_pretty(&mut ctx, &runtime);
    assert_eq!(execute_text("a~:@_b", &[], &mut ctx, &runtime).0, "a\nb");
    assert_eq!(execute_text("~_tail", &[], &mut ctx, &runtime).0, "tail");
    let exact_margin = "x".repeat(80);
    assert_eq!(
        execute_text(&format!("{exact_margin}~_tail"), &[], &mut ctx, &runtime).0,
        format!("{exact_margin}\ntail")
    );
    assert_eq!(
        execute_text(&format!("{exact_margin}~:_tail"), &[], &mut ctx, &runtime).0,
        format!("{exact_margin}\ntail")
    );
    let over_margin = "x".repeat(81);
    assert_eq!(
        execute_text(&format!("{over_margin}~_tail"), &[], &mut ctx, &runtime).0,
        format!("{over_margin}\ntail")
    );
    assert_eq!(
        execute_text(&format!("{over_margin}~@_tail"), &[], &mut ctx, &runtime).0,
        format!("{over_margin}tail")
    );
    let block_prefix = "p".repeat(40);
    let block_content = "x".repeat(41);
    set_miser_width(&mut ctx, &runtime, Word::NIL);
    assert_eq!(
        execute_text(
            &format!("~@<{block_prefix}~;{block_content}~@_tail~:>"),
            &[],
            &mut ctx,
            &runtime,
        )
        .0,
        format!("{block_prefix}{block_content}tail")
    );
    set_miser_width(&mut ctx, &runtime, Word::fixnum(39));
    assert_eq!(
        execute_text(
            &format!("~@<{block_prefix}~;{block_content}~@_tail~:>"),
            &[],
            &mut ctx,
            &runtime,
        )
        .0,
        format!("{block_prefix}{block_content}tail")
    );
    set_miser_width(&mut ctx, &runtime, Word::fixnum(40));
    assert_eq!(
        execute_text(
            &format!("~@<{block_prefix}~;{block_content}~@_tail~:>"),
            &[],
            &mut ctx,
            &runtime,
        )
        .0,
        format!("{block_prefix}{block_content}\n{}tail", " ".repeat(40))
    );
}

#[test] // CLHS 22.3.5.3: block and current indentation.
fn indent_uses_block_or_current_column_and_affects_the_next_line() {
    let (runtime, mut ctx) = context();
    enable_pretty(&mut ctx, &runtime);
    assert_eq!(
        execute_text("ab~2I~%x", &[], &mut ctx, &runtime).0,
        "ab\n  x"
    );
    assert_eq!(
        execute_text("ab~2:I~%x", &[], &mut ctx, &runtime).0,
        "ab\n    x"
    );
}

#[test] // CLHS 22.3.6.1: absolute, relative, and section-relative tabulation.
fn tab_supports_absolute_relative_and_block_relative_stops() {
    let (runtime, mut ctx) = context();
    assert_eq!(execute_text("a~3Tz", &[], &mut ctx, &runtime).0, "a  z");
    assert_eq!(
        execute_text("abcdef~3Tz", &[], &mut ctx, &runtime).0,
        "abcdef z"
    );
    assert_eq!(
        execute_text("abc~3,4@Tz", &[], &mut ctx, &runtime).0,
        "abc     z"
    );
    assert_eq!(
        execute_text("~<ab~;~2:Tz~:>", &[Word::NIL], &mut ctx, &runtime).0,
        "ab  z"
    );
    assert_eq!(
        execute_text("~<ab~;~3,4:@Tz~:>", &[Word::NIL], &mut ctx, &runtime).0,
        "ab    z"
    );
}

#[test] // CLHS 22.3.5.2: logical-block prefix, body, suffix, and default parentheses.
fn logical_blocks_apply_prefix_body_suffix_and_default_parentheses() {
    let (runtime, mut ctx) = context();
    let value = string(&runtime, &mut ctx, "item");
    let list = make_cons(&mut ctx, &runtime, value, Word::NIL).expect("list");
    assert_eq!(
        execute_text("~:<~A~:>", &[list], &mut ctx, &runtime),
        ("(item)".into(), 1)
    );
    assert_eq!(
        execute_text("~:<pre~;~A~;post~:>", &[list], &mut ctx, &runtime).0,
        "preitempost"
    );
}

#[test] // CLHS 22.3.5.2: an @-terminated first separator defines a per-line prefix.
fn logical_block_at_sign_separator_repeats_prefix_after_each_line_break() {
    let (runtime, mut ctx) = context();
    enable_pretty(&mut ctx, &runtime);
    let first = string(&runtime, &mut ctx, "ab");
    let second = string(&runtime, &mut ctx, "ef");
    let tail = make_cons(&mut ctx, &runtime, second, Word::NIL).expect("tail");
    let list = make_cons(&mut ctx, &runtime, first, tail).expect("list");
    assert_eq!(
        execute_text("~<pre~@;~A~:@_~A~:>", &[list], &mut ctx, &runtime).0,
        "preab\npreef"
    );

    let first = string(&runtime, &mut ctx, "ab\ncd");
    let second = string(&runtime, &mut ctx, "ef");
    let tail = make_cons(&mut ctx, &runtime, second, Word::NIL).expect("tail");
    let list = make_cons(&mut ctx, &runtime, first, tail).expect("list");
    assert_eq!(
        execute_text("~<pre~@;~A~3,4T~A~:>", &[list], &mut ctx, &runtime).0,
        "preab\nprecd  ef"
    );
}

#[test] // CLHS 22.3.5.2: the outer @ modifier passes all remaining arguments.
fn at_sign_logical_block_consumes_all_remaining_arguments() {
    let (runtime, mut ctx) = context();
    assert_eq!(
        execute_text(
            "~@<~A~:>",
            &[Word::fixnum(7), Word::fixnum(8)],
            &mut ctx,
            &runtime,
        ),
        ("7".into(), 2)
    );
}

#[test] // CLHS 22.3.5.2: ~^ exits a logical-block body while preserving its suffix.
fn logical_block_escape_stops_the_body_and_keeps_the_suffix() {
    let (runtime, mut ctx) = context();
    let value = string(&runtime, &mut ctx, "item");
    let list = make_cons(&mut ctx, &runtime, value, Word::NIL).expect("list");
    assert_eq!(
        execute_text("~:<~A~^ignored~:>", &[list], &mut ctx, &runtime).0,
        "(item)"
    );
}

#[test] // CLHS 22.3.5.2: logical-block escape behavior is distinct from iteration.
fn logical_block_does_not_make_colon_up_arrow_valid() {
    let (runtime, mut ctx) = context();
    let value = string(&runtime, &mut ctx, "item");
    let list = make_cons(&mut ctx, &runtime, value, Word::NIL).expect("list");
    let control = parse("~:<~A~:^ignored~:>").expect("format control");
    let mut sink = StringSink::new();
    let error = execute(&control, &[list], &mut ctx, &runtime, &mut sink)
        .expect_err("logical block is not a sublist iteration");
    assert_eq!(error.kind, FormatErrorKind::InvalidDirectiveUse);
}

#[test] // CLHS 22.3.5.2: the @ terminator enables fill-style line breaking.
fn logical_block_fill_terminator_breaks_at_spaces_past_margin() {
    let (runtime, mut ctx) = context();
    enable_pretty(&mut ctx, &runtime);
    let word = "x".repeat(80);
    let control = format!("~@<a {word}~:@>");
    assert_eq!(
        execute_text(&control, &[], &mut ctx, &runtime).0,
        format!("a\n{word}")
    );
}

#[test] // CLHS 22.3.6.2: justification alignment and minimum inter-segment padding.
fn justification_supports_alignment_padding_and_minimum_internal_padding() {
    let (runtime, mut ctx) = context();
    let value = string(&runtime, &mut ctx, "x");
    assert_eq!(
        execute_text("~8<~A~>", &[value], &mut ctx, &runtime).0,
        "       x"
    );
    assert_eq!(
        execute_text("~8@<~A~>", &[value], &mut ctx, &runtime).0,
        "x       "
    );
    assert_eq!(
        execute_text("~8:<~A~>", &[value], &mut ctx, &runtime).0,
        "       x"
    );
    // CLHS 22.3.6.2 leaves seven spaces before x for this empty first segment.
    assert_eq!(
        execute_text("~8<~;x~>", &[], &mut ctx, &runtime).0,
        "       x"
    );
    assert_eq!(
        execute_text("~8:@<~A~>", &[value], &mut ctx, &runtime).0,
        "    x   "
    );
    let left = string(&runtime, &mut ctx, "a");
    let right = string(&runtime, &mut ctx, "b");
    assert_eq!(
        execute_text("~12,2,2,'-<~A~;~A~>", &[left, right], &mut ctx, &runtime).0,
        "a----------b"
    );
}

#[test] // CLHS 22.3.6.2: only completed clauses are justified after ~^.
fn justification_escape_keeps_prefix_text_and_justifies_it() {
    let (runtime, mut ctx) = context();
    assert_eq!(
        execute_text("~10<pre~^ignored~:;body~>", &[], &mut ctx, &runtime).0,
        "       pre"
    );
}

#[test] // CLHS 22.3.6.2: a special prefix is emitted when the justified field will not fit.
fn justification_prefix_is_emitted_on_a_wrapped_line() {
    let (runtime, mut ctx) = context();
    let control = format!("{}~5<~%pre~:;body~>", "x".repeat(78));
    assert_eq!(
        execute_text(&control, &[], &mut ctx, &runtime).0,
        format!("{}\npre body", "x".repeat(78))
    );
}

fn formatter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    arguments: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = arguments.required(0)?;
    let value = arguments.required(1)?;
    let colon = arguments.required(2)? != Word::NIL;
    let at_sign = arguments.required(3)? != Word::NIL;
    let parameter_word = arguments.required(4)?;
    let parameter = if parameter_word == Word::NIL {
        "nil".to_owned()
    } else {
        match classify_object(ctx, parameter_word) {
            ObjectRef::Fixnum(value) => value.to_string(),
            ObjectRef::Character(value) => char::from_u32(value)
                .map(|character| character.to_string())
                .ok_or(ObjectError::TypeError)?,
            ObjectRef::Bignum(_) => "bignum".to_owned(),
            _ => return Err(ObjectError::TypeError),
        }
    };
    let write_string = runtime
        .function(ctx, "COMMON-LISP", "WRITE-STRING")
        .and_then(|word| FunctionObject::try_from(word).ok())
        .ok_or(ObjectError::UndefinedFunction)?;
    runtime.call_builtin(ctx, write_string, &[value, stream])?;
    let marker = format!(
        "{}{}{}",
        if colon { ":" } else { "" },
        if at_sign { "@" } else { "" },
        parameter
    );
    let marker = make_string(ctx, runtime, &marker.chars().collect::<Vec<_>>())?;
    runtime.call_builtin(ctx, write_string, &[marker, stream])?;
    Ok(value)
}

fn register_formatter(ctx: &mut ThreadContext, runtime: &Runtime) {
    let descriptor = Builtin {
        lambda_list: LambdaList::fixed(FORMATTER_PARAMETERS),
        convention: BuiltinConvention::Direct(Arity::exact(5)),
    };
    runtime
        .register_builtin(
            ctx,
            BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("FORMATTER")),
            BuiltinImplementation::direct(descriptor, formatter),
        )
        .expect("register formatter");
}

#[test] // CLHS 22.3.5.4: named functions receive the stream, value, modifiers, and parameters.
fn named_formatter_receives_stream_value_modifiers_and_parameters() {
    let (runtime, mut ctx) = context();
    register_formatter(&mut ctx, &runtime);
    let value = string(&runtime, &mut ctx, "x");
    assert_eq!(
        execute_text("~'!:@/NCL-TEST::FORMATTER/", &[value], &mut ctx, &runtime),
        ("x:@!".into(), 1)
    );
}

#[test] // CLHS 22.3.5.4: named functions receive resolved parameters and remaining-argument count.
fn named_formatter_receives_resolved_parameters_and_remaining_arguments() {
    let (runtime, mut ctx) = context();
    register_formatter(&mut ctx, &runtime);
    let value = string(&runtime, &mut ctx, "x");
    for (control, arguments, expected) in [
        ("~4:@/NCL-TEST::FORMATTER/", vec![value], "x:@4"),
        (
            "~V@/NCL-TEST::FORMATTER/",
            vec![Word::fixnum(6), value],
            "x@6",
        ),
        (
            "~V@/NCL-TEST::FORMATTER/",
            vec![Word::character(u32::from('!')), value],
            "x@!",
        ),
        ("~,/NCL-TEST::FORMATTER/", vec![value], "xnil"),
        ("~#/NCL-TEST::FORMATTER/~A", vec![value, value], "x2x"),
        (
            "~4611686018427387904/NCL-TEST::FORMATTER/",
            vec![value],
            "xbignum",
        ),
    ] {
        assert_eq!(
            execute_text(control, &arguments, &mut ctx, &runtime).0,
            expected,
            "{control}"
        );
    }
}

#[test] // CLHS 22.3.5.4: named formatter lookup requires an accessible function.
fn named_formatter_lookup_errors_are_typed() {
    let (runtime, mut ctx) = context();
    register_formatter(&mut ctx, &runtime);
    let value = string(&runtime, &mut ctx, "x");
    for control in [
        "~/NCL-TEST::MISSING/",
        "~/NCL-TEST:FORMATTER/",
        "~/NO-SUCH-PACKAGE::FORMATTER/",
    ] {
        assert_format_error(
            control,
            &[value],
            &mut ctx,
            &runtime,
            &FormatErrorKind::InvalidDirectiveUse,
        );
    }
}
