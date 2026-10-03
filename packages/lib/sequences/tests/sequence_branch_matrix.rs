#![allow(missing_docs, clippy::too_many_lines, clippy::unwrap_used)]

use ncl_object::{FunctionObject, ObjectError, Package, Runtime, ThreadContext, Word};

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
fn selection_branch_matrix_returns_exact_keyed_and_bounded_results() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let four = Word::fixnum(4);
    let source = list(&runtime, &mut ctx, &[one, two, one, three, two]);
    let start = keyword(&mut ctx, &runtime, "START");
    let end = keyword(&mut ctx, &runtime, "END");
    let from_end = keyword(&mut ctx, &runtime, "FROM-END");
    let count = keyword(&mut ctx, &runtime, "COUNT");
    let test = keyword(&mut ctx, &runtime, "TEST");
    let test_not = keyword(&mut ctx, &runtime, "TEST-NOT");
    let equal = function(&runtime, &mut ctx, "EQUAL");
    let atom = function(&runtime, &mut ctx, "ATOM");

    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "FIND",
            &[two, source, start, one, end, four, from_end, Word::TRUE],
        ),
        Ok(two)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "POSITION",
            &[one, source, from_end, Word::TRUE, count, one],
        ),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        call(&runtime, &mut ctx, "COUNT", &[two, source, test, equal]),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "COUNT",
            &[two, source, test_not, equal, start, one, end, four],
        ),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        call(&runtime, &mut ctx, "REMOVE", &[two, source, test, equal])
            .map(|value| list_values(&ctx, value)),
        Ok(vec![one, one, three])
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SUBSTITUTE",
            &[four, one, source, from_end, Word::TRUE, count, one],
        )
        .map(|value| list_values(&ctx, value)),
        Ok(vec![one, two, four, three, two])
    );

    let nested_one = list(&runtime, &mut ctx, &[one, Word::character(u32::from('a'))]);
    let nested_two = list(&runtime, &mut ctx, &[two, Word::character(u32::from('b'))]);
    let nested_three = list(
        &runtime,
        &mut ctx,
        &[three, Word::character(u32::from('c'))],
    );
    let nested = list(&runtime, &mut ctx, &[nested_one, nested_two, nested_three]);
    let car = function(&runtime, &mut ctx, "CAR");
    let key = keyword(&mut ctx, &runtime, "KEY");
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "FIND",
            &[two, nested, key, car, test, equal],
        ),
        Ok(nested_two)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "POSITION",
            &[two, nested, key, car, test_not, equal],
        ),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        call(&runtime, &mut ctx, "FIND-IF", &[atom, source]),
        Ok(one)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "FIND-IF-NOT", &[atom, nested]),
        Ok(nested_one)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "POSITION-IF", &[atom, nested]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "POSITION-IF-NOT", &[atom, nested]),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        call(&runtime, &mut ctx, "COUNT-IF", &[atom, source]),
        Ok(Word::fixnum(5))
    );
    assert_eq!(
        call(&runtime, &mut ctx, "COUNT-IF-NOT", &[atom, nested]),
        Ok(Word::fixnum(3))
    );
    assert_eq!(
        call(&runtime, &mut ctx, "REMOVE-IF", &[atom, source])
            .map(|value| list_values(&ctx, value)),
        Ok(Vec::new())
    );
    assert_eq!(
        call(&runtime, &mut ctx, "REMOVE-IF-NOT", &[atom, nested])
            .map(|value| list_values(&ctx, value)),
        Ok(Vec::new())
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SUBSTITUTE-IF", &[four, atom, source])
            .map(|value| list_values(&ctx, value)),
        Ok(vec![four, four, four, four, four])
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SUBSTITUTE-IF-NOT",
            &[four, atom, nested]
        )
        .map(|value| list_values(&ctx, value)),
        Ok(vec![four, four, four])
    );

    let left = list(&runtime, &mut ctx, &[two, three]);
    let right = list(&runtime, &mut ctx, &[one, two, three, two, three]);
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
        Ok(Word::fixnum(3))
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SEARCH", &[left, source, start, three]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MISMATCH", &[right, right, test, equal]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MISMATCH", &[left, right, test, equal]),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MISMATCH",
            &[left, right, start, one, end, three, test, equal],
        ),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn filter_and_list_branch_matrix_preserves_sequence_types_and_ranges() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let four = Word::fixnum(4);
    let start = keyword(&mut ctx, &runtime, "START");
    let end = keyword(&mut ctx, &runtime, "END");
    let list_value = list(&runtime, &mut ctx, &[one, two, three]);
    let vector = ncl_object::make_simple_vector(&mut ctx, &runtime, &[one, two, three]).unwrap();
    let text = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b', 'c']).unwrap();

    assert_eq!(
        call(&runtime, &mut ctx, "LENGTH", &[list_value]),
        Ok(Word::fixnum(3))
    );
    assert_eq!(
        call(&runtime, &mut ctx, "LENGTH", &[vector]),
        Ok(Word::fixnum(3))
    );
    assert_eq!(
        call(&runtime, &mut ctx, "LENGTH", &[text]),
        Ok(Word::fixnum(3))
    );
    assert_eq!(call(&runtime, &mut ctx, "ELT", &[vector, two]), Ok(three));
    assert_eq!(
        call(&runtime, &mut ctx, "ELT", &[text, one]),
        Ok(Word::character(u32::from('b')))
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SUBSEQ", &[list_value, one, three])
            .map(|value| list_values(&ctx, value)),
        Ok(vec![two, three])
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SUBSEQ", &[vector, one, three])
            .map(|value| vector_values(&ctx, value)),
        Ok(vec![two, three])
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SUBSEQ", &[text, one, three])
            .map(|value| ncl_object::string_ref(&ctx, value, 0).unwrap()),
        Ok('b')
    );
    assert_eq!(
        call(&runtime, &mut ctx, "REVERSE", &[list_value]).map(|value| list_values(&ctx, value)),
        Ok(vec![three, two, one])
    );
    assert_eq!(
        call(&runtime, &mut ctx, "REVERSE", &[vector]).map(|value| vector_values(&ctx, value)),
        Ok(vec![three, two, one])
    );
    assert_eq!(
        call(&runtime, &mut ctx, "REVERSE", &[text]).map(|value| {
            (0..3)
                .map(|index| ncl_object::string_ref(&ctx, value, index).unwrap())
                .collect::<String>()
        }),
        Ok("cba".to_owned())
    );

    let fill_list = list(&runtime, &mut ctx, &[one, two, three, four]);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "FILL",
            &[fill_list, four, start, one, end, three],
        ),
        Ok(fill_list)
    );
    assert_eq!(list_values(&ctx, fill_list), vec![one, four, four, four]);
    let fill_vector =
        ncl_object::make_simple_vector(&mut ctx, &runtime, &[one, two, three]).unwrap();
    call(&runtime, &mut ctx, "FILL", &[fill_vector, four, start, two]).unwrap();
    assert_eq!(vector_values(&ctx, fill_vector), vec![one, two, four]);
    let fill_text = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b', 'c']).unwrap();
    call(
        &runtime,
        &mut ctx,
        "FILL",
        &[
            fill_text,
            Word::character(u32::from('z')),
            start,
            one,
            end,
            two,
        ],
    )
    .unwrap();
    assert_eq!(
        (0..3)
            .map(|index| ncl_object::string_ref(&ctx, fill_text, index).unwrap())
            .collect::<String>(),
        "azc"
    );

    let destination = ncl_object::make_simple_vector(&mut ctx, &runtime, &[one, one, one]).unwrap();
    let replacement = list(&runtime, &mut ctx, &[two, three]);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "REPLACE",
            &[destination, replacement, start, one]
        ),
        Ok(destination)
    );
    assert_eq!(vector_values(&ctx, destination), vec![one, two, three]);
    let concatenated_list = common_lisp_symbol(&mut ctx, &runtime, "LIST");
    let concatenated_vector = common_lisp_symbol(&mut ctx, &runtime, "VECTOR");
    let concatenated_string = common_lisp_symbol(&mut ctx, &runtime, "STRING");
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "CONCATENATE",
            &[concatenated_list, list_value, vector]
        )
        .map(|value| list_values(&ctx, value)),
        Ok(vec![one, two, three, one, two, three])
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "CONCATENATE",
            &[concatenated_vector, list_value, text]
        )
        .map(|value| vector_values(&ctx, value)),
        Ok(vec![
            one,
            two,
            three,
            Word::character(u32::from('a')),
            Word::character(u32::from('b')),
            Word::character(u32::from('c'))
        ])
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "CONCATENATE",
            &[concatenated_string, text, text]
        )
        .map(|value| {
            (0..6)
                .map(|index| ncl_object::string_ref(&ctx, value, index).unwrap())
                .collect::<String>()
        }),
        Ok("abcabc".to_owned())
    );
    let append_tail = list(&runtime, &mut ctx, &[one, two, three]);
    assert_eq!(
        call(&runtime, &mut ctx, "APPEND", &[list_value, append_tail])
            .map(|value| list_values(&ctx, value)),
        Ok(vec![one, two, three, one, two, three])
    );
    let list_star = call(&runtime, &mut ctx, "LIST*", &[one, two, three]).unwrap();
    assert_eq!(ncl_object::car(&ctx, list_star), Ok(one));
    let list_star_tail = ncl_object::cdr(&ctx, list_star).unwrap();
    assert_eq!(ncl_object::car(&ctx, list_star_tail), Ok(two));
    assert_eq!(ncl_object::cdr(&ctx, list_star_tail).unwrap(), three);
}

