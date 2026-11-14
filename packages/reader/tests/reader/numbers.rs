use super::*;

#[test]
fn reads_fixnums() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    assert_eq!(
        classify(read_one(&runtime, &mut ctx, "42")),
        ObjectRef::Fixnum(42)
    );
    assert_eq!(
        classify(read_one(&runtime, &mut ctx, "-7")),
        ObjectRef::Fixnum(-7)
    );
    assert_eq!(
        classify(read_one(&runtime, &mut ctx, "+3")),
        ObjectRef::Fixnum(3)
    );
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
fn reads_bignums_and_ratios_beyond_i128() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let integer = read_one(
        &runtime,
        &mut ctx,
        "1234678123487612347896123467851234671234",
    );
    assert!(matches!(
        classify_object(&ctx, integer),
        ObjectRef::Bignum(_)
    ));
    assert_eq!(
        bignum_limbs(&ctx, Bignum::from_word(integer)).unwrap(),
        [
            3_806_722_690,
            1_768_204_404,
            1_787_948_446,
            2_698_926_946,
            3
        ],
    );

    let ratio = read_one(
        &runtime,
        &mut ctx,
        "189729874978126783786123/1234678123487612347896123467851234671234",
    );
    assert!(matches!(classify_object(&ctx, ratio), ObjectRef::Ratio(_)));
    let denominator = ratio_denominator(&ctx, Ratio::from_word(ratio)).unwrap();
    assert!(matches!(
        classify_object(&ctx, denominator),
        ObjectRef::Bignum(_)
    ));
}

#[test]
fn reads_ratios_and_floats() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    assert_eq!(
        standard(&runtime, &mut ctx).default_float_format(),
        ncl_reader::FloatFormat::SingleFloat
    );
    let ratio = read_one(&runtime, &mut ctx, "3/4");
    assert!(matches!(classify_object(&ctx, ratio), ObjectRef::Ratio(_)));
    let float = read_one(&runtime, &mut ctx, "3.14");
    assert!(matches!(
        classify_object(&ctx, float),
        ObjectRef::DoubleFloat(_)
    ));
    let exp = read_one(&runtime, &mut ctx, "1.5e3");
    assert!(matches!(
        classify_object(&ctx, exp),
        ObjectRef::DoubleFloat(_)
    ));
}

#[test]
fn exponent_marker_names_are_symbols_without_a_mantissa() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    for token in ["e1", "s1", "f1", "d1", "l1"] {
        assert_eq!(read_name(&runtime, &mut ctx, token), token.to_uppercase());
    }
    let opts = standard(&runtime, &mut ctx);
    let error = read_from_string(&mut ctx, &runtime, "+e1", &opts).unwrap_err();
    assert!(matches!(error, ncl_reader::ReadError::InvalidNumber(_)));
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
    opts.set_readtable(table);
    let mut source = ncl_reader::StringSource::new("HELLO");
    let word = read(&mut ctx, &runtime, &mut source, &opts)
        .unwrap()
        .unwrap();
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
    let pkg_name = ncl_object::Package::from_word(pkg).name(&ctx).unwrap();
    let len = string_length(&ctx, pkg_name).unwrap();
    let pkg_name: String = (0..len)
        .map(|i| string_ref(&ctx, pkg_name, i).unwrap())
        .collect();
    assert_eq!(pkg_name, "KEYWORD");

    let empty = read_one(&runtime, &mut ctx, ":||");
    assert_eq!(name_of(&ctx, empty), "");
    assert_eq!(
        symbol_package(&ctx, empty).unwrap(),
        runtime.find_package(&ctx, "KEYWORD").unwrap()
    );

    let empty_external = read_one(&runtime, &mut ctx, "COMMON-LISP-USER:||");
    assert_eq!(name_of(&ctx, empty_external), "");
    assert_eq!(
        symbol_package(&ctx, empty_external).unwrap(),
        runtime.find_package(&ctx, "COMMON-LISP-USER").unwrap()
    );
}

