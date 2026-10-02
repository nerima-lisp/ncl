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

fn string_value(ctx: &ThreadContext, value: Word) -> String {
    (0..ncl_object::string_length(ctx, value).unwrap())
        .map(|index| ncl_object::string_ref(ctx, value, index).unwrap())
        .collect()
}

#[test]
fn positional_accessors_cover_each_index_nil_and_improper_tails() {
    let (runtime, mut ctx) = setup();
    let values = (1..=10).map(Word::fixnum).collect::<Vec<_>>();
    let source = list(&runtime, &mut ctx, &values);

    for (name, expected) in [
        ("FIRST", values[0]),
        ("SECOND", values[1]),
        ("THIRD", values[2]),
        ("FOURTH", values[3]),
        ("FIFTH", values[4]),
        ("SIXTH", values[5]),
        ("SEVENTH", values[6]),
        ("EIGHTH", values[7]),
        ("NINTH", values[8]),
        ("TENTH", values[9]),
    ] {
        assert_eq!(
            call(&runtime, &mut ctx, name, &[source]),
            Ok(expected),
            "{name}"
        );
    }
    assert_eq!(
        call(&runtime, &mut ctx, "NTH", &[Word::fixnum(0), source]),
        Ok(values[0])
    );
    assert_eq!(
        call(&runtime, &mut ctx, "NTH", &[Word::fixnum(9), source]),
        Ok(values[9])
    );
    assert_eq!(
        call(&runtime, &mut ctx, "NTH", &[Word::fixnum(10), source]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "NTHCDR", &[Word::fixnum(0), source]),
        Ok(source)
    );
    let tail = call(&runtime, &mut ctx, "NTHCDR", &[Word::fixnum(4), source]).unwrap();
    assert_eq!(list_values(&ctx, tail), values[4..].to_vec());
    assert_eq!(
        call(&runtime, &mut ctx, "NTHCDR", &[Word::fixnum(10), source]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "LIST-LENGTH", &[source]),
        Ok(Word::fixnum(10))
    );

    for name in [
        "FIRST", "SECOND", "THIRD", "FOURTH", "FIFTH", "SIXTH", "SEVENTH", "EIGHTH", "NINTH",
        "TENTH",
    ] {
        assert_eq!(
            call(&runtime, &mut ctx, name, &[Word::NIL]),
            Ok(Word::NIL),
            "{name}"
        );
    }
    assert_eq!(
        call(&runtime, &mut ctx, "NTH", &[Word::fixnum(-1), source]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "NTHCDR", &[Word::fixnum(-1), source]),
        Err(ObjectError::TypeError)
    );

    let dotted = ncl_object::make_cons(&mut ctx, &runtime, values[0], values[1]).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "LIST-LENGTH", &[dotted]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "NTH", &[Word::fixnum(0), dotted]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "NTHCDR", &[Word::fixnum(0), dotted]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(call(&runtime, &mut ctx, "CAR", &[Word::NIL]), Ok(Word::NIL));
    assert_eq!(call(&runtime, &mut ctx, "CDR", &[Word::NIL]), Ok(Word::NIL));
}

