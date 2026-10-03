#![allow(clippy::unwrap_used, reason = "tests assert on reader behavior")]

use std::cell::Cell;
use std::fmt::Debug;

use ncl_object::{Runtime, ThreadContext, Word, string_length, string_ref, symbol_name};

use crate::dispatch::read_sharp;
use crate::features::eval_feature_expr;
use crate::input::StringSource;
use crate::number::{digit_value, parse_integer_chars, parse_number};
use crate::reader::{FloatFormat, PackageName, ReadBase, ReadEvaluation, ReadSuppression};
use crate::readtable::{
    CustomMacroKind, ReadtableCase, SyntaxKind, readtable_from_word, syntax_kind,
};
use crate::{ReadError, ReadOptions, read_from_string};

trait Required<T> {
    fn required(self) -> T;
}

impl<T, E: Debug> Required<T> for Result<T, E> {
    fn required(self) -> T {
        self.unwrap_or_else(|error| unreachable!("unexpected error: {error:?}"))
    }
}

impl<T> Required<T> for Option<T> {
    fn required(self) -> T {
        self.unwrap_or_else(|| unreachable!("unexpected None"))
    }
}

fn setup() -> (Runtime, ThreadContext, ReadOptions) {
    let runtime = Runtime::new().required();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).required();
    let options = ReadOptions::standard(&mut ctx, &runtime).required();
    (runtime, ctx, options)
}

fn read_one(runtime: &Runtime, ctx: &mut ThreadContext, text: &str, options: &ReadOptions) -> Word {
    read_from_string(ctx, runtime, text, options)
        .required()
        .required()
}

fn symbol_text(ctx: &ThreadContext, symbol: Word) -> String {
    let name = symbol_name(ctx, symbol).required();
    let length = string_length(ctx, name).required();
    (0..length)
        .map(|index| string_ref(ctx, name, index).required())
        .collect()
}

#[test]
fn options_and_small_value_types_report_state_and_errors() {
    assert_eq!(ReadBase::new(2).required().value(), 2);
    assert_eq!(ReadBase::new(36).required().value(), 36);
    assert_eq!(ReadBase::new(1), Err(ReadError::InvalidBase(1)));
    assert_eq!(ReadBase::new(37), Err(ReadError::InvalidBase(37)));
    assert_eq!(PackageName::new("PKG").required().as_str(), "PKG");
    assert_eq!(
        PackageName::new(""),
        Err(ReadError::InvalidSymbolToken(String::new()))
    );

    let (runtime, mut ctx, mut options) = setup();
    runtime.ensure_package(&mut ctx, "PKG").required();
    let table = options.readtable();
    options.set_read_base(ReadBase::new(16).required());
    options.set_read_evaluation(ReadEvaluation::Disabled);
    options.set_read_suppression(ReadSuppression::Keep);
    options.set_default_float_format(FloatFormat::SingleFloat);
    options.set_readtable(table);
    options.set_current_package("PKG").required();
    assert_eq!(options.read_base().value(), 16);
    assert_eq!(options.read_evaluation(), ReadEvaluation::Disabled);
    assert_eq!(options.read_suppression(), ReadSuppression::Keep);
    assert_eq!(options.default_float_format(), FloatFormat::SingleFloat);
    assert_eq!(options.current_package().required().as_str(), "PKG");
    let word = read_one(&runtime, &mut ctx, "G", &options);
    assert_eq!(symbol_text(&ctx, word), "G");
}

#[test]
fn reader_errors_and_suppression_are_observable() {
    let (runtime, mut ctx, mut options) = setup();
    for (text, error) in [
        (")", ReadError::UnmatchedRightParen),
        ("(.", ReadError::DotWithoutCdr),
        ("(", ReadError::UnexpectedEof),
        ("\"unterminated", ReadError::UnexpectedEof),
        ("#?", ReadError::UndefinedDispatchMacro('?')),
        ("#.", ReadError::ReadEvalUnavailable),
        ("#A", ReadError::ArraySyntax),
        ("#P", ReadError::PathnameSyntax),
    ] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, text, &options),
            Err(error)
        );
    }
    options.set_read_evaluation(ReadEvaluation::Disabled);
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#.", &options),
        Err(ReadError::ReadEvalDisabled)
    );
    options.set_read_suppression(ReadSuppression::Discard);
    assert_eq!(read_one(&runtime, &mut ctx, "(a b)", &options), Word::NIL);
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "   ; comment\n", &options),
        Ok(None)
    );
}

#[test]
fn number_parser_covers_radices_ratios_and_float_failures() {
    let (runtime, mut ctx, _) = setup();
    assert_eq!(digit_value('F', 16), Some(15));
    assert_eq!(digit_value('G', 16), None);
    assert_eq!(digit_value('x', 10), None);
    assert_eq!(
        parse_integer_chars(&mut ctx, &runtime, &['-', '1', '0', '.'], 10).required(),
        classify_word(-10)
    );
    assert!(
        parse_number(
            &mut ctx,
            &runtime,
            &['1', '/', '2'],
            10,
            FloatFormat::DoubleFloat
        )
        .required()
        .is_some()
    );
    assert_eq!(
        parse_number(
            &mut ctx,
            &runtime,
            &['1', '.', 'x'],
            10,
            FloatFormat::DoubleFloat
        )
        .required(),
        None
    );
    assert_eq!(
        parse_number(
            &mut ctx,
            &runtime,
            &['1', '.', '2', 's', '3'],
            10,
            FloatFormat::DoubleFloat
        ),
        Err(ReadError::FloatFormatUnavailable('s'))
    );
    assert_eq!(
        parse_integer_chars(&mut ctx, &runtime, &[], 10),
        Err(ReadError::InvalidNumber(String::new()))
    );
    assert_eq!(
        parse_integer_chars(&mut ctx, &runtime, &['2'], 2),
        Err(ReadError::InvalidDigit('2'))
    );
}