#[test]
fn reads_an_empty_symbol_name() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let empty = read_one(&runtime, &mut ctx, "||");
    assert_eq!(name_of(&ctx, empty), "");
    let package = read_one(&runtime, &mut ctx, "COMMON-LISP-USER||:");
    assert_eq!(
        package,
        runtime.find_package(&ctx, "COMMON-LISP-USER").unwrap()
    );
    assert_eq!(
        classify(read_one(&runtime, &mut ctx, "||123")),
        ObjectRef::Fixnum(123)
    );
}

#[test]
fn current_package_local_nickname_resolves_before_global_lookup() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut current = runtime.ensure_package(&mut ctx, "CURRENT").unwrap();
    let current_token = ncl_object::push_root(&mut ctx, &mut current);
    let mut target = runtime.ensure_package(&mut ctx, "TARGET").unwrap();
    let target_token = ncl_object::push_root(&mut ctx, &mut target);
    let mut global = runtime.ensure_package(&mut ctx, "GLOBAL").unwrap();
    let global_token = ncl_object::push_root(&mut ctx, &mut global);
    let nickname = make_string(&mut ctx, &runtime, &['L', 'O', 'C', 'A', 'L']).unwrap();
    let entry = make_cons(&mut ctx, &runtime, nickname, target).unwrap();
    let local_nicknames = make_cons(&mut ctx, &runtime, entry, Word::NIL).unwrap();
    Package::from_word(current)
        .set_local_nicknames(&mut ctx, local_nicknames)
        .unwrap();
    let mut opts = standard(&runtime, &mut ctx);
    opts.set_current_package("CURRENT").unwrap();
    assert_eq!(runtime.find_package(&ctx, "GLOBAL"), Some(global));

    let symbol = read_from_string(&mut ctx, &runtime, "LOCAL:ITEM", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_package(&ctx, symbol).unwrap(), target);
    assert_eq!(name_of(&ctx, symbol), "ITEM");

    let symbol = read_from_string(&mut ctx, &runtime, "GLOBAL:ITEM", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_package(&ctx, symbol).unwrap(), global);
    assert!(ncl_object::pop_root(&mut ctx, global_token));
    assert!(ncl_object::pop_root(&mut ctx, target_token));
    assert!(ncl_object::pop_root(&mut ctx, current_token));
}

#[test]
fn reads_quote_and_quasiquote() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let quoted = read_one(&runtime, &mut ctx, "'x");
    let head = car(&ctx, quoted).unwrap();
    assert_eq!(name_of(&ctx, head), "QUOTE");
    let rest = cdr(&ctx, quoted).unwrap();
    let inner = car(&ctx, rest).unwrap();
    assert_eq!(name_of(&ctx, inner), "X");
}

#[test]
fn reads_lists_and_dotted_lists() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    assert_eq!(read_names(&runtime, &mut ctx, "(a b c)"), ["A", "B", "C"]);
    let dotted = read_one(&runtime, &mut ctx, "(a . b)");
    let head = car(&ctx, dotted).unwrap();
    assert_eq!(name_of(&ctx, head), "A");
    let tail = cdr(&ctx, dotted).unwrap();
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
    assert_eq!(
        read_one(&runtime, &mut ctx, "#\\Space"),
        Word::character(32)
    );
    assert_eq!(
        read_one(&runtime, &mut ctx, "#\\Newline"),
        Word::character(10)
    );
    assert_eq!(read_one(&runtime, &mut ctx, "#\\("), Word::character(40));
}

#[test]
fn reads_vector_and_bit_vector_literals() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let v = read_one(&runtime, &mut ctx, "#(1 2 3)");
    assert!(matches!(
        classify_object(&ctx, v),
        ObjectRef::SimpleVector(_)
    ));
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
fn reads_quote_with_the_common_lisp_symbol() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let common = Package::from_word(runtime.find_package(&ctx, "COMMON-LISP").unwrap());
    let (quote, status) = common.intern(&mut ctx, &runtime, "QUOTE").unwrap();
    assert_eq!(status, ncl_object::package::FindStatus::External);
    let form = read_one(&runtime, &mut ctx, "(quote a)");
    let operator = car(&ctx, form).unwrap();
    assert_eq!(operator, quote);
    assert_eq!(symbol_package(&ctx, operator), Ok(common.as_word()));
}
