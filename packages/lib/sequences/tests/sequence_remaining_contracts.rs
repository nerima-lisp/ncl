#![allow(
    clippy::similar_names,
    clippy::too_many_lines,
    missing_docs,
    clippy::unwrap_used
)]

use std::collections::HashMap;

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, ObjectError, Package, Runtime, ThreadContext,
    Word, make_array, make_bignum_from_i128, make_complex, make_double, make_ratio,
    make_simple_vector, make_specialized_array, make_string,
};

fn setup() -> (Runtime, ThreadContext, HashMap<String, FunctionObject>) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_sequences::register(&runtime).unwrap();
    let names = [
        "ADJOIN",
        "ATOM",
        "CAR",
        "CDR",
        "CONCATENATE",
        "CONS",
        "CONSP",
        "COPY-LIST",
        "COPY-SEQ",
        "COUNT",
        "ELT",
        "ENDP",
        "EQUAL",
        "EQUALP",
        "FIND",
        "FIND-IF",
        "INTERSECTION",
        "LENGTH",
        "LIST",
        "LISTP",
        "MEMBER",
        "MISMATCH",
        "NREVERSE",
        "POSITION",
        "POSITION-IF-NOT",
        "REMOVE",
        "REVERSE",
        "SEARCH",
        "SET-DIFFERENCE",
        "SET-EXCLUSIVE-OR",
        "SORT",
        "SUBSEQ",
        "SUBSETP",
        "SUBSTITUTE",
        "UNION",
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

fn list_values(ctx: &ThreadContext, mut value: Word) -> Vec<Word> {
    let mut result = Vec::new();
    while value != Word::NIL {
        result.push(ncl_object::car(ctx, value).unwrap());
        value = ncl_object::cdr(ctx, value).unwrap();
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

fn common_lisp_symbol(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
    let package = runtime.find_package(ctx, "COMMON-LISP").unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn vector_values(ctx: &ThreadContext, value: Word) -> Vec<Word> {
    (0..ncl_object::simple_vector_length(ctx, value).unwrap())
        .map(|index| ncl_object::simple_vector_ref(ctx, value, index).unwrap())
        .collect()
}

#[test]
fn selection_options_and_sequence_types_return_exact_values_and_errors() {
    let (runtime, mut ctx, functions) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let source = list(&runtime, &mut ctx, &functions, &[one, two, one]);
    let vector = make_simple_vector(&mut ctx, &runtime, &[one, two, one]).unwrap();
    let text = make_string(&mut ctx, &runtime, &['a', 'b', 'a']).unwrap();
    let start = keyword(&mut ctx, &runtime, "START");
    let end = keyword(&mut ctx, &runtime, "END");
    let from_end = keyword(&mut ctx, &runtime, "FROM-END");
    let count = keyword(&mut ctx, &runtime, "COUNT");
    let test = keyword(&mut ctx, &runtime, "TEST");
    let test_not = keyword(&mut ctx, &runtime, "TEST-NOT");
    let equal = functions["EQUAL"].as_word();

    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "FIND",
            &[two, vector, start, one, end, three, from_end, Word::NIL],
        ),
        Ok(two)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "POSITION",
            &[
                Word::character(u32::from('b')),
                text,
                start,
                Word::fixnum(0),
                end,
                three
            ],
        ),
        Ok(Word::fixnum(1))
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "REMOVE",
            &[one, vector, from_end, Word::TRUE, count, one],
        )
        .map(|value| list_values(&ctx, value)),
        Ok(vec![one, two])
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "SUBSTITUTE",
            &[
                Word::character(u32::from('x')),
                Word::character(u32::from('a')),
                text,
                test_not,
                equal,
            ],
        )
        .map(|value| list_values(&ctx, value)),
        Ok(vec![
            Word::character(u32::from('a')),
            Word::character(u32::from('x')),
            Word::character(u32::from('a'))
        ])
    );

    let atom = functions["ATOM"].as_word();
    let consp = functions["CONSP"].as_word();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "FIND-IF", &[atom, text]),
        Ok(Word::character(u32::from('a')))
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "POSITION-IF-NOT",
            &[consp, vector],
        ),
        Ok(Word::fixnum(0))
    );

    let left = list(&runtime, &mut ctx, &functions, &[one, two]);
    let search_right =
        make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(9), one, two, one, two]).unwrap();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "SEARCH",
            &[
                left,
                search_right,
                start,
                Word::fixnum(0),
                end,
                two,
                from_end,
                Word::TRUE
            ],
        ),
        Ok(Word::fixnum(3))
    );

    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "FIND",
            &[one, source, start]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "FIND",
            &[one, source, start, Word::character(u32::from('x'))],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "FIND",
            &[one, source, count, Word::fixnum(-1)],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "FIND",
            &[one, source, start, Word::fixnum(4)],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "FIND",
            &[one, source, test_not, equal, test, equal],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "FIND",
            &[one, source, test, one],
        ),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn equality_numeric_array_and_hash_paths_return_concrete_booleans() {
    let (runtime, mut ctx, functions) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let text = make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();
    let same_text = make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();
    let other_text = make_string(&mut ctx, &runtime, &['a', 'c']).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUAL", &[text, same_text]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUAL", &[text, other_text]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUALP",
            &[
                Word::character(u32::from('a')),
                Word::character(u32::from('A'))
            ],
        ),
        Ok(Word::TRUE)
    );

    let vector = make_simple_vector(&mut ctx, &runtime, &[one, two]).unwrap();
    let same_vector = make_simple_vector(&mut ctx, &runtime, &[one, two]).unwrap();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUAL",
            &[vector, same_vector]
        ),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUALP",
            &[vector, same_vector]
        ),
        Ok(Word::TRUE)
    );

    let array_options = ArrayOptions {
        element_type: ArrayElementType::T,
        initial_element: one,
        adjustable: false,
        fill_pointer: None,
        displaced_to: None,
        displaced_index_offset: 0,
    };
    let array = make_array(&mut ctx, &runtime, &[2], array_options).unwrap();
    let same_array = make_array(&mut ctx, &runtime, &[2], array_options).unwrap();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUALP",
            &[array, same_array]
        ),
        Ok(Word::TRUE)
    );
    let bits = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(0), Word::fixnum(1)],
    )
    .unwrap();
    let other_bits = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(0), Word::fixnum(0)],
    )
    .unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUAL", &[bits, other_bits]),
        Ok(Word::NIL)
    );

    let negative_big = make_bignum_from_i128(&mut ctx, &runtime, -(1_i128 << 70))
        .unwrap()
        .as_word();
    let negative_double = make_double(&mut ctx, &runtime, -(2_f64.powi(70)))
        .unwrap()
        .as_word();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUALP",
            &[negative_big, negative_double],
        ),
        Ok(Word::TRUE)
    );
    let ratio = make_ratio(&mut ctx, &runtime, two, one).unwrap().as_word();
    let complex = make_complex(&mut ctx, &runtime, two, Word::fixnum(0))
        .unwrap()
        .as_word();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUALP", &[ratio, complex]),
        Ok(Word::TRUE)
    );

    let left = HashTable::new(&mut ctx, &runtime, HashTest::Equalp, Weakness::None)
        .unwrap()
        .as_word();
    let right = HashTable::new(&mut ctx, &runtime, HashTest::Equalp, Weakness::None)
        .unwrap()
        .as_word();
    HashTable::from_word(left)
        .insert(&mut ctx, &runtime, one, two)
        .unwrap();
    HashTable::from_word(right)
        .insert(&mut ctx, &runtime, one, two)
        .unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUALP", &[left, right]),
        Ok(Word::TRUE)
    );
    HashTable::from_word(right)
        .insert(&mut ctx, &runtime, one, Word::fixnum(9))
        .unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUALP", &[left, right]),
        Ok(Word::NIL)
    );
    let different_test = HashTable::new(&mut ctx, &runtime, HashTest::Eq, Weakness::None)
        .unwrap()
        .as_word();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUALP",
            &[left, different_test],
        ),
        Ok(Word::NIL)
    );
}

