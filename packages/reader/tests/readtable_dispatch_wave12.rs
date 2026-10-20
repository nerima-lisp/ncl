#![allow(clippy::unwrap_used, reason = "tests assert on reader behavior")]

//! Concrete LLVM-coverage follow-up for reachable reader branches.

use ncl_object::{
    ObjectRef, Package, Runtime, ThreadContext, Word, car, cdr, classify, classify_object,
    make_readtable, make_simple_vector, structure_ref, symbol_name,
};
use ncl_reader::{
    ReadError, ReadEvaluation, ReadOptions, Readtable, read_from_string, readtable_case,
    set_dispatch_macro_character,
};

fn setup() -> (Runtime, ThreadContext, ReadOptions) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let opts = ReadOptions::standard(&mut ctx, &runtime).unwrap();
    (runtime, ctx, opts)
}

fn symbol_text(ctx: &ThreadContext, word: Word) -> String {
    let name = symbol_name(ctx, word).unwrap();
    let length = ncl_object::string_length(ctx, name).unwrap();
    (0..length)
        .map(|index| ncl_object::string_ref(ctx, name, index).unwrap())
        .collect()
}

#[test]
fn malformed_readtable_cases_and_dispatch_entries_are_observable() {
    let (runtime, mut ctx, opts) = setup();
    let table = opts.readtable();
    let syntax = table.syntax_table(&ctx).unwrap();
    let dispatch = table.dispatch_table(&ctx).unwrap();
    let malformed_nil = make_readtable(&mut ctx, &runtime, syntax, dispatch, Word::NIL).unwrap();
    let mut bad_opts = opts.clone();
    bad_opts.set_readtable(Readtable::from_object(malformed_nil));
    assert!(matches!(
        readtable_case(&ctx, bad_opts.readtable()),
        Err(ReadError::InvalidNumber(_))
    ));
    assert!(matches!(
        read_from_string(&mut ctx, &runtime, "name", &bad_opts),
        Err(ReadError::InvalidNumber(_))
    ));

    let malformed_code =
        make_readtable(&mut ctx, &runtime, syntax, dispatch, Word::fixnum(99)).unwrap();
    assert_eq!(
        readtable_case(&ctx, Readtable::from_object(malformed_code)).unwrap_err(),
        ReadError::InvalidNumber(format!("{:?}", Word::fixnum(99)))
    );

    set_dispatch_macro_character(&mut ctx, &runtime, table, '#', 'q', Word::fixnum(17)).unwrap();
    assert_eq!(
        ncl_reader::get_dispatch_macro_character(&mut ctx, table, '#', 'q').unwrap(),
        Some(Word::fixnum(17))
    );
    assert_eq!(
        ncl_reader::get_dispatch_macro_character(&mut ctx, table, '#', 'λ').unwrap(),
        None
    );
}

#[test]
fn sharp_function_uninterned_eval_and_feature_forms_have_distinct_results() {
    let (runtime, mut ctx, mut opts) = setup();
    let function = read_from_string(&mut ctx, &runtime, "#'car", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, car(&ctx, function).unwrap()), "FUNCTION");
    assert_eq!(
        symbol_text(&ctx, car(&ctx, cdr(&ctx, function).unwrap()).unwrap()),
        "CAR"
    );

    let uninterned = read_from_string(&mut ctx, &runtime, "#:mixed", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, uninterned),
        ObjectRef::Symbol(_)
    ));
    assert_eq!(symbol_text(&ctx, uninterned), "MIXED");
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#:", &opts).unwrap_err(),
        ReadError::UnexpectedEof
    );

    opts.set_read_evaluation(ReadEvaluation::Disabled);
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#.", &opts).unwrap_err(),
        ReadError::ReadEvalDisabled
    );

    runtime.add_feature("WAVE12");
    let present = read_from_string(&mut ctx, &runtime, "#+wave12 13", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(classify(present), ObjectRef::Fixnum(13));
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#-wave12 14", &opts).unwrap(),
        None
    );
}

