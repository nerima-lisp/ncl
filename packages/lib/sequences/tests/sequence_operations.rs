#![allow(
    missing_docs,
    clippy::missing_const_for_fn,
    clippy::too_many_lines,
    clippy::unwrap_used
)]

use std::collections::HashMap;

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ArrayElementType, FunctionObject, ObjectError, Package, Runtime, ThreadContext, Word,
    make_bignum_from_i128, make_complex, make_double, make_ratio, make_simple_vector,
    make_specialized_array, make_string,
};

fn setup() -> (Runtime, ThreadContext, HashMap<String, FunctionObject>) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_sequences::register(&runtime).unwrap();
    let names = [
        "LIST",
        "LIST*",
        "APPEND",
        "NCONC",
        "CONCATENATE",
        "LENGTH",
        "COPY-SEQ",
        "REVERSE",
        "NREVERSE",
        "ELT",
        "SUBSEQ",
        "GETF",
        "EQUAL",
        "EQUALP",
        "FIND",
        "POSITION",
        "COUNT",
        "SEARCH",
        "MISMATCH",
        "FILL",
        "REPLACE",
        "REDUCE",
        "CAR",
    ];
    let functions = names
        .into_iter()
        .map(|name| {
            let word = runtime.function(&mut ctx, "COMMON-LISP", name).unwrap();
            (name.to_owned(), FunctionObject::try_from(word).unwrap())
        })
        .collect();
    (runtime, ctx, functions)
}

#[test]
fn destructive_sequence_operations_honor_ranges_and_sequence_types() {
    let (runtime, mut ctx, functions) = setup();
    let one = Word::fixnum(1);
    let nine = Word::fixnum(9);
    let start = keyword(&mut ctx, &runtime, "START");
    let end = keyword(&mut ctx, &runtime, "END");

    let source_list = list(&runtime, &mut ctx, &functions, &[one, one, one, one]);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "FILL",
            &[
                source_list,
                nine,
                start,
                Word::fixnum(1),
                end,
                Word::fixnum(3)
            ],
        )
        .unwrap(),
        source_list
    );
    assert_eq!(values(&ctx, source_list), vec![one, nine, nine, one]);

    let vector = ncl_object::make_simple_vector(&mut ctx, &runtime, &[one, one, one]).unwrap();
    let replacement = list(&runtime, &mut ctx, &functions, &[nine, nine]);
    call(
        &runtime,
        &mut ctx,
        &functions,
        "REPLACE",
        &[vector, replacement, start, Word::fixnum(1)],
    )
    .unwrap();
    assert_eq!(
        (0..3)
            .map(|index| ncl_object::simple_vector_ref(&ctx, vector, index).unwrap())
            .collect::<Vec<_>>(),
        vec![one, nine, nine]
    );

    let text = ncl_object::make_string(&mut ctx, &runtime, &['a', 'a', 'a']).unwrap();
    call(
        &runtime,
        &mut ctx,
        &functions,
        "FILL",
        &[
            text,
            Word::character(u32::from('z')),
            start,
            Word::fixnum(1),
        ],
    )
    .unwrap();
    assert_eq!(
        (0..3)
            .map(|index| ncl_object::string_ref(&ctx, text, index).unwrap())
            .collect::<String>(),
        "azz"
    );
}

#[test]
fn equality_distinguishes_array_shapes_and_non_bit_specializations() {
    let (runtime, mut ctx, functions) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let short = ncl_object::make_simple_vector(&mut ctx, &runtime, &[one]).unwrap();
    let long = ncl_object::make_simple_vector(&mut ctx, &runtime, &[one, two]).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUALP", &[short, long]).unwrap(),
        Word::NIL
    );

    let left =
        make_specialized_array(&mut ctx, &runtime, ArrayElementType::Fixnum, &[one, two]).unwrap();
    let right =
        make_specialized_array(&mut ctx, &runtime, ArrayElementType::Fixnum, &[one, two]).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUAL", &[left, right]).unwrap(),
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUALP", &[left, right]).unwrap(),
        Word::TRUE
    );

    let mismatched_table = HashTable::new(&mut ctx, &runtime, HashTest::Equalp, Weakness::None)
        .unwrap()
        .as_word();
    HashTable::from_word(mismatched_table)
        .insert(&mut ctx, &runtime, one, two)
        .unwrap();
    let empty_table = HashTable::new(&mut ctx, &runtime, HashTest::Equalp, Weakness::None)
        .unwrap()
        .as_word();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUALP",
            &[mismatched_table, empty_table],
        )
        .unwrap(),
        Word::NIL
    );
}