#[test]
fn constructors_copying_and_sequence_views_return_exact_shapes() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let list_one = list(&runtime, &mut ctx, &[one]);
    let list_two = list(&runtime, &mut ctx, &[two, three]);
    let vector = ncl_object::make_simple_vector(&mut ctx, &runtime, &[one, two, three]).unwrap();
    let text = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b', 'c']).unwrap();
    let nil_type = common_lisp_symbol(&mut ctx, &runtime, "NIL");
    let list_type = common_lisp_symbol(&mut ctx, &runtime, "LIST");
    let vector_type = common_lisp_symbol(&mut ctx, &runtime, "VECTOR");
    let string_type = common_lisp_symbol(&mut ctx, &runtime, "STRING");

    let empty = call(&runtime, &mut ctx, "LIST", &[]).unwrap();
    assert_eq!(empty, Word::NIL);
    assert_eq!(list_values(&ctx, list_one), vec![one]);
    let dotted = call(&runtime, &mut ctx, "LIST*", &[one, two, three]).unwrap();
    assert_eq!(ncl_object::car(&ctx, dotted), Ok(one));
    assert_eq!(
        ncl_object::car(&ctx, ncl_object::cdr(&ctx, dotted).unwrap()),
        Ok(two)
    );
    assert_eq!(
        ncl_object::cdr(&ctx, ncl_object::cdr(&ctx, dotted).unwrap()).unwrap(),
        three
    );
    assert_eq!(
        call(&runtime, &mut ctx, "LIST*", &[]),
        Err(ObjectError::TypeError)
    );

    assert_eq!(call(&runtime, &mut ctx, "APPEND", &[]), Ok(Word::NIL));
    assert_eq!(
        call(&runtime, &mut ctx, "APPEND", &[list_one]),
        Ok(list_one)
    );
    assert_eq!(call(&runtime, &mut ctx, "NCONC", &[]), Ok(Word::NIL));
    assert_eq!(call(&runtime, &mut ctx, "NCONC", &[list_one]), Ok(list_one));
    let appended_values = call(&runtime, &mut ctx, "APPEND", &[list_one, list_two]).unwrap();
    assert_eq!(list_values(&ctx, appended_values), vec![one, two, three]);

    let appended = call(&runtime, &mut ctx, "APPEND", &[list_one, list_two]).unwrap();
    let copied_tree = call(&runtime, &mut ctx, "COPY-TREE", &[appended]).unwrap();
    assert_eq!(list_values(&ctx, copied_tree), vec![one, two, three]);
    assert_eq!(call(&runtime, &mut ctx, "COPY-TREE", &[one]), Ok(one));
    let nested = list(&runtime, &mut ctx, &[list_one, list_two]);
    let nested_copy = call(&runtime, &mut ctx, "COPY-TREE", &[nested]).unwrap();
    assert_eq!(list_values(&ctx, nested_copy).len(), 2);
    let improper = ncl_object::make_cons(&mut ctx, &runtime, one, two).unwrap();
    let improper_copy = call(&runtime, &mut ctx, "COPY-TREE", &[improper]).unwrap();
    assert_eq!(ncl_object::car(&ctx, improper_copy), Ok(one));
    assert_eq!(ncl_object::cdr(&ctx, improper_copy), Ok(two));

    assert_eq!(
        call(&runtime, &mut ctx, "LENGTH", &[Word::NIL]),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        call(&runtime, &mut ctx, "LENGTH", &[vector]),
        Ok(Word::fixnum(3))
    );
    assert_eq!(
        call(&runtime, &mut ctx, "LENGTH", &[text]),
        Ok(Word::fixnum(3))
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ELT", &[Word::NIL, Word::fixnum(0)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ELT", &[vector, Word::fixnum(-1)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ELT", &[text, Word::fixnum(1)]),
        Ok(Word::character(u32::from('b')))
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ELT", &[text, Word::fixnum(3)]),
        Err(ObjectError::TypeError)
    );

    let list_subseq = call(
        &runtime,
        &mut ctx,
        "SUBSEQ",
        &[appended, Word::fixnum(1), Word::fixnum(3)],
    )
    .unwrap();
    assert_eq!(list_values(&ctx, list_subseq), vec![two, three]);
    let vector_subseq = call(
        &runtime,
        &mut ctx,
        "SUBSEQ",
        &[vector, Word::fixnum(0), Word::fixnum(2)],
    )
    .unwrap();
    assert_eq!(vector_values(&ctx, vector_subseq), vec![one, two]);
    let string_subseq = call(&runtime, &mut ctx, "SUBSEQ", &[text, Word::fixnum(1)]).unwrap();
    assert_eq!(string_value(&ctx, string_subseq), "bc");
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SUBSEQ",
            &[vector, Word::fixnum(3), Word::fixnum(1)]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SUBSEQ",
            &[vector, Word::fixnum(0), Word::fixnum(4)]
        ),
        Err(ObjectError::TypeError)
    );

    assert_eq!(
        call(&runtime, &mut ctx, "CONCATENATE", &[nil_type, list_one]),
        Ok(Word::NIL)
    );
    let list_concat = call(
        &runtime,
        &mut ctx,
        "CONCATENATE",
        &[list_type, list_one, vector],
    )
    .unwrap();
    assert_eq!(list_values(&ctx, list_concat), vec![one, one, two, three]);
    let vector_concat = call(
        &runtime,
        &mut ctx,
        "CONCATENATE",
        &[vector_type, list_one, vector],
    )
    .unwrap();
    assert_eq!(
        vector_values(&ctx, vector_concat),
        vec![one, one, two, three]
    );
    let chars = list(
        &runtime,
        &mut ctx,
        &[
            Word::character(u32::from('x')),
            Word::character(u32::from('y')),
        ],
    );
    let string_concat = call(&runtime, &mut ctx, "CONCATENATE", &[string_type, chars]).unwrap();
    assert_eq!(string_value(&ctx, string_concat), "xy");
    assert_eq!(
        call(&runtime, &mut ctx, "CONCATENATE", &[one, list_one]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "CONCATENATE", &[string_type, list_one]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn higher_order_mapping_returns_shortest_results_and_mutates_destinations() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let left = list(&runtime, &mut ctx, &[one, two, three]);
    let right = list(&runtime, &mut ctx, &[Word::fixnum(9), Word::fixnum(8)]);
    let list_fn = runtime.function(&mut ctx, "COMMON-LISP", "LIST").unwrap();
    let list_fn = FunctionObject::try_from(list_fn).unwrap().as_word();
    let car_fn = runtime.function(&mut ctx, "COMMON-LISP", "CAR").unwrap();
    let car_fn = FunctionObject::try_from(car_fn).unwrap().as_word();
    let string_type = common_lisp_symbol(&mut ctx, &runtime, "STRING");

    let mapped = call(&runtime, &mut ctx, "MAPCAR", &[list_fn, left, right]).unwrap();
    let mapped_values = list_values(&ctx, mapped);
    assert_eq!(mapped_values.len(), 2);
    assert_eq!(
        list_values(&ctx, mapped_values[0]),
        vec![one, Word::fixnum(9)]
    );
    assert_eq!(
        list_values(&ctx, mapped_values[1]),
        vec![two, Word::fixnum(8)]
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MAPC", &[list_fn, left, right]),
        Ok(left)
    );
    let maplist = call(&runtime, &mut ctx, "MAPLIST", &[list_fn, left]).unwrap();
    let maplist_values = list_values(&ctx, maplist);
    assert_eq!(maplist_values.len(), 3);
    let first_tail = list_values(&ctx, maplist_values[0]);
    assert_eq!(first_tail.len(), 1);
    assert_eq!(list_values(&ctx, first_tail[0]), vec![one, two, three]);
    let second_tail = list_values(&ctx, maplist_values[1]);
    assert_eq!(second_tail.len(), 1);
    assert_eq!(list_values(&ctx, second_tail[0]), vec![two, three]);
    let third_tail = list_values(&ctx, maplist_values[2]);
    assert_eq!(third_tail.len(), 1);
    assert_eq!(list_values(&ctx, third_tail[0]), vec![three]);
    assert_eq!(call(&runtime, &mut ctx, "MAPL", &[list_fn, left]), Ok(left));

    let mapcan = call(&runtime, &mut ctx, "MAPCAN", &[list_fn, left]).unwrap();
    assert_eq!(list_values(&ctx, mapcan), vec![one, two, three]);
    let mapcon = call(&runtime, &mut ctx, "MAPCON", &[list_fn, left]).unwrap();
    let left_tail = ncl_object::cdr(&ctx, left).unwrap();
    let left_last = ncl_object::cdr(&ctx, left_tail).unwrap();
    assert_eq!(list_values(&ctx, mapcon), vec![left, left_tail, left_last]);
    let map_left = list(&runtime, &mut ctx, &[one, two, three]);
    let map_right = list(&runtime, &mut ctx, &[Word::fixnum(9), Word::fixnum(8)]);
    let list_result = list(&runtime, &mut ctx, &[Word::NIL]);
    let mapped_list = call(
        &runtime,
        &mut ctx,
        "MAP",
        &[list_result, list_fn, map_left, map_right],
    )
    .unwrap();
    let mapped_list_values = list_values(&ctx, mapped_list);
    assert_eq!(
        list_values(&ctx, mapped_list_values[0]),
        vec![one, Word::fixnum(9)]
    );
    let vector_source = list(&runtime, &mut ctx, &[one, two, three]);
    let vector_result = ncl_object::make_simple_vector(&mut ctx, &runtime, &[]).unwrap();
    let mapped_vector = call(
        &runtime,
        &mut ctx,
        "MAP",
        &[vector_result, list_fn, vector_source],
    )
    .unwrap();
    let mapped_vector_values = vector_values(&ctx, mapped_vector);
    assert_eq!(list_values(&ctx, mapped_vector_values[0]), vec![one]);
    assert_eq!(list_values(&ctx, mapped_vector_values[1]), vec![two]);
    assert_eq!(list_values(&ctx, mapped_vector_values[2]), vec![three]);
    assert_eq!(
        call(&runtime, &mut ctx, "MAP", &[string_type, car_fn, left]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MAP", &[Word::NIL, list_fn]),
        Ok(Word::NIL)
    );

    let into_one = list(&runtime, &mut ctx, &[one]);
    let into_two = list(&runtime, &mut ctx, &[two]);
    let into_three = list(&runtime, &mut ctx, &[three]);
    let into_source = list(&runtime, &mut ctx, &[into_one, into_two, into_three]);
    let destination = list(&runtime, &mut ctx, &[Word::NIL, Word::NIL, Word::NIL]);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAP-INTO",
            &[destination, car_fn, into_source]
        ),
        Ok(destination)
    );
    assert_eq!(list_values(&ctx, destination), vec![one, two, three]);
    let vector_destination =
        ncl_object::make_simple_vector(&mut ctx, &runtime, &[Word::NIL, Word::NIL]).unwrap();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAP-INTO",
            &[vector_destination, car_fn, into_source]
        ),
        Ok(vector_destination)
    );
    assert_eq!(vector_values(&ctx, vector_destination), vec![one, two]);
    let short_destination = list(&runtime, &mut ctx, &[Word::NIL]);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAP-INTO",
            &[short_destination, car_fn, into_source]
        ),
        Ok(short_destination)
    );
    assert_eq!(list_values(&ctx, short_destination), vec![one]);
    let string_destination = ncl_object::make_string(&mut ctx, &runtime, &['a']).unwrap();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAP-INTO",
            &[string_destination, car_fn, into_source]
        ),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn predicate_families_short_circuit_and_reduce_options_are_observable() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let source = list(&runtime, &mut ctx, &[one, two, three]);
    let atom = runtime.function(&mut ctx, "COMMON-LISP", "ATOM").unwrap();
    let atom = FunctionObject::try_from(atom).unwrap().as_word();
    let consp = runtime.function(&mut ctx, "COMMON-LISP", "CONSP").unwrap();
    let consp = FunctionObject::try_from(consp).unwrap().as_word();
    let list_fn = runtime.function(&mut ctx, "COMMON-LISP", "LIST").unwrap();
    let list_fn = FunctionObject::try_from(list_fn).unwrap().as_word();
    let true_result = call(&runtime, &mut ctx, "EVERY", &[atom, source]);
    assert_eq!(true_result, Ok(Word::TRUE));
    assert_eq!(
        call(&runtime, &mut ctx, "SOME", &[consp, source]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "NOTANY", &[consp, source]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "NOTEVERY", &[consp, source]),
        Ok(Word::TRUE)
    );
    let nested_item = list(&runtime, &mut ctx, &[one]);
    let nested = list(&runtime, &mut ctx, &[nested_item]);
    assert_eq!(
        call(&runtime, &mut ctx, "EVERY", &[atom, nested]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SOME", &[consp, nested]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "EVERY", &[atom, Word::NIL]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SOME", &[atom, Word::NIL]),
        Ok(Word::NIL)
    );

    let start = keyword(&mut ctx, &runtime, "START");
    let end = keyword(&mut ctx, &runtime, "END");
    let from_end = keyword(&mut ctx, &runtime, "FROM-END");
    let initial = keyword(&mut ctx, &runtime, "INITIAL-VALUE");
    let key = keyword(&mut ctx, &runtime, "KEY");
    let keyed_one = list(&runtime, &mut ctx, &[one]);
    let keyed_two = list(&runtime, &mut ctx, &[two]);
    let keyed = list(&runtime, &mut ctx, &[keyed_one, keyed_two]);
    let car = runtime.function(&mut ctx, "COMMON-LISP", "CAR").unwrap();
    let car = FunctionObject::try_from(car).unwrap().as_word();
    let reduced = call(
        &runtime,
        &mut ctx,
        "REDUCE",
        &[
            list_fn,
            keyed,
            key,
            car,
            start,
            Word::fixnum(0),
            end,
            Word::fixnum(2),
        ],
    )
    .unwrap();
    assert_eq!(list_values(&ctx, reduced), vec![one, two]);
    let reversed = call(
        &runtime,
        &mut ctx,
        "REDUCE",
        &[
            list_fn,
            source,
            from_end,
            Word::TRUE,
            initial,
            Word::fixnum(0),
        ],
    )
    .unwrap();
    let reversed_values = list_values(&ctx, reversed);
    assert_eq!(reversed_values[0], one);
    let reversed_tail = list_values(&ctx, reversed_values[1]);
    assert_eq!(reversed_tail[0], two);
    assert_eq!(
        list_values(&ctx, reversed_tail[1]),
        vec![three, Word::fixnum(0)]
    );
    assert_eq!(
        call(&runtime, &mut ctx, "REDUCE", &[list_fn, Word::NIL]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "REDUCE",
            &[list_fn, source, start, Word::fixnum(-1)]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "REDUCE",
            &[
                list_fn,
                source,
                end,
                Word::fixnum(1),
                start,
                Word::fixnum(2)
            ]
        ),
        Err(ObjectError::TypeError)
    );
    let unknown = keyword(&mut ctx, &runtime, "UNKNOWN-REDUCE-OPTION");
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "REDUCE",
            &[list_fn, source, unknown, Word::TRUE]
        ),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn destructive_filters_and_predicate_selection_honor_ranges_and_types() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let four = Word::fixnum(4);
    let start = keyword(&mut ctx, &runtime, "START");
    let end = keyword(&mut ctx, &runtime, "END");
    let count = keyword(&mut ctx, &runtime, "COUNT");
    let from_end = keyword(&mut ctx, &runtime, "FROM-END");
    let test_not = keyword(&mut ctx, &runtime, "TEST-NOT");
    let source = list(&runtime, &mut ctx, &[one, two, one, three]);
    let atom = runtime.function(&mut ctx, "COMMON-LISP", "ATOM").unwrap();
    let atom = FunctionObject::try_from(atom).unwrap().as_word();
    let consp = runtime.function(&mut ctx, "COMMON-LISP", "CONSP").unwrap();
    let consp = FunctionObject::try_from(consp).unwrap().as_word();
    let equal = runtime.function(&mut ctx, "COMMON-LISP", "EQUAL").unwrap();
    let equal = FunctionObject::try_from(equal).unwrap().as_word();

    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "REMOVE-IF",
            &[atom, source, start, Word::fixnum(1), end, Word::fixnum(3)]
        )
        .map(|value| list_values(&ctx, value)),
        Ok(vec![one, three])
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "REMOVE-IF-NOT",
            &[atom, source, count, one]
        )
        .map(|value| list_values(&ctx, value)),
        Ok(vec![one, two, one, three])
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SUBSTITUTE-IF",
            &[
                four,
                atom,
                source,
                from_end,
                Word::TRUE,
                count,
                Word::fixnum(2)
            ]
        )
        .map(|value| list_values(&ctx, value)),
        Ok(vec![one, two, four, four])
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SUBSTITUTE-IF-NOT",
            &[four, atom, source, test_not, equal]
        )
        .map(|value| list_values(&ctx, value)),
        Ok(vec![one, two, one, three])
    );
    assert_eq!(
        call(&runtime, &mut ctx, "DELETE-IF", &[atom, source, count, one])
            .map(|value| list_values(&ctx, value)),
        Ok(vec![two, one, three])
    );
    assert_eq!(
        call(&runtime, &mut ctx, "DELETE-IF-NOT", &[consp, source])
            .map(|value| list_values(&ctx, value)),
        Ok(Vec::new())
    );

    let vector = ncl_object::make_simple_vector(&mut ctx, &runtime, &[one, two, three]).unwrap();
    let text = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b', 'c']).unwrap();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "FILL",
            &[vector, four, start, Word::fixnum(1), end, Word::fixnum(3)]
        ),
        Ok(vector)
    );
    assert_eq!(vector_values(&ctx, vector), vec![one, four, four]);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "FILL",
            &[
                text,
                Word::character(u32::from('z')),
                start,
                Word::fixnum(1)
            ]
        ),
        Ok(text)
    );
    assert_eq!(string_value(&ctx, text), "azz");
    let replacement = list(&runtime, &mut ctx, &[two, three]);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "REPLACE",
            &[vector, replacement, start, Word::fixnum(0)]
        ),
        Ok(vector)
    );
    assert_eq!(vector_values(&ctx, vector), vec![two, three, four]);
    assert_eq!(
        call(&runtime, &mut ctx, "FILL", &[text, four]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "REPLACE", &[text, replacement]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "FILL",
            &[vector, four, start, Word::fixnum(3), end, Word::fixnum(1)]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "REPLACE", &[vector, Word::fixnum(9),]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn option_and_registration_boundaries_report_precise_errors() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let source = list(&runtime, &mut ctx, &[one, two]);
    let start = keyword(&mut ctx, &runtime, "START");
    let test = keyword(&mut ctx, &runtime, "TEST");
    let test_not = keyword(&mut ctx, &runtime, "TEST-NOT");
    let key = keyword(&mut ctx, &runtime, "KEY");
    let equal = runtime.function(&mut ctx, "COMMON-LISP", "EQUAL").unwrap();
    let equal = FunctionObject::try_from(equal).unwrap().as_word();

    for name in [
        "FIND",
        "POSITION",
        "COUNT",
        "SEARCH",
        "MISMATCH",
        "REMOVE",
        "SUBSTITUTE",
        "FIND-IF",
        "POSITION-IF",
        "COUNT-IF",
        "REMOVE-IF",
        "SUBSTITUTE-IF",
    ] {
        assert_eq!(
            call(&runtime, &mut ctx, name, &[]),
            Err(ObjectError::TypeError),
            "{name}"
        );
    }
    assert_eq!(
        call(&runtime, &mut ctx, "FIND", &[one, source, start]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "FIND",
            &[one, source, start, Word::fixnum(-1)]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "FIND",
            &[one, source, test, equal, test_not, equal]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "FIND",
            &[one, source, key, Word::fixnum(9)]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "FIND",
            &[one, source, test, Word::fixnum(9)]
        ),
        Err(ObjectError::TypeError)
    );

    let list_type = common_lisp_symbol(&mut ctx, &runtime, "LIST");
    let list_fn = runtime.function(&mut ctx, "COMMON-LISP", "LIST").unwrap();
    let list_fn = FunctionObject::try_from(list_fn).unwrap().as_word();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAP",
            &[list_type, Word::fixnum(9), source]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAP-INTO",
            &[source, Word::fixnum(9), source]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MAPCAR", &[Word::fixnum(9), source]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MAPCAN", &[list_fn, Word::fixnum(9)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "REDUCE", &[list_fn, source, Word::NIL]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "REDUCE", &[list_fn, source, start]),
        Err(ObjectError::TypeError)
    );

    let bogus = common_lisp_symbol(&mut ctx, &runtime, "NOT-A-DESTINATION");
    assert_eq!(
        call(&runtime, &mut ctx, "CONCATENATE", &[bogus, source]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "LENGTH", &[one]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "COPY-SEQ", &[one]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "REVERSE", &[one]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "NREVERSE", &[one]),
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
    assert_eq!(
        call(&runtime, &mut ctx, "ENDP", &[one]),
        Err(ObjectError::TypeError)
    );
    let dotted = ncl_object::make_cons(&mut ctx, &runtime, one, two).unwrap();
    assert_eq!(call(&runtime, &mut ctx, "LISTP", &[dotted]), Ok(Word::NIL));
}
