#![allow(missing_docs, clippy::too_many_lines, clippy::unwrap_used)]

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, ObjectError, Package, Runtime, ThreadContext,
    Word, make_array, make_bignum_from_i128, make_complex, make_double, make_ratio,
    make_simple_vector, make_specialized_array, make_string,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_sequences::register(&runtime).unwrap();
    (runtime, ctx)
}

fn call(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let function = runtime.function(ctx, "COMMON-LISP", name).unwrap();
    let function = FunctionObject::try_from(function).unwrap();
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        runtime.call_builtin(
            ctx,
            function,
            &roots.iter().map(|root| **root).collect::<Vec<_>>(),
        )
    })
}

fn list(runtime: &Runtime, ctx: &mut ThreadContext, values: &[Word]) -> Word {
    call(runtime, ctx, "LIST", values).unwrap()
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
fn selection_variants_cover_direction_count_predicates_and_boundaries() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let four = Word::fixnum(4);
    let source = list(&runtime, &mut ctx, &[one, two, one, three]);
    let start = keyword(&mut ctx, &runtime, "START");
    let end = keyword(&mut ctx, &runtime, "END");
    let from_end = keyword(&mut ctx, &runtime, "FROM-END");
    let count = keyword(&mut ctx, &runtime, "COUNT");
    let test = keyword(&mut ctx, &runtime, "TEST");
    let test_not = keyword(&mut ctx, &runtime, "TEST-NOT");
    let equal = runtime.function(&mut ctx, "COMMON-LISP", "EQUAL").unwrap();
    let equal = FunctionObject::try_from(equal).unwrap().as_word();
    let atom = runtime.function(&mut ctx, "COMMON-LISP", "ATOM").unwrap();
    let atom = FunctionObject::try_from(atom).unwrap().as_word();
    let cons = runtime.function(&mut ctx, "COMMON-LISP", "CONS").unwrap();
    let cons = FunctionObject::try_from(cons).unwrap().as_word();

    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "FIND",
            &[one, source, from_end, Word::TRUE, count, one],
        ),
        Ok(one)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "POSITION",
            &[one, source, start, one, end, four, test, equal],
        ),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        call(&runtime, &mut ctx, "COUNT", &[one, source, test_not, equal],),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        call(&runtime, &mut ctx, "FIND-IF", &[atom, source]),
        Ok(one)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "FIND-IF-NOT", &[atom, source]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "POSITION-IF", &[atom, source]),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        call(&runtime, &mut ctx, "POSITION-IF-NOT", &[atom, source]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "COUNT-IF", &[atom, source]),
        Ok(Word::fixnum(4))
    );
    assert_eq!(
        call(&runtime, &mut ctx, "COUNT-IF-NOT", &[atom, source]),
        Ok(Word::fixnum(0))
    );

    let left = list(&runtime, &mut ctx, &[one, two]);
    let right = list(&runtime, &mut ctx, &[three, one, two]);
    assert_eq!(
        call(&runtime, &mut ctx, "SEARCH", &[left, right, test, equal]),
        Ok(Word::fixnum(1))
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SEARCH",
            &[left, right, from_end, Word::TRUE, test, equal],
        ),
        Ok(Word::fixnum(1))
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MISMATCH",
            &[source, source, test, equal],
        ),
        Ok(Word::NIL)
    );
    let different = list(&runtime, &mut ctx, &[two, one]);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MISMATCH",
            &[source, different, test, equal],
        ),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MISMATCH",
            &[left, source, start, one, end, two, test, equal],
        ),
        Ok(Word::fixnum(2))
    );

    let nested_one = list(&runtime, &mut ctx, &[one]);
    let nested_two = list(&runtime, &mut ctx, &[two]);
    let nested = list(&runtime, &mut ctx, &[nested_one, nested_two]);
    let car = runtime.function(&mut ctx, "COMMON-LISP", "CAR").unwrap();
    let car = FunctionObject::try_from(car).unwrap().as_word();
    let key = keyword(&mut ctx, &runtime, "KEY");
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "FIND",
            &[two, nested, key, car, test, equal]
        ),
        Ok(nested_two)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "REMOVE", &[one, source, test, equal],)
            .map(|value| list_values(&ctx, value)),
        Ok(vec![two, three])
    );
    assert_eq!(
        call(&runtime, &mut ctx, "REMOVE-IF-NOT", &[atom, source],)
            .map(|value| list_values(&ctx, value)),
        Ok(vec![one, two, one, three])
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SUBSTITUTE",
            &[
                four,
                one,
                source,
                count,
                one,
                from_end,
                Word::TRUE,
                test,
                equal
            ],
        )
        .map(|value| list_values(&ctx, value)),
        Ok(vec![one, two, four, three])
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SUBSTITUTE-IF-NOT",
            &[four, atom, source],
        )
        .map(|value| list_values(&ctx, value)),
        Ok(vec![one, two, one, three])
    );
    assert_eq!(
        call(&runtime, &mut ctx, "DELETE-IF", &[atom, source]),
        Ok(Word::NIL)
    );

    assert_eq!(
        call(&runtime, &mut ctx, "FIND", &[one, source, start]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "FIND",
            &[one, source, test, equal, test_not, equal],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SEARCH", &[source, Word::fixnum(9)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MISMATCH", &[source, Word::fixnum(9)]),
        Err(ObjectError::TypeError)
    );
    let _ = cons;
}