#[test]
fn selection_rejects_dotted_and_non_sequence_arguments() {
    let (runtime, mut ctx, functions) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let dotted = ncl_object::make_cons(&mut ctx, &runtime, one, two).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "FIND", &[one, dotted]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "FIND",
            &[one, Word::fixnum(99)],
        ),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn reduce_applies_key_ranges_direction_and_initial_value() {
    let (runtime, mut ctx, functions) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let source = list(&runtime, &mut ctx, &functions, &[one, two, three]);
    let reduce = [
        functions["LIST"].as_word(),
        source,
        keyword(&mut ctx, &runtime, "INITIAL-VALUE"),
        Word::fixnum(0),
        keyword(&mut ctx, &runtime, "FROM-END"),
        Word::TRUE,
        keyword(&mut ctx, &runtime, "START"),
        Word::fixnum(1),
        keyword(&mut ctx, &runtime, "END"),
        Word::fixnum(3),
    ];
    let result = call(&runtime, &mut ctx, &functions, "REDUCE", &reduce).unwrap();
    assert_eq!(ncl_object::car(&ctx, result).unwrap(), two);
    let first = ncl_object::car(&ctx, ncl_object::cdr(&ctx, result).unwrap()).unwrap();
    assert_eq!(ncl_object::car(&ctx, first), Ok(three));
    let initial = ncl_object::cdr(&ctx, first).unwrap();
    assert_eq!(ncl_object::car(&ctx, initial), Ok(Word::fixnum(0)));
    assert_eq!(ncl_object::cdr(&ctx, initial).unwrap(), Word::NIL);

    let key = functions["CAR"].as_word();
    let keyed_one = list(&runtime, &mut ctx, &functions, &[one]);
    let keyed_two = list(&runtime, &mut ctx, &functions, &[two]);
    let keyed_source = list(&runtime, &mut ctx, &functions, &[keyed_one, keyed_two]);
    let key_keyword = keyword(&mut ctx, &runtime, "KEY");
    let end_keyword = keyword(&mut ctx, &runtime, "END");
    let keyed_result = call(
        &runtime,
        &mut ctx,
        &functions,
        "REDUCE",
        &[
            functions["LIST"].as_word(),
            keyed_source,
            key_keyword,
            key,
            end_keyword,
            Word::NIL,
        ],
    )
    .unwrap();
    assert_eq!(ncl_object::car(&ctx, keyed_result).unwrap(), one);
    let keyed_tail = ncl_object::cdr(&ctx, keyed_result).unwrap();
    assert_eq!(ncl_object::car(&ctx, keyed_tail), Ok(two));
    assert_eq!(ncl_object::cdr(&ctx, keyed_tail).unwrap(), Word::NIL);
}

fn call(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    functions: &HashMap<String, FunctionObject>,
    name: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let function = functions[name];
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        runtime.call_builtin(
            ctx,
            function,
            &roots.iter().map(|root| **root).collect::<Vec<_>>(),
        )
    })
}

fn list(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    functions: &HashMap<String, FunctionObject>,
    values: &[Word],
) -> Word {
    call(runtime, ctx, functions, "LIST", values).unwrap()
}

fn values(ctx: &ThreadContext, mut list: Word) -> Vec<Word> {
    let mut result = Vec::new();
    while list != Word::NIL {
        result.push(ncl_object::car(ctx, list).unwrap());
        list = ncl_object::cdr(ctx, list).unwrap();
    }
    result
}