fn classify_word(value: i64) -> Word {
    Word::fixnum(value)
}

#[test]
fn readtable_operations_copy_and_classify_entries() {
    let (runtime, mut ctx, _) = setup();
    let table = crate::standard_readtable(&mut ctx, &runtime).required();
    assert_eq!(table.case_mode(&ctx).required(), ReadtableCase::Upcase);
    assert_eq!(syntax_kind(&ctx, table, ' '), Ok(SyntaxKind::Whitespace));
    assert_eq!(syntax_kind(&ctx, table, 'λ'), Ok(SyntaxKind::Constituent));
    assert_eq!(
        crate::get_macro_character(&mut ctx, table, 'A').required(),
        None
    );
    assert_eq!(
        crate::get_macro_character(&mut ctx, table, 'λ').required(),
        None
    );
    crate::set_macro_character(
        &mut ctx,
        &runtime,
        table,
        '@',
        Some(Word::NIL),
        CustomMacroKind::Terminating,
    )
    .required();
    assert_eq!(
        syntax_kind(&ctx, table, '@'),
        Ok(SyntaxKind::CustomMacro(CustomMacroKind::Terminating))
    );
    assert_eq!(
        crate::get_macro_character(&mut ctx, table, '@').required(),
        Some(Word::NIL)
    );
    crate::set_macro_character(
        &mut ctx,
        &runtime,
        table,
        '@',
        None,
        CustomMacroKind::Terminating,
    )
    .required();
    assert_eq!(syntax_kind(&ctx, table, '@'), Ok(SyntaxKind::Constituent));
    assert_eq!(
        crate::set_dispatch_macro_character(&mut ctx, &runtime, table, '#', 'q', Word::fixnum(7)),
        Ok(())
    );
    assert_eq!(
        crate::get_dispatch_macro_character(&mut ctx, table, '#', 'q').required(),
        Some(Word::fixnum(7))
    );
    assert_eq!(
        crate::get_dispatch_macro_character(&mut ctx, table, '!', 'q'),
        Err(ReadError::NotDispatchMacro('!'))
    );
    crate::make_dispatch_macro_character(&mut ctx, &runtime, table, '#').required();
    crate::set_syntax_from_char(&mut ctx, '@', '(', table, table).required();
    assert_eq!(
        syntax_kind(&ctx, table, '@'),
        Ok(SyntaxKind::TerminatingMacro)
    );
    let copy = crate::copy_readtable(&mut ctx, &runtime, table).required();
    assert_eq!(copy.case_mode(&ctx).required(), ReadtableCase::Upcase);
    assert!(crate::readtablep(&ctx, table.object().as_word()));
    assert!(!crate::readtablep(&ctx, Word::NIL));
    assert_eq!(
        readtable_from_word(Word::NIL),
        Err(ReadError::Object(ncl_object::ObjectError::TypeError))
    );
}

#[test]
fn feature_expressions_and_dispatch_helpers_return_values() {
    let (runtime, mut ctx, options) = setup();
    let and_form = read_one(&runtime, &mut ctx, "(and :x (not :y))", &options);
    let or_form = read_one(&runtime, &mut ctx, "(or :y :x)", &options);
    let bad_form = read_one(&runtime, &mut ctx, "(xor :x)", &options);
    assert!(eval_feature_expr(&mut ctx, &runtime, and_form, &["X".into()]).required());
    assert!(eval_feature_expr(&mut ctx, &runtime, or_form, &["X".into()]).required());
    assert_eq!(
        eval_feature_expr(&mut ctx, &runtime, bad_form, &["X".into()]),
        Err(ReadError::InvalidFeatureExpression)
    );
    assert_eq!(
        eval_feature_expr(&mut ctx, &runtime, Word::fixnum(1), &[]),
        Err(ReadError::InvalidFeatureExpression)
    );

    let rt = Cell::new(options.readtable().object().as_word());
    let labels = Cell::new(Word::NIL);
    let mut source = StringSource::new("'");
    assert_eq!(
        read_sharp(&mut ctx, &runtime, &mut source, &options, &rt, &labels),
        Err(ReadError::UnexpectedEof)
    );
    let mut source = StringSource::new("");
    assert_eq!(
        read_sharp(&mut ctx, &runtime, &mut source, &options, &rt, &labels),
        Err(ReadError::UnexpectedEof)
    );
}

#[test]
fn error_display_and_source_have_stable_results() {
    let errors = [
        ReadError::InvalidSymbolToken("x".into()),
        ReadError::PackageNotFound("P".into()),
        ReadError::InvalidBase(1),
        ReadError::InvalidDigit('z'),
        ReadError::InvalidNumber("x".into()),
        ReadError::NumberOutOfRange,
        ReadError::FloatFormatUnavailable('s'),
        ReadError::InvalidCharacter,
        ReadError::UnknownCharacterName("NOPE".into()),
        ReadError::UninvocableMacroFunction('@'),
        ReadError::InvalidFeatureExpression,
        ReadError::StructureSyntax,
        ReadError::NotDispatchMacro('!'),
    ];
    for error in errors {
        assert!(!error.to_string().is_empty());
        assert!(std::error::Error::source(&error).is_none());
    }
}