#[test]
fn equality_variants_cover_content_keys_numeric_edges_and_fallbacks() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let text = make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();
    let longer = make_string(&mut ctx, &runtime, &['a', 'b', 'c']).unwrap();
    let folded_text = make_string(&mut ctx, &runtime, &['A', 'B']).unwrap();
    let car_symbol = common_lisp_symbol(&mut ctx, &runtime, "CAR");
    assert_eq!(
        call(&runtime, &mut ctx, "EQUAL", &[text, longer]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "EQUALP", &[text, folded_text]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "EQUALP", &[car_symbol, one]),
        Ok(Word::NIL)
    );

    let ratio_left = make_ratio(&mut ctx, &runtime, two, three)
        .unwrap()
        .as_word();
    let ratio_right = make_ratio(&mut ctx, &runtime, two, Word::fixnum(4))
        .unwrap()
        .as_word();
    assert_eq!(
        call(&runtime, &mut ctx, "EQUAL", &[ratio_left, ratio_right]),
        Ok(Word::NIL)
    );
    let complex_left = make_complex(&mut ctx, &runtime, two, one)
        .unwrap()
        .as_word();
    let complex_right = make_complex(&mut ctx, &runtime, two, two)
        .unwrap()
        .as_word();
    assert_eq!(
        call(&runtime, &mut ctx, "EQUAL", &[complex_left, complex_right]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "EQUALP", &[complex_left, complex_right],),
        Ok(Word::NIL)
    );
    let zero_complex = make_complex(&mut ctx, &runtime, two, Word::fixnum(0))
        .unwrap()
        .as_word();
    let double = make_double(&mut ctx, &runtime, 2.0).unwrap().as_word();
    assert_eq!(
        call(&runtime, &mut ctx, "EQUALP", &[zero_complex, double]),
        Ok(Word::TRUE)
    );

    let vector_left = make_simple_vector(&mut ctx, &runtime, &[one, two]).unwrap();
    let vector_right = make_simple_vector(&mut ctx, &runtime, &[one, two]).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "EQUALP", &[vector_left, vector_right]),
        Ok(Word::TRUE)
    );
    let options = ArrayOptions {
        element_type: ArrayElementType::T,
        initial_element: one,
        adjustable: false,
        fill_pointer: None,
        displaced_to: None,
        displaced_index_offset: 0,
    };
    let array_left = make_array(&mut ctx, &runtime, &[2], options).unwrap();
    let array_right = make_array(&mut ctx, &runtime, &[1, 2], options).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "EQUALP", &[array_left, array_right]),
        Ok(Word::NIL)
    );
    let bits_left = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(1), Word::fixnum(0)],
    )
    .unwrap();
    let bits_right = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(1), Word::fixnum(0)],
    )
    .unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "EQUAL", &[bits_left, bits_right]),
        Ok(Word::TRUE)
    );

    let left_table = HashTable::new(&mut ctx, &runtime, HashTest::Equalp, Weakness::None)
        .unwrap()
        .as_word();
    let right_table = HashTable::new(&mut ctx, &runtime, HashTest::Equalp, Weakness::None)
        .unwrap()
        .as_word();
    let left_key = make_string(&mut ctx, &runtime, &['k']).unwrap();
    let right_key = make_string(&mut ctx, &runtime, &['k']).unwrap();
    HashTable::from_word(left_table)
        .insert(&mut ctx, &runtime, left_key, one)
        .unwrap();
    HashTable::from_word(right_table)
        .insert(&mut ctx, &runtime, right_key, one)
        .unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "EQUALP", &[left_table, right_table]),
        Ok(Word::TRUE)
    );
    HashTable::from_word(right_table)
        .insert(&mut ctx, &runtime, right_key, two)
        .unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "EQUALP", &[left_table, right_table]),
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
            "EQUALP",
            &[negative_big, negative_double]
        ),
        Ok(Word::TRUE)
    );
}