fn register_point(runtime: &Runtime, ctx: &mut ThreadContext) {
    let package = runtime.ensure_package(ctx, "COMMON-LISP-USER").unwrap();
    let package = Package::from_word(package);
    let (name, _) = package.intern(ctx, runtime, "W12-POINT").unwrap();
    let (x_name, _) = package.intern(ctx, runtime, "X").unwrap();
    let (y_name, _) = package.intern(ctx, runtime, "Y").unwrap();
    let x_descriptor = make_simple_vector(ctx, runtime, &[x_name, Word::NIL, Word::NIL]).unwrap();
    let y_descriptor = make_simple_vector(ctx, runtime, &[y_name, Word::NIL, Word::NIL]).unwrap();
    let slots = make_simple_vector(ctx, runtime, &[x_descriptor, y_descriptor]).unwrap();
    let class = make_simple_vector(
        ctx,
        runtime,
        &[name, Word::NIL, slots, Word::fixnum(12), slots],
    )
    .unwrap();
    runtime.define_class(ctx, "W12-POINT", class).unwrap();
    let qualified = runtime.structure_class_name(ctx, name).unwrap();
    runtime.define_class(ctx, qualified, class).unwrap();
    let layout = runtime.register_structure_layout(2).unwrap();
    runtime
        .register_structure_class_with_parent(ctx, layout, None, name)
        .unwrap();
}

#[test]
fn structure_dispatch_reads_all_slots_and_rejects_bad_field_shapes() {
    let (runtime, mut ctx, opts) = setup();
    register_point(&runtime, &mut ctx);
    let structure = read_from_string(&mut ctx, &runtime, "#S(W12-POINT :X 8 :Y 9)", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(structure_ref(&ctx, structure, 0).unwrap(), Word::fixnum(8));
    assert_eq!(structure_ref(&ctx, structure, 1).unwrap(), Word::fixnum(9));

    for text in [
        "#S(W12-POINT :X 8 :X 9)",
        "#S(W12-POINT :X)",
        "#S(W12-POINT 1 2)",
        "#S(W12-POINT :UNKNOWN 1)",
    ] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, text, &opts).unwrap_err(),
            ReadError::StructureSyntax,
            "input: {text}"
        );
    }
}

#[test]
fn dispatch_character_radix_label_and_bit_boundaries_are_asserted() {
    let (runtime, mut ctx, opts) = setup();
    for (text, code) in [("#\\newline", 10), ("#\\escape", 27), ("#\\?", 63)] {
        let character = read_from_string(&mut ctx, &runtime, text, &opts)
            .unwrap()
            .unwrap();
        assert_eq!(character.as_character(), Some(code));
    }
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#\\bad-name", &opts).unwrap_err(),
        ReadError::UnknownCharacterName("bad-name".to_owned())
    );

    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#36rZ", &opts)
            .unwrap()
            .unwrap()
            .as_fixnum(),
        Some(35)
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#1r1", &opts).unwrap_err(),
        ReadError::InvalidBase(1)
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#2r", &opts).unwrap_err(),
        ReadError::InvalidNumber("".to_owned())
    );

    let labelled = read_from_string(&mut ctx, &runtime, "(#3=alpha #3#)", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, car(&ctx, labelled).unwrap()), "ALPHA");
    assert_eq!(
        symbol_text(&ctx, car(&ctx, cdr(&ctx, labelled).unwrap()).unwrap()),
        "ALPHA"
    );

    let bits = read_from_string(&mut ctx, &runtime, "#*0101x", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, bits),
        ObjectRef::SpecializedArray(_)
    ));
}

#[test]
fn package_markers_and_parse_integer_boundaries_preserve_contracts() {
    let (runtime, mut ctx, mut opts) = setup();
    let named = read_from_string(&mut ctx, &runtime, "COMMON-LISP:CAR", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, named), "CAR");
    let package_object = read_from_string(&mut ctx, &runtime, "COMMON-LISP::CAR", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, package_object), "CAR");
    let escaped = read_from_string(&mut ctx, &runtime, "A\\:B", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, escaped), "A:B");

    opts.set_read_base(ncl_reader::ReadBase::new(16).unwrap());
    let hex = read_from_string(&mut ctx, &runtime, "ff", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(classify(hex), ObjectRef::Fixnum(255));
    let (integer, index) =
        ncl_reader::parse_integer(&mut ctx, &runtime, "-2f tail", Some(16), None, None).unwrap();
    assert_eq!(integer.as_fixnum(), Some(-47));
    assert_eq!(index, 3);
    assert_eq!(
        ncl_reader::parse_integer(&mut ctx, &runtime, "+", None, None, None).unwrap_err(),
        ReadError::InvalidNumber("+".to_owned())
    );
    assert_eq!(
        ncl_reader::parse_integer(&mut ctx, &runtime, "99", Some(1), None, None).unwrap_err(),
        ReadError::InvalidBase(1)
    );
}
