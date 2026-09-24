#![allow(clippy::unwrap_used, reason = "tests assert on reader behavior")]

//! Reader acceptance tests: macro characters, dispatch characters, numbers,
//! readtable case, package prefixes, errors, and feature evaluation.

use ncl_object::{
    ObjectRef, Runtime, ThreadContext, Word, car, cdr, classify, classify_object, string_length,
    string_ref, symbol_name,
};
use ncl_reader::{ReadOptions, ReadtableCase, read, read_from_string};

fn standard(runtime: &Runtime, ctx: &mut ThreadContext) -> ReadOptions {
    ReadOptions::standard(ctx, runtime).unwrap()
}

fn read_one(runtime: &Runtime, ctx: &mut ThreadContext, text: &str) -> Word {
    let opts = standard(runtime, ctx);
    read_from_string(ctx, runtime, text, &opts).unwrap().unwrap()
}

fn name_of(ctx: &ThreadContext, word: Word) -> String {
    let name = symbol_name(ctx, word).unwrap();
    let length = string_length(ctx, name).unwrap();
    (0..length).map(|i| string_ref(ctx, name, i).unwrap()).collect()
}

fn read_name(runtime: &Runtime, ctx: &mut ThreadContext, text: &str) -> String {
    let word = read_one(runtime, ctx, text);
    name_of(ctx, word)
}

fn list_names(ctx: &mut ThreadContext, list: Word) -> Vec<String> {
    let mut names = Vec::new();
    let mut cursor = list;
    while cursor != Word::NIL {
        let item = car(ctx, cursor).unwrap();
        names.push(name_of(ctx, item));
        cursor = cdr(ctx, cursor).unwrap();
    }
    names
}

fn read_names(runtime: &Runtime, ctx: &mut ThreadContext, text: &str) -> Vec<String> {
    let list = read_one(runtime, ctx, text);
    list_names(ctx, list)
}

#[test]
fn reads_fixnums() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    assert_eq!(classify(read_one(&runtime, &mut ctx, "42")), ObjectRef::Fixnum(42));
    assert_eq!(classify(read_one(&runtime, &mut ctx, "-7")), ObjectRef::Fixnum(-7));
    assert_eq!(classify(read_one(&runtime, &mut ctx, "+3")), ObjectRef::Fixnum(3));
}

#[test]
fn reads_a_bignum_beyond_fixnum() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let big = read_one(&runtime, &mut ctx, "4611686018427387904");
    assert!(matches!(classify_object(&ctx, big), ObjectRef::Bignum(_)));
}

#[test]
fn reads_ratios_and_floats() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let ratio = read_one(&runtime, &mut ctx, "3/4");
    assert!(matches!(classify_object(&ctx, ratio), ObjectRef::Ratio(_)));
    let float = read_one(&runtime, &mut ctx, "3.14");
    assert!(matches!(classify_object(&ctx, float), ObjectRef::DoubleFloat(_)));
    let exp = read_one(&runtime, &mut ctx, "1.5e3");
    assert!(matches!(classify_object(&ctx, exp), ObjectRef::DoubleFloat(_)));
}

#[test]
fn readtable_case_upcases_symbols_by_default() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    assert_eq!(read_name(&runtime, &mut ctx, "hello"), "HELLO");
    assert_eq!(read_name(&runtime, &mut ctx, "HiThere"), "HITHERE");
}

#[test]
fn escapes_and_bars_preserve_case() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    assert_eq!(read_name(&runtime, &mut ctx, "|Hello|"), "Hello");
    assert_eq!(read_name(&runtime, &mut ctx, "\\H\\ello"), "HeLLO");
}

#[test]
fn downcase_readtable_case_folds_symbols() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let table = ncl_reader::standard_readtable(&mut ctx, &runtime).unwrap();
    ctx.write_object_slot(
        table.object().as_word(),
        ncl_object::readtable_offset::CASE,
        Word::fixnum(1),
    )
    .unwrap();
    let mut opts = standard(&runtime, &mut ctx);
    opts.readtable = table;
    let mut source = ncl_reader::StringSource::new("HELLO");
    let word = read(&mut ctx, &runtime, &mut source, &opts).unwrap().unwrap();
    assert_eq!(name_of(&ctx, word), "hello");
}

