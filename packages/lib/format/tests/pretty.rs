//! Expected FORMAT output for pretty and layout directives.

#![expect(
    clippy::expect_used,
    reason = "Test setup uses expect and unwrap to keep fixture failures local."
)]

use ncl_lib_format::{FormatErrorKind, execute, parse};
use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, FunctionObject, LambdaList, MultipleValues, ObjectError, Package,
    Parameter as BuiltinParameter, ParameterType, Runtime, ThreadContext, Word, make_cons,
    make_string, set_symbol_special, set_symbol_value,
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

#[test]
fn underscore_modes_distinguish_linear_fill_miser_and_mandatory_breaks() {
    let (runtime, mut ctx) = context();
    assert_eq!(execute_text("a~:@_b", &[], &mut ctx, &runtime).0, "ab");
    enable_pretty(&mut ctx, &runtime);
    assert_eq!(execute_text("a~:@_b", &[], &mut ctx, &runtime).0, "a\nb");
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

#[test]
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

#[test]
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

#[test]
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

#[test]
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

#[test]
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

#[test]
fn logical_block_escape_stops_the_body_and_keeps_the_suffix() {
    let (runtime, mut ctx) = context();
    let value = string(&runtime, &mut ctx, "item");
    let list = make_cons(&mut ctx, &runtime, value, Word::NIL).expect("list");
    assert_eq!(
        execute_text("~:<~A~^ignored~:>", &[list], &mut ctx, &runtime).0,
        "(item)"
    );
}

#[test]
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

#[test]
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

#[test]
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

#[test]
fn justification_escape_keeps_prefix_text_and_justifies_it() {
    let (runtime, mut ctx) = context();
    assert_eq!(
        execute_text("~10<pre~^ignored~:;body~>", &[], &mut ctx, &runtime).0,
        "       pre"
    );
}

#[test]
fn justification_prefix_is_emitted_on_a_wrapped_line() {
    let (runtime, mut ctx) = context();
    let control = format!("{}~5<pre~:;body~>", "x".repeat(78));
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
    let parameter = arguments
        .required(4)?
        .as_character()
        .and_then(char::from_u32)
        .ok_or(ObjectError::TypeError)?;
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

#[test]
fn named_formatter_receives_stream_value_modifiers_and_parameters() {
    let (runtime, mut ctx) = context();
    let descriptor = Builtin {
        lambda_list: LambdaList::fixed(FORMATTER_PARAMETERS),
        convention: BuiltinConvention::Direct(Arity::exact(5)),
    };
    runtime
        .register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("FORMATTER")),
            BuiltinImplementation::direct(descriptor, formatter),
        )
        .expect("register formatter");
    let value = string(&runtime, &mut ctx, "x");
    assert_eq!(
        execute_text("~'!:@/NCL-TEST::FORMATTER/", &[value], &mut ctx, &runtime),
        ("x:@!".into(), 1)
    );
}

#[test]
fn named_formatter_lookup_errors_are_typed() {
    let (runtime, mut ctx) = context();
    let value = string(&runtime, &mut ctx, "x");
    let control = parse("~/NCL-TEST::MISSING/").expect("format control");
    let mut sink = StringSink::new();
    let error =
        execute(&control, &[value], &mut ctx, &runtime, &mut sink).expect_err("missing formatter");
    assert_eq!(error.kind, FormatErrorKind::InvalidDirectiveUse);
}

#[test]
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
}
