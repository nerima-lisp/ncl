#![allow(clippy::unwrap_used, reason = "tests assert on builtin behavior")]
#![allow(missing_docs)]

use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, Package, Runtime, ThreadContext, Word,
    make_array, make_cons, make_double, make_simple_vector, make_specialized_array, make_string,
    symbol_name,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_types::register(&runtime).unwrap();
    ncl_types::builtins::register(&runtime).unwrap();
    (runtime, ctx)
}

fn function(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap()
}

fn intern(ctx: &mut ThreadContext, runtime: &Runtime, package: &str, name: &str) -> Word {
    let package = runtime.find_package(ctx, package).unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn list(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Word {
    values.iter().rev().fold(Word::NIL, |tail, value| {
        make_cons(ctx, runtime, *value, tail).unwrap()
    })
}

fn symbol_text(ctx: &ThreadContext, symbol: Word) -> String {
    let name = symbol_name(ctx, symbol).unwrap();
    (0..ncl_object::string_length(ctx, name).unwrap())
        .map(|index| ncl_object::string_ref(ctx, name, index).unwrap())
        .collect()
}

#[test]
fn type_of_and_keywordp_classify_runtime_objects() {
    let (runtime, mut ctx) = setup();
    let type_of = function(&runtime, &mut ctx, "TYPE-OF");
    let keywordp = function(&runtime, &mut ctx, "KEYWORDP");
    let string = make_string(&mut ctx, &runtime, &['x']).unwrap();
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1)]).unwrap();
    let bits = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(1)],
    )
    .unwrap();
    let array = make_array(
        &mut ctx,
        &runtime,
        &[1, 2],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap();
    let keyword = intern(&mut ctx, &runtime, "KEYWORD", "FLAG");
    let ordinary = intern(&mut ctx, &runtime, "COMMON-LISP", "FLAG");

    for (object, expected) in [
        (Word::NIL, "NULL"),
        (Word::fixnum(1), "FIXNUM"),
        (string, "SIMPLE-STRING"),
        (vector, "SIMPLE-VECTOR"),
        (bits, "SIMPLE-BIT-VECTOR"),
        (array, "ARRAY"),
    ] {
        let type_name = runtime.call_builtin(&mut ctx, type_of, &[object]).unwrap();
        assert_eq!(symbol_text(&ctx, type_name), expected);
    }
    assert_eq!(
        runtime.call_builtin(&mut ctx, keywordp, &[keyword]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, keywordp, &[ordinary]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, keywordp, &[Word::fixnum(1)]),
        Ok(Word::NIL)
    );
}

#[test]
fn coerce_converts_sequences_characters_and_reals() {
    let (runtime, mut ctx) = setup();
    let coerce = function(&runtime, &mut ctx, "COERCE");
    let list_type = intern(&mut ctx, &runtime, "COMMON-LISP", "LIST");
    let vector_type = intern(&mut ctx, &runtime, "COMMON-LISP", "VECTOR");
    let string_type = intern(&mut ctx, &runtime, "COMMON-LISP", "STRING");
    let character_type = intern(&mut ctx, &runtime, "COMMON-LISP", "CHARACTER");
    let float_type = intern(&mut ctx, &runtime, "COMMON-LISP", "DOUBLE-FLOAT");
    let chars = make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();
    let values = list(
        &mut ctx,
        &runtime,
        &[Word::character('x' as u32), Word::character('y' as u32)],
    );

    let as_list = runtime
        .call_builtin(&mut ctx, coerce, &[chars, list_type])
        .unwrap();
    assert!(as_list.is_cons());
    assert_eq!(
        ncl_object::car(&ctx, as_list).unwrap(),
        Word::character('a' as u32)
    );
    let as_vector = runtime
        .call_builtin(&mut ctx, coerce, &[values, vector_type])
        .unwrap();
    assert_eq!(
        ncl_object::simple_vector_length(&ctx, as_vector).unwrap(),
        2
    );
    let as_string = runtime
        .call_builtin(&mut ctx, coerce, &[values, string_type])
        .unwrap();
    assert_eq!(ncl_object::string_length(&ctx, as_string).unwrap(), 2);
    let one_character = make_string(&mut ctx, &runtime, &['z']).unwrap();
    let as_character = runtime
        .call_builtin(&mut ctx, coerce, &[one_character, character_type])
        .unwrap();
    assert_eq!(as_character, Word::character('z' as u32));
    let as_float = runtime
        .call_builtin(&mut ctx, coerce, &[Word::fixnum(-7), float_type])
        .unwrap();
    assert!(matches!(
        ncl_object::classify_object(&ctx, as_float),
        ncl_object::ObjectRef::DoubleFloat(_)
    ));
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[Word::fixnum(7), character_type]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn subtypep_reports_true_false_and_uncertain_results() {
    let (runtime, mut ctx) = setup();
    let subtypep = function(&runtime, &mut ctx, "SUBTYPEP");
    let integer = intern(&mut ctx, &runtime, "COMMON-LISP", "INTEGER");
    let number = intern(&mut ctx, &runtime, "COMMON-LISP", "NUMBER");
    let string = intern(&mut ctx, &runtime, "COMMON-LISP", "STRING");
    let unknown = intern(&mut ctx, &runtime, "COMMON-LISP", "NOT-A-TYPE");

    assert_eq!(
        runtime.call_builtin(&mut ctx, subtypep, &[integer, number]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, subtypep, &[string, integer]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, subtypep, &[unknown, integer]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, subtypep, &[Word::fixnum(1), integer]),
        Ok(Word::NIL)
    );
}

#[test]
fn coercion_preserves_already_matching_values_and_rejects_unsupported_specs() {
    let (runtime, mut ctx) = setup();
    let coerce = function(&runtime, &mut ctx, "COERCE");
    let integer = intern(&mut ctx, &runtime, "COMMON-LISP", "INTEGER");
    let function_type = intern(&mut ctx, &runtime, "COMMON-LISP", "FUNCTION");
    let value = Word::fixnum(42);
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[value, integer]),
        Ok(value)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[Word::NIL, function_type]),
        Err(ncl_object::ObjectError::TypeError)
    );
    let double = make_double(&mut ctx, &runtime, 2.5).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[double.into(), integer]),
        Err(ncl_object::ObjectError::TypeError)
    );
}