#[test]
fn list_operations_cover_list_vector_string_results_and_error_paths() {
    let (runtime, mut ctx, functions) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let values = list(&runtime, &mut ctx, &functions, &[one, two, three]);
    let vector = make_simple_vector(&mut ctx, &runtime, &[one, two, three]).unwrap();
    let text = make_string(&mut ctx, &runtime, &['a', 'b', 'c']).unwrap();

    assert_eq!(
        call(&runtime, &mut ctx, &functions, "ATOM", &[one]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "CONSP", &[one]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "LISTP", &[Word::NIL]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "ENDP", &[Word::NIL]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "CAR", &[Word::NIL]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "CDR", &[Word::NIL]),
        Ok(Word::NIL)
    );

    for sequence in [values, vector, text] {
        assert_eq!(
            call(&runtime, &mut ctx, &functions, "LENGTH", &[sequence]),
            Ok(Word::fixnum(3))
        );
    }
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "ELT",
            &[vector, Word::fixnum(1)]
        ),
        Ok(two)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "ELT",
            &[text, Word::fixnum(2)]
        ),
        Ok(Word::character(u32::from('c')))
    );
    let copied_text = call(&runtime, &mut ctx, &functions, "COPY-SEQ", &[text]).unwrap();
    assert_eq!(ncl_object::string_length(&ctx, copied_text).unwrap(), 3);
    let copied_list = call(&runtime, &mut ctx, &functions, "COPY-LIST", &[values]).unwrap();
    assert_eq!(list_values(&ctx, copied_list), vec![one, two, three]);
    let reversed_vector = call(&runtime, &mut ctx, &functions, "REVERSE", &[vector]).unwrap();
    assert_eq!(vector_values(&ctx, reversed_vector), vec![three, two, one]);
    let reversed_text = call(&runtime, &mut ctx, &functions, "REVERSE", &[text]).unwrap();
    assert_eq!(
        (0..3)
            .map(|index| ncl_object::string_ref(&ctx, reversed_text, index).unwrap())
            .collect::<String>(),
        "cba"
    );
    let list_subseq = call(
        &runtime,
        &mut ctx,
        &functions,
        "SUBSEQ",
        &[values, Word::fixnum(1)],
    )
    .unwrap();
    assert_eq!(list_values(&ctx, list_subseq), vec![two, three]);
    let text_subseq = call(
        &runtime,
        &mut ctx,
        &functions,
        "SUBSEQ",
        &[text, Word::fixnum(1), Word::fixnum(3)],
    )
    .unwrap();
    assert_eq!(
        (0..2)
            .map(|index| ncl_object::string_ref(&ctx, text_subseq, index).unwrap())
            .collect::<String>(),
        "bc"
    );

    let nil_type = common_lisp_symbol(&mut ctx, &runtime, "NIL");
    let string_type = common_lisp_symbol(&mut ctx, &runtime, "STRING");
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "CONCATENATE",
            &[nil_type, values]
        ),
        Ok(Word::NIL)
    );
    let invalid_string = list(&runtime, &mut ctx, &functions, &[one]);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "CONCATENATE",
            &[string_type, invalid_string],
        ),
        Err(ObjectError::TypeError)
    );

    let dotted = ncl_object::make_cons(&mut ctx, &runtime, one, two).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "COPY-LIST", &[dotted]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "NREVERSE", &[dotted]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn ordered_set_options_and_vector_sort_return_exact_results_and_errors() {
    let (runtime, mut ctx, functions) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let left = make_simple_vector(&mut ctx, &runtime, &[one, two, two]).unwrap();
    let right = make_simple_vector(&mut ctx, &runtime, &[two, three]).unwrap();
    let test = keyword(&mut ctx, &runtime, "TEST");
    let equal = functions["EQUAL"].as_word();

    for (name, expected) in [
        ("UNION", vec![one, two, three]),
        ("INTERSECTION", vec![two]),
        ("SET-DIFFERENCE", vec![one]),
        ("SET-EXCLUSIVE-OR", vec![one, three]),
    ] {
        let result = call(
            &runtime,
            &mut ctx,
            &functions,
            name,
            &[left, right, test, equal],
        )
        .unwrap();
        assert_eq!(list_values(&ctx, result), expected, "{name}");
    }
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "SUBSETP",
            &[left, right, test, equal],
        ),
        Ok(Word::NIL)
    );

    let member_values = list(&runtime, &mut ctx, &functions, &[one, two]);
    let test_not = keyword(&mut ctx, &runtime, "TEST-NOT");
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "MEMBER",
            &[one, member_values, test_not, equal],
        )
        .map(|value| list_values(&ctx, value)),
        Ok(vec![two])
    );

    let pairs_one = list(&runtime, &mut ctx, &functions, &[one, Word::fixnum(10)]);
    let pairs_two = list(&runtime, &mut ctx, &functions, &[two, Word::fixnum(20)]);
    let pairs = list(&runtime, &mut ctx, &functions, &[pairs_one, pairs_two]);
    let key = keyword(&mut ctx, &runtime, "KEY");
    let car = functions["CAR"].as_word();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "ADJOIN",
            &[pairs_one, pairs, key, car, test, equal],
        ),
        Ok(pairs)
    );

    let sort_vector = make_simple_vector(&mut ctx, &runtime, &[three, one, two]).unwrap();
    let cons = functions["CONS"].as_word();
    let sorted = call(&runtime, &mut ctx, &functions, "SORT", &[sort_vector, cons]);
    assert_eq!(sorted, Ok(sort_vector));
    assert_eq!(vector_values(&ctx, sort_vector), vec![two, one, three]);

    let string = make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "UNION", &[string, left]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "UNION",
            &[left, right, test],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "UNION",
            &[left, right, test, equal, test_not, equal,],
        ),
        Err(ObjectError::TypeError)
    );
}
