#![allow(
    clippy::similar_names,
    clippy::too_many_lines,
    missing_docs,
    clippy::unwrap_used
)]

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, ObjectError, Package, Runtime, ThreadContext,
    Word, make_array, make_bignum_from_i128, make_complex, make_double, make_ratio,
    make_simple_vector, make_string,
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

fn function(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> Word {
    let function = runtime.function(ctx, "COMMON-LISP", name).unwrap();
    FunctionObject::try_from(function).unwrap().as_word()
}

fn list(runtime: &Runtime, ctx: &mut ThreadContext, values: &[Word]) -> Word {
    call(runtime, ctx, "LIST", values).unwrap()
}

fn dotted(runtime: &Runtime, ctx: &mut ThreadContext, car: Word, cdr: Word) -> Word {
    call(runtime, ctx, "CONS", &[car, cdr]).unwrap()
}

fn list_values(ctx: &ThreadContext, mut value: Word) -> Vec<Word> {
    let mut values = Vec::new();
    while value != Word::NIL {
        values.push(ncl_object::car(ctx, value).unwrap());
        value = ncl_object::cdr(ctx, value).unwrap();
    }
    values
}

fn vector_values(ctx: &ThreadContext, value: Word) -> Vec<Word> {
    (0..ncl_object::simple_vector_length(ctx, value).unwrap())
        .map(|index| ncl_object::simple_vector_ref(ctx, value, index).unwrap())
        .collect()
}

fn string_value(ctx: &ThreadContext, value: Word) -> String {
    (0..ncl_object::string_length(ctx, value).unwrap())
        .map(|index| ncl_object::string_ref(ctx, value, index).unwrap())
        .collect()
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

#[test]
fn registered_list_accessors_and_sequence_entries_return_contract_values() {
    let (runtime, mut ctx) = setup();
    let values = (1..=10).map(Word::fixnum).collect::<Vec<_>>();
    let source = list(&runtime, &mut ctx, &values);
    assert_eq!(call(&runtime, &mut ctx, "LISTP", &[source]), Ok(Word::TRUE));
    assert_eq!(call(&runtime, &mut ctx, "CONSP", &[source]), Ok(Word::TRUE));
    assert_eq!(
        call(&runtime, &mut ctx, "ENDP", &[Word::NIL]),
        Ok(Word::TRUE)
    );
    assert_eq!(call(&runtime, &mut ctx, "ENDP", &[source]), Ok(Word::NIL));
    assert_eq!(call(&runtime, &mut ctx, "CAR", &[Word::NIL]), Ok(Word::NIL));
    assert_eq!(call(&runtime, &mut ctx, "CDR", &[Word::NIL]), Ok(Word::NIL));
    assert_eq!(
        call(&runtime, &mut ctx, "CAR", &[Word::fixnum(0)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ENDP", &[Word::fixnum(0)]),
        Err(ObjectError::TypeError)
    );

    for (name, expected) in [
        ("FIRST", 1),
        ("SECOND", 2),
        ("THIRD", 3),
        ("FOURTH", 4),
        ("FIFTH", 5),
        ("SIXTH", 6),
        ("SEVENTH", 7),
        ("EIGHTH", 8),
        ("NINTH", 9),
        ("TENTH", 10),
    ] {
        assert_eq!(
            call(&runtime, &mut ctx, name, &[source]),
            Ok(Word::fixnum(expected)),
            "{name}"
        );
    }
    assert_eq!(
        call(&runtime, &mut ctx, "NTH", &[Word::fixnum(4), source]),
        Ok(Word::fixnum(5))
    );
    assert_eq!(
        call(&runtime, &mut ctx, "NTHCDR", &[Word::fixnum(8), source])
            .map(|value| list_values(&ctx, value)),
        Ok(vec![Word::fixnum(9), Word::fixnum(10)])
    );
    assert_eq!(
        call(&runtime, &mut ctx, "NTH", &[Word::fixnum(20), source]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "LIST-LENGTH", &[source]),
        Ok(Word::fixnum(10))
    );
    let dotted_values = dotted(&runtime, &mut ctx, values[0], values[1]);
    assert_eq!(
        call(&runtime, &mut ctx, "LIST-LENGTH", &[dotted_values]),
        Err(ObjectError::TypeError)
    );

    let nested = list(&runtime, &mut ctx, &[source]);
    assert_eq!(
        call(&runtime, &mut ctx, "CAAR", &[nested]),
        Ok(Word::fixnum(1))
    );
    let cadr_input = list(&runtime, &mut ctx, &[Word::NIL, source]);
    assert_eq!(call(&runtime, &mut ctx, "CADR", &[cadr_input]), Ok(source));
    assert_eq!(
        call(&runtime, &mut ctx, "CDAR", &[nested]).map(|value| list_values(&ctx, value)),
        Ok(values[1..].to_vec())
    );
    let cddr_input = list(&runtime, &mut ctx, &[Word::NIL, Word::NIL, source]);
    assert_eq!(
        call(&runtime, &mut ctx, "CDDR", &[cddr_input]).map(|value| list_values(&ctx, value)),
        Ok(vec![source])
    );

    assert_eq!(
        call(&runtime, &mut ctx, "LENGTH", &[source]),
        Ok(Word::fixnum(10))
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ELT", &[source, Word::fixnum(9)]),
        Ok(Word::fixnum(10))
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ELT", &[source, Word::fixnum(10)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SUBSEQ",
            &[source, Word::fixnum(3), Word::fixnum(6)]
        )
        .map(|value| list_values(&ctx, value)),
        Ok(vec![Word::fixnum(4), Word::fixnum(5), Word::fixnum(6)])
    );
}

#[test]
fn equality_covers_numeric_arrays_hash_tables_and_structure_differences() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let ratio = make_ratio(&mut ctx, &runtime, two, Word::fixnum(4))
        .unwrap()
        .as_word();
    let double = make_double(&mut ctx, &runtime, 0.5).unwrap().as_word();
    let integer = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 70)
        .unwrap()
        .as_word();
    let complex = make_complex(&mut ctx, &runtime, two, Word::fixnum(0))
        .unwrap()
        .as_word();
    assert_eq!(
        call(&runtime, &mut ctx, "EQUALP", &[ratio, double]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "EQUALP", &[complex, two]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "EQUALP", &[integer, one]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "EQUAL", &[ratio, ratio]),
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
    let array_left = make_array(&mut ctx, &runtime, &[2, 2], options).unwrap();
    let array_right = make_array(&mut ctx, &runtime, &[2, 2], options).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "EQUALP", &[array_left, array_right]),
        Ok(Word::TRUE)
    );
    let array_other = make_array(&mut ctx, &runtime, &[4], options).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "EQUALP", &[array_left, array_other]),
        Ok(Word::NIL)
    );
    let vector = make_simple_vector(&mut ctx, &runtime, &[one, two]).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "EQUAL", &[vector, vector]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "EQUAL", &[vector, array_left]),
        Ok(Word::NIL)
    );

    let left_table = HashTable::new(&mut ctx, &runtime, HashTest::Equalp, Weakness::None)
        .unwrap()
        .as_word();
    let right_table = HashTable::new(&mut ctx, &runtime, HashTest::Equalp, Weakness::None)
        .unwrap()
        .as_word();
    let key_left = make_string(&mut ctx, &runtime, &['a']).unwrap();
    let key_right = make_string(&mut ctx, &runtime, &['b']).unwrap();
    HashTable::from_word(left_table)
        .insert(&mut ctx, &runtime, key_left, one)
        .unwrap();
    HashTable::from_word(right_table)
        .insert(&mut ctx, &runtime, key_right, one)
        .unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "EQUALP", &[left_table, right_table]),
        Ok(Word::NIL)
    );

    let proper = list(&runtime, &mut ctx, &[one, two]);
    let dotted_value = dotted(&runtime, &mut ctx, one, two);
    assert_eq!(
        call(&runtime, &mut ctx, "EQUAL", &[proper, dotted_value]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "EQUALP",
            &[three, Word::character(u32::from('3'))]
        ),
        Ok(Word::NIL)
    );
}