#[test]
fn reads_keyword_symbols() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let kw = read_one(&runtime, &mut ctx, ":foo");
    assert_eq!(name_of(&ctx, kw), "FOO");
    let pkg = ncl_object::symbol_package(&ctx, kw).unwrap();
    let pkg_name = ncl_object::Package::from(pkg).name(&ctx).unwrap();
    let len = string_length(&ctx, pkg_name).unwrap();
    let pkg_name: String = (0..len).map(|i| string_ref(&ctx, pkg_name, i).unwrap()).collect();
    assert_eq!(pkg_name, "KEYWORD");
}

#[test]
fn reads_quote_and_quasiquote() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let quoted = read_one(&runtime, &mut ctx, "'x");
    let head = car(&mut ctx, quoted).unwrap();
    assert_eq!(name_of(&ctx, head), "QUOTE");
    let rest = cdr(&mut ctx, quoted).unwrap();
    let inner = car(&mut ctx, rest).unwrap();
    assert_eq!(name_of(&ctx, inner), "X");
}

#[test]
fn reads_lists_and_dotted_lists() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    assert_eq!(read_names(&runtime, &mut ctx, "(a b c)"), ["A", "B", "C"]);
    let dotted = read_one(&runtime, &mut ctx, "(a . b)");
    let head = car(&mut ctx, dotted).unwrap();
    assert_eq!(name_of(&ctx, head), "A");
    let tail = cdr(&mut ctx, dotted).unwrap();
    assert_eq!(name_of(&ctx, tail), "B");
}

#[test]
fn reads_strings_with_escapes() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let s = read_one(&runtime, &mut ctx, "\"he\\\"llo\"");
    assert!(matches!(classify_object(&ctx, s), ObjectRef::String(_)));
    let len = string_length(&ctx, s).unwrap();
    let text: String = (0..len).map(|i| string_ref(&ctx, s, i).unwrap()).collect();
    assert_eq!(text, "he\"llo");
}

#[test]
fn reads_character_literals() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    assert_eq!(read_one(&runtime, &mut ctx, "#\\a"), Word::character(97));
    assert_eq!(read_one(&runtime, &mut ctx, "#\\Space"), Word::character(32));
    assert_eq!(read_one(&runtime, &mut ctx, "#\\Newline"), Word::character(10));
    assert_eq!(read_one(&runtime, &mut ctx, "#\\("), Word::character(40));
}

#[test]
fn reads_vector_and_bit_vector_literals() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let v = read_one(&runtime, &mut ctx, "#(1 2 3)");
    assert!(matches!(classify_object(&ctx, v), ObjectRef::SimpleVector(_)));
    let bits = read_one(&runtime, &mut ctx, "#*101");
    assert!(matches!(
        classify_object(&ctx, bits),
        ObjectRef::SpecializedArray(_)
    ));
}

#[test]
fn reads_uninterned_symbols() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let sym = read_one(&runtime, &mut ctx, "#:foo");
    assert_eq!(name_of(&ctx, sym), "FOO");
}

#[test]
fn reads_radix_integers() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    assert_eq!(classify(read_one(&runtime, &mut ctx, "#xff")), ObjectRef::Fixnum(255));
    assert_eq!(classify(read_one(&runtime, &mut ctx, "#b101")), ObjectRef::Fixnum(5));
    assert_eq!(classify(read_one(&runtime, &mut ctx, "#o17")), ObjectRef::Fixnum(15));
    assert_eq!(classify(read_one(&runtime, &mut ctx, "#2r101")), ObjectRef::Fixnum(5));
}

#[test]
fn reads_complex_literals() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let c = read_one(&runtime, &mut ctx, "#c(1 2)");
    assert!(matches!(classify_object(&ctx, c), ObjectRef::Complex(_)));
}

#[test]
fn reads_function_abbreviation() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let form = read_one(&runtime, &mut ctx, "#'car");
    let head = car(&mut ctx, form).unwrap();
    assert_eq!(name_of(&ctx, head), "FUNCTION");
}

#[test]
fn block_comments_are_skipped() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    assert_eq!(
        classify(read_one(&runtime, &mut ctx, "#| a comment |# 42")),
        ObjectRef::Fixnum(42)
    );
}