#[test]
fn list_entries_cover_empty_variants_mutation_and_getf_errors() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let values = list(&runtime, &mut ctx, &[one, two]);
    let vector = make_simple_vector(&mut ctx, &runtime, &[one, two]).unwrap();
    let text = make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();
    let list_type = common_lisp_symbol(&mut ctx, &runtime, "LIST");
    let vector_type = common_lisp_symbol(&mut ctx, &runtime, "VECTOR");
    let string_type = common_lisp_symbol(&mut ctx, &runtime, "STRING");

    assert_eq!(call(&runtime, &mut ctx, "APPEND", &[]), Ok(Word::NIL));
    assert_eq!(call(&runtime, &mut ctx, "NCONC", &[]), Ok(Word::NIL));
    assert_eq!(call(&runtime, &mut ctx, "LIST*", &[one]), Ok(one));
    assert_eq!(
        call(&runtime, &mut ctx, "LIST*", &[]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "CONCATENATE",
            &[vector_type, values, vector],
        )
        .map(|value| vector_values(&ctx, value)),
        Ok(vec![one, two, one, two])
    );
    assert_eq!(
        call(&runtime, &mut ctx, "CONCATENATE", &[string_type, text],).map(|value| {
            (0..ncl_object::string_length(&ctx, value).unwrap())
                .map(|i| ncl_object::string_ref(&ctx, value, i).unwrap())
                .collect::<String>()
        }),
        Ok("ab".to_owned())
    );
    assert_eq!(
        call(&runtime, &mut ctx, "CONCATENATE", &[list_type, Word::NIL],)
            .map(|value| list_values(&ctx, value)),
        Ok(Vec::new())
    );

    let cons = ncl_object::make_cons(&mut ctx, &runtime, one, Word::NIL).unwrap();
    let dotted_list = ncl_object::make_cons(&mut ctx, &runtime, one, two).unwrap();
    assert_eq!(call(&runtime, &mut ctx, "RPLACA", &[cons, three]), Ok(cons));
    assert_eq!(
        call(&runtime, &mut ctx, "RPLACD", &[cons, values]),
        Ok(cons)
    );
    assert_eq!(list_values(&ctx, cons), vec![three, one, two]);
    assert_eq!(
        call(&runtime, &mut ctx, "LISTP", &[dotted_list]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ENDP", &[one]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "CAR", &[one]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "CDR", &[one]),
        Err(ObjectError::TypeError)
    );

    let dotted = ncl_object::make_cons(&mut ctx, &runtime, one, two).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "GETF", &[dotted, one]),
        Err(ObjectError::TypeError)
    );
    let malformed = list(&runtime, &mut ctx, &[one, two, three]);
    assert_eq!(
        call(&runtime, &mut ctx, "GETF", &[malformed, Word::fixnum(9)]),
        Err(ObjectError::TypeError)
    );
    let plist = list(&runtime, &mut ctx, &[one, two]);
    assert_eq!(
        call(&runtime, &mut ctx, "GETF", &[plist, Word::fixnum(9), three]),
        Ok(three)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "COPY-SEQ", &[values]).map(|value| list_values(&ctx, value)),
        Ok(vec![one, two])
    );
    assert_eq!(
        call(&runtime, &mut ctx, "REVERSE", &[values]).map(|value| list_values(&ctx, value)),
        Ok(vec![two, one])
    );
}