#[test]
fn selection_adapters_cover_aliases_predicates_ranges_and_errors() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let four = Word::fixnum(4);
    let source = list(&runtime, &mut ctx, &[one, two, one, three, two]);
    let start = keyword(&mut ctx, &runtime, "START");
    let end = keyword(&mut ctx, &runtime, "END");
    let count = keyword(&mut ctx, &runtime, "COUNT");
    let from_end = keyword(&mut ctx, &runtime, "FROM-END");
    let test = keyword(&mut ctx, &runtime, "TEST");
    let test_not = keyword(&mut ctx, &runtime, "TEST-NOT");
    let equal = function(&runtime, &mut ctx, "EQUAL");
    let atom = function(&runtime, &mut ctx, "ATOM");
    let nested_one = list(&runtime, &mut ctx, &[one]);
    let nested_two = list(&runtime, &mut ctx, &[two]);
    let nested = list(&runtime, &mut ctx, &[nested_one, nested_two]);
    let car = function(&runtime, &mut ctx, "CAR");
    let key = keyword(&mut ctx, &runtime, "KEY");

    assert_eq!(
        call(&runtime, &mut ctx, "FIND-IF-NOT", &[atom, nested]),
        Ok(nested_one)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "POSITION-IF-NOT", &[atom, nested]),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        call(&runtime, &mut ctx, "COUNT-IF-NOT", &[atom, nested]),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        call(&runtime, &mut ctx, "REMOVE-IF-NOT", &[atom, nested])
            .map(|value| list_values(&ctx, value)),
        Ok(Vec::new())
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SUBSTITUTE-IF",
            &[four, atom, source, count, two]
        )
        .map(|value| list_values(&ctx, value)),
        Ok(vec![four, four, one, three, two])
    );
    assert_eq!(
        call(&runtime, &mut ctx, "DELETE-IF", &[atom, source]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "NSUBSTITUTE",
            &[four, one, source, from_end, Word::TRUE, count, one],
        )
        .map(|value| list_values(&ctx, value)),
        Ok(vec![one, two, four, three, two])
    );
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
        call(
            &runtime,
            &mut ctx,
            "POSITION",
            &[two, source, test_not, equal, start, one, end, four],
        ),
        Ok(Word::fixnum(2))
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
        call(
            &runtime,
            &mut ctx,
            "POSITION",
            &[one, source, start, Word::fixnum(5), end, Word::fixnum(2)],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SEARCH", &[source, Word::fixnum(3)]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn ordered_sets_and_destructive_lists_cover_option_and_mutation_branches() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let four = Word::fixnum(4);
    let left = list(&runtime, &mut ctx, &[one, two, two]);
    let right = list(&runtime, &mut ctx, &[two, three, four]);
    let test = keyword(&mut ctx, &runtime, "TEST");
    let test_not = keyword(&mut ctx, &runtime, "TEST-NOT");
    let equal = function(&runtime, &mut ctx, "EQUAL");

    for (name, expected) in [
        ("UNION", vec![one, two, three, four]),
        ("INTERSECTION", vec![two]),
        ("SET-DIFFERENCE", vec![one]),
        ("SET-EXCLUSIVE-OR", vec![one, three, four]),
    ] {
        assert_eq!(
            call(&runtime, &mut ctx, name, &[left, right, test, equal])
                .map(|value| list_values(&ctx, value)),
            Ok(expected),
            "{name}"
        );
    }
    assert_eq!(
        call(&runtime, &mut ctx, "SUBSETP", &[left, right, test, equal]),
        Ok(Word::NIL)
    );
    let subset = list(&runtime, &mut ctx, &[two, three]);
    assert_eq!(
        call(&runtime, &mut ctx, "SUBSETP", &[subset, right, test, equal]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MEMBER", &[one, right, test_not, equal])
            .map(|value| list_values(&ctx, value)),
        Ok(vec![two, three, four])
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ADJOIN", &[two, right, test, equal]),
        Ok(right)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ADJOIN", &[one, right, test, equal])
            .map(|value| list_values(&ctx, value)),
        Ok(vec![one, two, three, four])
    );

    let pair_one = list(&runtime, &mut ctx, &[one, Word::fixnum(10)]);
    let pair_two = list(&runtime, &mut ctx, &[two, Word::fixnum(20)]);
    let alist = list(&runtime, &mut ctx, &[pair_one, pair_two]);
    assert_eq!(
        call(&runtime, &mut ctx, "ASSOC", &[two, alist]),
        Ok(pair_two)
    );
    let rassoc_pair = dotted(&runtime, &mut ctx, two, Word::fixnum(20));
    let rassoc_alist = list(&runtime, &mut ctx, &[rassoc_pair, pair_one]);
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
        call(&runtime, &mut ctx, "ASSOC", &[four, alist]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "RASSOC", &[four, alist]),
        Ok(Word::NIL)
    );

    let vector = make_simple_vector(&mut ctx, &runtime, &[three, one, two]).unwrap();
    let cons = function(&runtime, &mut ctx, "CONS");
    assert_eq!(
        call(&runtime, &mut ctx, "SORT", &[vector, cons]),
        Ok(vector)
    );
    assert_eq!(vector_values(&ctx, vector), vec![two, one, three]);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "UNION",
            &[left, right, test, equal, test_not, equal],
        ),
        Err(ObjectError::TypeError)
    );

    let first = list(&runtime, &mut ctx, &[one, two]);
    let second = list(&runtime, &mut ctx, &[three]);
    assert_eq!(
        call(&runtime, &mut ctx, "NCONC", &[Word::NIL, first, second]),
        Ok(first)
    );
    assert_eq!(list_values(&ctx, first), vec![one, two, three]);
    assert_eq!(
        call(&runtime, &mut ctx, "RPLACD", &[first, Word::NIL]),
        Ok(first)
    );
    assert_eq!(list_values(&ctx, first), vec![one]);
    assert_eq!(
        call(&runtime, &mut ctx, "APPEND", &[first, Word::NIL])
            .map(|value| list_values(&ctx, value)),
        Ok(vec![one])
    );
    assert_eq!(
        call(&runtime, &mut ctx, "APPEND", &[Word::fixnum(8), Word::NIL]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(call(&runtime, &mut ctx, "NCONC", &[]), Ok(Word::NIL));
    assert_eq!(call(&runtime, &mut ctx, "LIST*", &[one]), Ok(one));
    let dotted_tail = dotted(&runtime, &mut ctx, two, three);
    let list_star = call(&runtime, &mut ctx, "LIST*", &[one, dotted_tail]).unwrap();
    assert_eq!(ncl_object::car(&ctx, list_star), Ok(one));
    assert_eq!(ncl_object::cdr(&ctx, list_star), Ok(dotted_tail));

    let text = make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();
    let string_type = common_lisp_symbol(&mut ctx, &runtime, "STRING");
    assert_eq!(
        call(&runtime, &mut ctx, "CONCATENATE", &[string_type, text])
            .map(|value| string_value(&ctx, value)),
        Ok("ab".to_owned())
    );
    assert_eq!(
        call(&runtime, &mut ctx, "CONCATENATE", &[string_type, left]),
        Err(ObjectError::TypeError)
    );
}
