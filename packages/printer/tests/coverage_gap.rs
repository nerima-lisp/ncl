#![allow(clippy::unwrap_used, reason = "tests assert on printer output")]

//! Regression tests for printer paths not covered by the object-kind examples.

use ncl_object::{Package, Runtime, ThreadContext, Word, make_cons};
use ncl_printer::{
    CharSink, CircleSharingMode, PrintCase, PrintError, PrintOptions, StringSink,
    copy_pprint_dispatch, pprint_dispatch, set_pprint_dispatch, write, write_to_string,
};

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    (runtime, ctx)
}

fn intern(runtime: &Runtime, ctx: &mut ThreadContext, package: &str, name: &str) -> Word {
    let package = runtime.find_package(ctx, package).unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn list(runtime: &Runtime, ctx: &mut ThreadContext, values: &[Word]) -> Word {
    values.iter().rev().fold(Word::NIL, |tail, value| {
        make_cons(ctx, runtime, *value, tail).unwrap()
    })
}

fn string_value(ctx: &ThreadContext, string: Word) -> String {
    let length = ncl_object::string_length(ctx, string).unwrap();
    (0..length)
        .map(|index| ncl_object::string_ref(ctx, string, index).unwrap())
        .collect()
}

#[test]
fn print_options_cover_all_modes_and_base_validation() {
    let options = PrintOptions::new()
        .with_escape(false)
        .with_readably(true)
        .with_base(36)
        .with_radix(true)
        .with_case(PrintCase::Capitalize)
        .with_circle(true)
        .with_length(Some(3))
        .with_level(Some(2))
        .with_pretty(true)
        .with_array(false)
        .with_gensym(false)
        .with_circle_not_shared(true)
        .with_vector_length(Some(1));

    assert!(!options.escape());
    assert!(options.readably());
    assert_eq!(options.base().get(), 36);
    assert!(options.radix());
    assert_eq!(options.case(), PrintCase::Capitalize);
    assert!(options.circle());
    assert_eq!(options.length().map(ncl_printer::NonNegative::get), Some(3));
    assert_eq!(options.level().map(ncl_printer::NonNegative::get), Some(2));
    assert!(options.pretty());
    assert!(!options.array());
    assert!(!options.gensym());
    assert_eq!(options.circle_sharing_mode(), CircleSharingMode::OnlyShared);
    assert_eq!(
        options.vector_length().map(ncl_printer::NonNegative::get),
        Some(1)
    );
    assert_eq!(options.try_with_base(1), None);
    assert_eq!(options.try_with_base(2).unwrap().base().get(), 2);
    assert_eq!(options.with_base(37).base().get(), 36);
}

#[test]
fn ambient_specials_accept_values_and_reject_invalid_values() {
    let (runtime, mut ctx) = context();
    let escape = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-ESCAPE*");
    let base = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-BASE*");
    let case = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-CASE*");
    let length = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-LENGTH*");
    for symbol in [escape, base, case, length] {
        ncl_object::set_symbol_special(&mut ctx, symbol, true).unwrap();
    }
    ncl_object::set_symbol_value(&mut ctx, escape, Word::NIL).unwrap();
    ncl_object::set_symbol_value(&mut ctx, base, Word::fixnum(16)).unwrap();
    ncl_object::set_symbol_value(&mut ctx, case, Word::fixnum(7)).unwrap();
    ncl_object::set_symbol_value(&mut ctx, length, Word::fixnum(4)).unwrap();

    let options = PrintOptions::from_specials(&mut ctx, &runtime);
    assert!(!options.escape());
    assert_eq!(options.base().get(), 16);
    assert_eq!(options.case(), PrintCase::Upcase);
    assert_eq!(options.length().map(ncl_printer::NonNegative::get), Some(4));

    ncl_object::set_symbol_value(&mut ctx, base, Word::fixnum(1)).unwrap();
    ncl_object::set_symbol_value(&mut ctx, case, Word::NIL).unwrap();
    ncl_object::set_symbol_value(&mut ctx, length, Word::TRUE).unwrap();
    let fallback = PrintOptions::from_specials(&mut ctx, &runtime);
    assert_eq!(fallback.base().get(), 10);
    assert_eq!(fallback.case(), PrintCase::Upcase);
    assert_eq!(fallback.length(), None);
}

#[test]
fn dispatch_tables_match_identity_skip_atoms_and_copy_entries() {
    let (runtime, mut ctx) = context();
    let object = Word::fixnum(42);
    let marker = Word::fixnum(99);
    let exact_entry = make_cons(&mut ctx, &runtime, object, marker).unwrap();
    let default_entry = make_cons(&mut ctx, &runtime, Word::TRUE, Word::fixnum(1)).unwrap();
    let table = list(
        &runtime,
        &mut ctx,
        &[Word::fixnum(7), exact_entry, default_entry],
    );
    assert_eq!(pprint_dispatch(&mut ctx, object, table).unwrap(), marker);
    assert_eq!(
        pprint_dispatch(&mut ctx, Word::fixnum(8), table).unwrap(),
        Word::fixnum(1)
    );

    let copied_source = list(&runtime, &mut ctx, &[exact_entry]);
    let copied = copy_pprint_dispatch(&mut ctx, &runtime, copied_source).unwrap();
    assert_eq!(pprint_dispatch(&mut ctx, object, copied).unwrap(), marker);
    assert_eq!(
        pprint_dispatch(&mut ctx, Word::fixnum(8), Word::NIL).unwrap(),
        Word::NIL
    );

    let shadowed =
        set_pprint_dispatch(&mut ctx, &runtime, object, Word::fixnum(123), table).unwrap();
    assert_eq!(
        pprint_dispatch(&mut ctx, object, shadowed).unwrap(),
        Word::fixnum(123)
    );
}

#[test]
fn pretty_printing_and_write_to_string_preserve_output_contract() {
    let (runtime, mut ctx) = context();
    let values: Vec<_> = (0..60).map(Word::fixnum).collect();
    let object = list(&runtime, &mut ctx, &values);
    let rendered = write_to_string(
        &mut ctx,
        &runtime,
        object,
        &PrintOptions::new().with_pretty(true),
    )
    .unwrap();
    let text = string_value(&ctx, rendered);
    assert!(
        text.contains('\n'),
        "pretty output should wrap at the margin: {text}"
    );
    assert!(text.starts_with('('));
    assert!(text.ends_with(')'));
}

struct FailingSink;

impl CharSink for FailingSink {
    fn write_char(&mut self, _character: char) -> Result<(), PrintError> {
        Err(PrintError::Sink("closed".to_owned()))
    }
}

#[test]
fn sink_errors_and_print_errors_are_observable() {
    let (runtime, mut ctx) = context();
    let mut sink = FailingSink;
    let error = write(
        &mut ctx,
        &runtime,
        Word::fixnum(1),
        &mut sink,
        &PrintOptions::new(),
    )
    .unwrap_err();
    assert_eq!(error, PrintError::Sink("closed".to_owned()));
    assert_eq!(error.to_string(), "print: sink error: closed");
    assert!(std::error::Error::source(&error).is_none());

    let mut string_sink = StringSink::new();
    string_sink.write_str("ok").unwrap();
    assert_eq!(string_sink.as_str(), "ok");
    assert_eq!(string_sink.into_string(), "ok");
}