#[test]
fn feature_conditionals_select_forms() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    runtime.add_feature("MY-FEATURE");
    assert_eq!(
        classify(read_one(&runtime, &mut ctx, "#+my-feature 1 2")),
        ObjectRef::Fixnum(1)
    );
    assert_eq!(
        classify(read_one(&runtime, &mut ctx, "#-my-feature 1 2")),
        ObjectRef::Fixnum(2)
    );
}

#[test]
fn read_base_is_honoured() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut opts = standard(&runtime, &mut ctx);
    opts.read_base = 16;
    let mut source = ncl_reader::StringSource::new("ff");
    let word = read(&mut ctx, &runtime, &mut source, &opts).unwrap().unwrap();
    assert_eq!(classify(word), ObjectRef::Fixnum(255));
}

#[test]
fn single_float_marker_is_rejected() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let opts = standard(&runtime, &mut ctx);
    let error = read_from_string(&mut ctx, &runtime, "1.5f0", &opts).unwrap_err();
    assert!(matches!(error, ncl_reader::ReadError::UnsupportedFloatFormat(_)));
    let word = read_from_string(&mut ctx, &runtime, "1.5d0", &opts).unwrap().unwrap();
    assert!(matches!(classify_object(&ctx, word), ObjectRef::DoubleFloat(_)));
}

#[test]
fn unmatched_paren_is_an_error() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let opts = standard(&runtime, &mut ctx);
    let error = read_from_string(&mut ctx, &runtime, ")", &opts).unwrap_err();
    assert!(matches!(error, ncl_reader::ReadError::UnmatchedRightParen));
}

#[test]
fn eof_in_string_is_an_error() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let opts = standard(&runtime, &mut ctx);
    let error = read_from_string(&mut ctx, &runtime, "\"unterminated", &opts).unwrap_err();
    assert!(matches!(error, ncl_reader::ReadError::UnexpectedEof));
}

#[test]
fn undefined_dispatch_is_an_error() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let opts = standard(&runtime, &mut ctx);
    let error = read_from_string(&mut ctx, &runtime, "#q", &opts).unwrap_err();
    assert!(matches!(error, ncl_reader::ReadError::UndefinedDispatchMacro('q')));
}

#[test]
fn read_eval_is_rejected() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let opts = standard(&runtime, &mut ctx);
    let error = read_from_string(&mut ctx, &runtime, "#.", &opts).unwrap_err();
    assert!(matches!(error, ncl_reader::ReadError::ReadEvalUnavailable));
}

#[test]
fn labels_reference_prior_forms() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let form = read_one(&runtime, &mut ctx, "(#1=(a b) #1#)");
    let first = car(&mut ctx, form).unwrap();
    assert_eq!(list_names(&mut ctx, first), ["A", "B"]);
    let rest = cdr(&mut ctx, form).unwrap();
    let second = car(&mut ctx, rest).unwrap();
    assert_eq!(list_names(&mut ctx, second), ["A", "B"]);
}

#[test]
fn standard_readtable_reports_upcase_and_is_a_readtable() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let table = ncl_reader::standard_readtable(&mut ctx, &runtime).unwrap();
    assert_eq!(ncl_reader::readtable_case(&ctx, table).unwrap(), ReadtableCase::Upcase);
    assert!(ncl_reader::readtablep(&ctx, table.object().as_word()));
    let copy = ncl_reader::copy_readtable(&mut ctx, &runtime, table).unwrap();
    assert_eq!(ncl_reader::readtable_case(&ctx, copy).unwrap(), ReadtableCase::Upcase);
}

#[test]
fn parse_integer_reads_digits_and_stops() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let (value, index) =
        ncl_reader::parse_integer(&mut ctx, &runtime, "42 rest", None, None, None).unwrap();
    assert_eq!(classify(value), ObjectRef::Fixnum(42));
    assert_eq!(index, 2);
    let (value, index) =
        ncl_reader::parse_integer(&mut ctx, &runtime, "ff", Some(16), None, None).unwrap();
    assert_eq!(classify(value), ObjectRef::Fixnum(255));
    assert_eq!(index, 2);
}