#[test]
fn higher_order_branch_matrix_returns_lists_vectors_and_short_circuits() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let four = Word::fixnum(4);
    let source = list(&runtime, &mut ctx, &[one, two, three]);
    let other = list(&runtime, &mut ctx, &[four, three, two]);
    let list_fn = function(&runtime, &mut ctx, "LIST");
    let car = function(&runtime, &mut ctx, "CAR");
    let atom = function(&runtime, &mut ctx, "ATOM");

    let mapped = call(&runtime, &mut ctx, "MAPCAR", &[list_fn, source, other]).unwrap();
    let mapped_values = list_values(&ctx, mapped);
    assert_eq!(mapped_values.len(), 3);
    assert_eq!(list_values(&ctx, mapped_values[0]), vec![one, four]);
    assert_eq!(list_values(&ctx, mapped_values[2]), vec![three, two]);
    assert_eq!(
        call(&runtime, &mut ctx, "MAPC", &[list_fn, source]),
        Ok(source)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MAPLIST", &[car, source]).map(|value| list_values(&ctx, value)),
        Ok(vec![one, two, three])
    );
    assert_eq!(call(&runtime, &mut ctx, "MAPL", &[car, source]), Ok(source));
    assert_eq!(
        call(&runtime, &mut ctx, "MAPCAN", &[list_fn, source])
            .map(|value| list_values(&ctx, value)),
        Ok(vec![one, two, three])
    );
    let mapcon = call(&runtime, &mut ctx, "MAPCON", &[list_fn, source]).unwrap();
    let mapcon_values = list_values(&ctx, mapcon);
    assert_eq!(mapcon_values.len(), 3);
    assert_eq!(mapcon_values[0], source);
    assert_eq!(list_values(&ctx, mapcon_values[1]), vec![two, three]);

    let map_result_list = list(&runtime, &mut ctx, &[Word::NIL]);
    let mapped_list = call(
        &runtime,
        &mut ctx,
        "MAP",
        &[map_result_list, list_fn, source, other],
    )
    .unwrap();
    assert_eq!(list_values(&ctx, mapped_list).len(), 3);
    let map_result_vector = ncl_object::make_simple_vector(&mut ctx, &runtime, &[]).unwrap();
    let mapped_vector = call(
        &runtime,
        &mut ctx,
        "MAP",
        &[map_result_vector, list_fn, source, other],
    )
    .unwrap();
    assert_eq!(vector_values(&ctx, mapped_vector).len(), 3);

    let source_one = list(&runtime, &mut ctx, &[one]);
    let source_two = list(&runtime, &mut ctx, &[two]);
    let source_three = list(&runtime, &mut ctx, &[three]);
    let source_of_lists = list(&runtime, &mut ctx, &[source_one, source_two, source_three]);
    let destination = list(&runtime, &mut ctx, &[Word::NIL, Word::NIL, Word::NIL]);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAP-INTO",
            &[destination, car, source_of_lists],
        ),
        Ok(destination)
    );
    assert_eq!(list_values(&ctx, destination), vec![one, two, three]);
    let destination_vector =
        ncl_object::make_simple_vector(&mut ctx, &runtime, &[Word::NIL, Word::NIL, Word::NIL])
            .unwrap();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAP-INTO",
            &[destination_vector, car, source_of_lists],
        ),
        Ok(destination_vector)
    );
    assert_eq!(
        vector_values(&ctx, destination_vector),
        vec![one, two, three]
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MAP", &[Word::NIL, list_fn]),
        Ok(Word::NIL)
    );

    assert_eq!(
        call(&runtime, &mut ctx, "EVERY", &[atom, source]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SOME", &[atom, source]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "NOTANY", &[atom, source]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "NOTEVERY", &[atom, source]),
        Ok(Word::NIL)
    );
    let nested = list(&runtime, &mut ctx, &[source, other]);
    assert_eq!(
        call(&runtime, &mut ctx, "EVERY", &[atom, nested]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SOME", &[atom, nested]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "NOTANY", &[atom, nested]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "NOTEVERY", &[atom, nested]),
        Ok(Word::TRUE)
    );

    let initial_value = keyword(&mut ctx, &runtime, "INITIAL-VALUE");
    let reduce = call(
        &runtime,
        &mut ctx,
        "REDUCE",
        &[list_fn, source, initial_value, Word::NIL],
    )
    .unwrap();
    let reduce_values = list_values(&ctx, reduce);
    assert_eq!(reduce_values.len(), 2);
    assert_eq!(reduce_values[1], three);
    let prior_reduce_values = list_values(&ctx, reduce_values[0]);
    assert_eq!(prior_reduce_values[1], two);
    assert_eq!(
        list_values(&ctx, prior_reduce_values[0]),
        vec![Word::NIL, one]
    );
}

#[test]
fn order_and_equality_branch_matrix_preserves_order_and_content_contracts() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let four = Word::fixnum(4);
    let first = list(&runtime, &mut ctx, &[one, two, three]);
    let second = list(&runtime, &mut ctx, &[three, four, two]);
    let equal = function(&runtime, &mut ctx, "EQUAL");
    let test = keyword(&mut ctx, &runtime, "TEST");
    let test_not = keyword(&mut ctx, &runtime, "TEST-NOT");
    let key = keyword(&mut ctx, &runtime, "KEY");
    let car = function(&runtime, &mut ctx, "CAR");

    assert_eq!(
        call(&runtime, &mut ctx, "UNION", &[first, second]).map(|value| list_values(&ctx, value)),
        Ok(vec![one, two, three, four])
    );
    assert_eq!(
        call(&runtime, &mut ctx, "INTERSECTION", &[first, second])
            .map(|value| list_values(&ctx, value)),
        Ok(vec![two, three])
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SET-DIFFERENCE", &[first, second])
            .map(|value| list_values(&ctx, value)),
        Ok(vec![one])
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SET-EXCLUSIVE-OR", &[first, second])
            .map(|value| list_values(&ctx, value)),
        Ok(vec![one, four])
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SUBSETP", &[first, second]),
        Ok(Word::NIL)
    );
    let subset = list(&runtime, &mut ctx, &[two, three]);
    assert_eq!(
        call(&runtime, &mut ctx, "SUBSETP", &[subset, second]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "UNION",
            &[first, second, test, equal, test_not, equal],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "UNION",
            &[first, second, test_not, equal]
        )
        .map(|value| list_values(&ctx, value)),
        Ok(vec![one])
    );

    let added = call(&runtime, &mut ctx, "ADJOIN", &[four, first]).unwrap();
    assert_eq!(list_values(&ctx, added), vec![four, one, two, three]);
    assert_eq!(call(&runtime, &mut ctx, "ADJOIN", &[two, first]), Ok(first));
    let pair_one = dotted(&runtime, &mut ctx, one, two);
    let pair_two = dotted(&runtime, &mut ctx, three, one);
    let alist = list(&runtime, &mut ctx, &[pair_one, pair_two]);
    assert_eq!(
        call(&runtime, &mut ctx, "ASSOC", &[three, alist]),
        Ok(pair_two)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "RASSOC", &[one, alist]),
        Ok(pair_two)
    );
    let member_tail = ncl_object::cdr(&ctx, ncl_object::cdr(&ctx, first).unwrap()).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "MEMBER", &[three, first]),
        Ok(member_tail)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ASSOC", &[one, alist, test, equal]),
        Ok(pair_one)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "RASSOC", &[one, alist, test_not, equal]),
        Ok(pair_one)
    );

    let unsorted = list(&runtime, &mut ctx, &[three, one, two]);
    let comparator = equal;
    assert_eq!(
        call(&runtime, &mut ctx, "SORT", &[unsorted, comparator])
            .map(|value| list_values(&ctx, value)),
        Ok(vec![three, one, two])
    );
    let stable = ncl_object::make_simple_vector(&mut ctx, &runtime, &[three, one, two]).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "STABLE-SORT", &[stable, comparator])
            .map(|value| vector_values(&ctx, value)),
        Ok(vec![three, one, two])
    );
    let keyed_one = list(&runtime, &mut ctx, &[three, one]);
    let keyed_two = list(&runtime, &mut ctx, &[one, two]);
    let keyed_values = list(&runtime, &mut ctx, &[keyed_one, keyed_two]);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SORT",
            &[keyed_values, comparator, key, car]
        )
        .map(|value| list_values(&ctx, value)),
        Ok(vec![keyed_one, keyed_two])
    );

    let nested_left_tail = list(&runtime, &mut ctx, &[two]);
    let nested_right_tail = list(&runtime, &mut ctx, &[two]);
    let nested_left = list(&runtime, &mut ctx, &[one, nested_left_tail]);
    let nested_right = list(&runtime, &mut ctx, &[one, nested_right_tail]);
    assert_eq!(
        call(&runtime, &mut ctx, "EQUAL", &[nested_left, nested_right]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "EQUAL", &[nested_left, first]),
        Ok(Word::NIL)
    );
    let lower = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();
    let upper = ncl_object::make_string(&mut ctx, &runtime, &['A', 'B']).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "EQUAL", &[lower, upper]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "EQUALP", &[lower, upper]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "EQUALP",
            &[
                Word::character(u32::from('z')),
                Word::character(u32::from('Z'))
            ],
        ),
        Ok(Word::TRUE)
    );
    let vector_one = ncl_object::make_simple_vector(&mut ctx, &runtime, &[one, two]).unwrap();
    let vector_two = ncl_object::make_simple_vector(&mut ctx, &runtime, &[one, two]).unwrap();
    let vector_three = ncl_object::make_simple_vector(&mut ctx, &runtime, &[one, three]).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "EQUAL", &[vector_one, vector_two]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "EQUALP", &[vector_one, vector_two]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "EQUALP", &[vector_one, vector_three]),
        Ok(Word::NIL)
    );
}