fn keyword(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
    let package = runtime.find_package(ctx, "KEYWORD").unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

#[test]
fn list_and_sequence_builtins_preserve_values_and_mutation() {
    let (runtime, mut ctx, functions) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let left = list(&runtime, &mut ctx, &functions, &[one, two]);
    let right = list(&runtime, &mut ctx, &functions, &[three]);
    let dotted = call(&runtime, &mut ctx, &functions, "LIST*", &[one, two, three]).unwrap();
    assert_eq!(ncl_object::car(&ctx, dotted).unwrap(), one);
    let dotted_tail = ncl_object::cdr(&ctx, dotted).unwrap();
    assert_eq!(ncl_object::car(&ctx, dotted_tail).unwrap(), two);
    assert_eq!(
        ncl_object::cdr(&ctx, ncl_object::cdr(&ctx, dotted).unwrap()).unwrap(),
        three
    );

    let appended = call(&runtime, &mut ctx, &functions, "APPEND", &[left, right]).unwrap();
    assert_eq!(values(&ctx, appended), vec![one, two, three]);
    let nconc = call(&runtime, &mut ctx, &functions, "NCONC", &[left, right]).unwrap();
    assert_eq!(values(&ctx, nconc), vec![one, two, three]);
    assert_eq!(
        ncl_object::cdr(&ctx, left).unwrap(),
        ncl_object::cdr(&ctx, nconc).unwrap()
    );

    let list_type = keyword(&mut ctx, &runtime, "LIST");
    let vector_type = keyword(&mut ctx, &runtime, "VECTOR");
    let string_type = keyword(&mut ctx, &runtime, "STRING");
    let concat_left = list(&runtime, &mut ctx, &functions, &[one, two]);
    let concat_right = list(&runtime, &mut ctx, &functions, &[three]);
    let concatenated_list = call(
        &runtime,
        &mut ctx,
        &functions,
        "CONCATENATE",
        &[list_type, concat_left, concat_right],
    )
    .unwrap();
    assert_eq!(values(&ctx, concatenated_list), vec![one, two, three]);
    let vector = call(
        &runtime,
        &mut ctx,
        &functions,
        "CONCATENATE",
        &[vector_type, concat_left, concat_right],
    )
    .unwrap();
    assert_eq!(ncl_object::simple_vector_length(&ctx, vector).unwrap(), 3);
    let chars = [
        Word::character(u32::from('a')),
        Word::character(u32::from('b')),
    ];
    let char_list = list(&runtime, &mut ctx, &functions, &chars);
    let string = call(
        &runtime,
        &mut ctx,
        &functions,
        "CONCATENATE",
        &[string_type, char_list],
    )
    .unwrap();
    assert_eq!(ncl_object::string_length(&ctx, string).unwrap(), 2);
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "LENGTH", &[string]).unwrap(),
        Word::fixnum(2)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "ELT",
            &[string, Word::fixnum(1)]
        )
        .unwrap(),
        chars[1]
    );
    let subseq = call(
        &runtime,
        &mut ctx,
        &functions,
        "SUBSEQ",
        &[vector, Word::fixnum(1), Word::fixnum(3)],
    )
    .unwrap();
    assert_eq!(ncl_object::simple_vector_length(&ctx, subseq).unwrap(), 2);
    assert_eq!(ncl_object::simple_vector_ref(&ctx, subseq, 0).unwrap(), two);
    assert_eq!(
        ncl_object::simple_vector_ref(&ctx, subseq, 1).unwrap(),
        three
    );
    let reverse_source = list(&runtime, &mut ctx, &functions, &[one, two]);
    let reversed = call(&runtime, &mut ctx, &functions, "REVERSE", &[reverse_source]).unwrap();
    assert_eq!(values(&ctx, reversed), vec![two, one]);
    let in_place_source = list(&runtime, &mut ctx, &functions, &[one, two]);
    let in_place = call(
        &runtime,
        &mut ctx,
        &functions,
        "NREVERSE",
        &[in_place_source],
    )
    .unwrap();
    assert_eq!(values(&ctx, in_place), vec![two, one]);

    let plist = list(
        &runtime,
        &mut ctx,
        &functions,
        &[one, two, three, Word::fixnum(4)],
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "GETF", &[plist, one]).unwrap(),
        two
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "GETF",
            &[plist, Word::fixnum(9), three]
        )
        .unwrap(),
        three
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "ELT",
            &[one, Word::fixnum(0)]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "SUBSEQ",
            &[vector, Word::fixnum(3), Word::fixnum(1)]
        ),
        Err(ObjectError::TypeError)
    );
    let improper = call(&runtime, &mut ctx, &functions, "LIST*", &[one, two, three]).unwrap();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "APPEND",
            &[improper, Word::NIL],
        ),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn selection_options_search_and_mismatch_return_precise_indices() {
    let (runtime, mut ctx, functions) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let four = Word::fixnum(4);
    let sequence = list(&runtime, &mut ctx, &functions, &[one, two, three, two]);
    let start = keyword(&mut ctx, &runtime, "START");
    let end = keyword(&mut ctx, &runtime, "END");
    let from_end = keyword(&mut ctx, &runtime, "FROM-END");
    let count = keyword(&mut ctx, &runtime, "COUNT");
    let test = keyword(&mut ctx, &runtime, "TEST");
    let equal = runtime.function(&mut ctx, "COMMON-LISP", "EQUAL").unwrap();
    let equal = FunctionObject::try_from(equal).unwrap().as_word();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "FIND",
            &[
                two,
                sequence,
                start,
                one,
                end,
                four,
                from_end,
                Word::TRUE,
                test,
                equal
            ]
        )
        .unwrap(),
        two
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "POSITION",
            &[
                two,
                sequence,
                start,
                one,
                end,
                four,
                from_end,
                Word::TRUE,
                count,
                one,
                test,
                equal
            ]
        )
        .unwrap(),
        Word::fixnum(3)
    );
    let nested_one = list(&runtime, &mut ctx, &functions, &[one]);
    let nested_two = list(&runtime, &mut ctx, &functions, &[two]);
    let nested_three = list(&runtime, &mut ctx, &functions, &[three]);
    let nested = list(
        &runtime,
        &mut ctx,
        &functions,
        &[nested_one, nested_two, nested_three],
    );
    let car = runtime.function(&mut ctx, "COMMON-LISP", "CAR").unwrap();
    let car = FunctionObject::try_from(car).unwrap().as_word();
    let key = keyword(&mut ctx, &runtime, "KEY");
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "FIND",
            &[two, nested, key, car, test, equal],
        )
        .unwrap(),
        nested_two
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "COUNT",
            &[two, sequence, test, equal]
        )
        .unwrap(),
        Word::fixnum(2)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "SEARCH",
            &[sequence, sequence, from_end, Word::TRUE, test, equal]
        )
        .unwrap(),
        Word::fixnum(0)
    );
    let shorter = list(&runtime, &mut ctx, &functions, &[one, two, four]);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "MISMATCH",
            &[sequence, shorter, test, equal],
        )
        .unwrap(),
        Word::fixnum(2)
    );
    let test_not = keyword(&mut ctx, &runtime, "TEST-NOT");
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "FIND",
            &[one, sequence, test, equal, test_not, equal]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "POSITION",
            &[one, sequence, start, Word::fixnum(9)]
        ),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn equality_covers_numeric_array_and_hash_table_content() {
    let (runtime, mut ctx, functions) = setup();
    let big = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 70)
        .unwrap()
        .as_word();
    let same_big = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 70)
        .unwrap()
        .as_word();
    let ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(2), Word::fixnum(3))
        .unwrap()
        .as_word();
    let same_ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(2), Word::fixnum(3))
        .unwrap()
        .as_word();
    let double = make_double(&mut ctx, &runtime, 2.0).unwrap().as_word();
    let complex = make_complex(&mut ctx, &runtime, Word::fixnum(2), Word::fixnum(0))
        .unwrap()
        .as_word();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUAL", &[big, same_big]).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUAL",
            &[ratio, same_ratio]
        )
        .unwrap(),
        Word::TRUE
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUALP",
            &[Word::fixnum(2), double]
        )
        .unwrap(),
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUALP", &[complex, double]).unwrap(),
        Word::TRUE
    );
    let other_double = make_double(&mut ctx, &runtime, 2.5).unwrap().as_word();
    let nonzero_complex = make_complex(&mut ctx, &runtime, Word::fixnum(2), Word::fixnum(1))
        .unwrap()
        .as_word();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUALP",
            &[double, other_double],
        )
        .unwrap(),
        Word::NIL
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUALP",
            &[nonzero_complex, double],
        )
        .unwrap(),
        Word::NIL
    );
    let vector =
        make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(2)]).unwrap();
    let other_vector =
        make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(2)]).unwrap();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUALP",
            &[vector, other_vector]
        )
        .unwrap(),
        Word::TRUE
    );
    let bits = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(1), Word::fixnum(0)],
    )
    .unwrap();
    let same_bits = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(1), Word::fixnum(0)],
    )
    .unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUAL", &[bits, same_bits]).unwrap(),
        Word::TRUE
    );
    let left_string = make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();
    let right_string = make_string(&mut ctx, &runtime, &['A', 'B']).unwrap();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUALP",
            &[left_string, right_string]
        )
        .unwrap(),
        Word::TRUE
    );
    let left_table = HashTable::new(&mut ctx, &runtime, HashTest::Equalp, Weakness::None)
        .unwrap()
        .as_word();
    let right_table = HashTable::new(&mut ctx, &runtime, HashTest::Equalp, Weakness::None)
        .unwrap()
        .as_word();
    HashTable::from_word(left_table)
        .insert(&mut ctx, &runtime, left_string, one_value())
        .unwrap();
    HashTable::from_word(right_table)
        .insert(&mut ctx, &runtime, left_string, one_value())
        .unwrap();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUALP",
            &[left_table, right_table]
        )
        .unwrap(),
        Word::TRUE
    );
}

fn one_value() -> Word {
    Word::fixnum(1)
}