#[test]
fn ordered_sets_cover_all_result_shapes_options_and_malformed_lists() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let left = list(&runtime, &mut ctx, &[one, two, two]);
    let right = list(&runtime, &mut ctx, &[two, three]);
    let equal = runtime.function(&mut ctx, "COMMON-LISP", "EQUAL").unwrap();
    let equal = FunctionObject::try_from(equal).unwrap().as_word();
    let test = keyword(&mut ctx, &runtime, "TEST");
    let test_not = keyword(&mut ctx, &runtime, "TEST-NOT");
    let key = keyword(&mut ctx, &runtime, "KEY");
    let car = runtime.function(&mut ctx, "COMMON-LISP", "CAR").unwrap();
    let car = FunctionObject::try_from(car).unwrap().as_word();

    assert_eq!(
        call(&runtime, &mut ctx, "UNION", &[left, right, test, equal])
            .map(|value| list_values(&ctx, value)),
        Ok(vec![one, two, three])
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "INTERSECTION",
            &[left, right, test, equal]
        )
        .map(|value| list_values(&ctx, value)),
        Ok(vec![two])
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SET-DIFFERENCE",
            &[left, right, test, equal]
        )
        .map(|value| list_values(&ctx, value)),
        Ok(vec![one])
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SET-EXCLUSIVE-OR",
            &[left, right, test, equal],
        )
        .map(|value| list_values(&ctx, value)),
        Ok(vec![one, three])
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SUBSETP", &[left, right, test, equal]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SUBSETP", &[Word::NIL, right]),
        Err(ObjectError::TypeError)
    );

    let pairs_one = list(&runtime, &mut ctx, &[one, Word::fixnum(10)]);
    let pairs_two = list(&runtime, &mut ctx, &[two, Word::fixnum(20)]);
    let alist = list(&runtime, &mut ctx, &[pairs_one, pairs_two]);
    assert_eq!(
        call(&runtime, &mut ctx, "ASSOC", &[one, alist]).map(|value| list_values(&ctx, value)),
        Ok(vec![one, Word::fixnum(10)])
    );
    let rassoc_pair = ncl_object::make_cons(&mut ctx, &runtime, two, Word::fixnum(20)).unwrap();
    let rassoc_alist = list(&runtime, &mut ctx, &[rassoc_pair, pairs_one]);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "RASSOC",
            &[Word::fixnum(20), rassoc_alist]
        ),
        Ok(rassoc_pair)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ASSOC", &[three, alist]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "RASSOC", &[three, alist]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MEMBER", &[three, right]).map(|value| list_values(&ctx, value)),
        Ok(vec![three])
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MEMBER", &[one, right, test_not, equal],)
            .map(|value| list_values(&ctx, value)),
        Ok(vec![two, three])
    );
    assert_eq!(call(&runtime, &mut ctx, "ADJOIN", &[two, right]), Ok(right));
    assert_eq!(
        call(&runtime, &mut ctx, "ADJOIN", &[one, right]).map(|value| list_values(&ctx, value)),
        Ok(vec![one, two, three])
    );

    let keyed_key_one = list(&runtime, &mut ctx, &[one, Word::fixnum(11)]);
    let keyed_key_two = list(&runtime, &mut ctx, &[two, Word::fixnum(22)]);
    let keyed_one = list(&runtime, &mut ctx, &[keyed_key_one, Word::fixnum(10)]);
    let keyed_two = list(&runtime, &mut ctx, &[keyed_key_two, Word::fixnum(20)]);
    let keyed = list(&runtime, &mut ctx, &[keyed_one, keyed_two]);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ASSOC",
            &[keyed_key_two, keyed, key, car, test, equal],
        )
        .map(|value| list_values(&ctx, value)),
        Ok(vec![keyed_key_two, Word::fixnum(20)])
    );
    let malformed_alist = list(&runtime, &mut ctx, &[one]);
    assert_eq!(
        call(&runtime, &mut ctx, "ASSOC", &[one, malformed_alist]),
        Err(ObjectError::TypeError)
    );
    let dotted_alist = ncl_object::make_cons(&mut ctx, &runtime, pairs_one, one).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "ASSOC", &[three, dotted_alist]),
        Err(ObjectError::TypeError)
    );
    let dotted_members = ncl_object::make_cons(&mut ctx, &runtime, one, two).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "MEMBER", &[three, dotted_members]),
        Err(ObjectError::TypeError)
    );

    let sort_values = make_simple_vector(&mut ctx, &runtime, &[three, one, two]).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "SORT", &[sort_values, equal],),
        Ok(sort_values)
    );
    let keyed_values = list(&runtime, &mut ctx, &[keyed_two, keyed_one]);
    let cons = runtime.function(&mut ctx, "COMMON-LISP", "CONS").unwrap();
    let cons = FunctionObject::try_from(cons).unwrap().as_word();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "STABLE-SORT",
            &[keyed_values, cons, key, car],
        ),
        Ok(keyed_values)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "UNION",
            &[left, right, test, equal, test_not, equal],
        ),
        Err(ObjectError::TypeError)
    );
    let _ = test_not;
}
