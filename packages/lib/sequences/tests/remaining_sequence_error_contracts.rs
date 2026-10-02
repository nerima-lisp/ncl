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

fn dotted(runtime: &Runtime, ctx: &mut ThreadContext, head: Word, tail: Word) -> Word {
    call(runtime, ctx, "CONS", &[head, tail]).unwrap()
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
fn list_and_sequence_edges_return_precise_type_errors() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let source = list(&runtime, &mut ctx, &[one, two]);
    let dotted_value = dotted(&runtime, &mut ctx, one, two);
    let vector = ncl_object::make_simple_vector(&mut ctx, &runtime, &[one, two]).unwrap();
    let text = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();

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
    assert_eq!(
        call(&runtime, &mut ctx, "NTH", &[Word::fixnum(-1), source]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "NTHCDR", &[Word::fixnum(-1), source]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "NTH",
            &[Word::character(u32::from('x')), source]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "LIST-LENGTH", &[dotted_value]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "COPY-LIST", &[dotted_value]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "COPY-SEQ", &[dotted_value]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "REVERSE", &[dotted_value]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "NREVERSE", &[vector]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "NREVERSE", &[text]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ELT", &[source, Word::fixnum(-1)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ELT", &[source, Word::fixnum(2)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(call(&runtime, &mut ctx, "ELT", &[source, one]), Ok(two));
    assert_eq!(
        call(&runtime, &mut ctx, "SUBSEQ", &[source, Word::fixnum(-1)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SUBSEQ",
            &[source, Word::fixnum(0), Word::fixnum(-1)]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SUBSEQ",
            &[source, Word::fixnum(2), Word::fixnum(1)]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SUBSEQ",
            &[source, Word::fixnum(0), Word::fixnum(3)]
        ),
        Err(ObjectError::TypeError)
    );

    let list_type = common_lisp_symbol(&mut ctx, &runtime, "LIST");
    let vector_type = common_lisp_symbol(&mut ctx, &runtime, "VECTOR");
    let string_type = common_lisp_symbol(&mut ctx, &runtime, "STRING");
    let unknown_type = common_lisp_symbol(&mut ctx, &runtime, "NOT-A-DESTINATION");
    assert_eq!(
        call(&runtime, &mut ctx, "CONCATENATE", &[one, source]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "CONCATENATE", &[unknown_type, source]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "CONCATENATE",
            &[list_type, dotted_value]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "CONCATENATE", &[string_type, source]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "CONCATENATE",
            &[vector_type, dotted_value]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "LIST*", &[]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "APPEND", &[dotted_value, source]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "NCONC", &[dotted_value, source]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "RPLACA", &[one, two]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "RPLACD", &[one, two]),
        Err(ObjectError::TypeError)
    );

    let malformed_plist = dotted(&runtime, &mut ctx, one, two);
    assert_eq!(
        call(&runtime, &mut ctx, "GETF", &[malformed_plist, one]),
        Err(ObjectError::TypeError)
    );
    let missing_value = list(&runtime, &mut ctx, &[one]);
    assert_eq!(
        call(&runtime, &mut ctx, "GETF", &[missing_value, one]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "GETF", &[source, one, two, three()]),
        Err(ObjectError::TypeError)
    );
}

const fn three() -> Word {
    Word::fixnum(3)
}

#[test]
fn selection_option_parser_rejects_malformed_values_and_callbacks() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let source = list(&runtime, &mut ctx, &[one, two, one]);
    let equal = function(&runtime, &mut ctx, "EQUAL");
    let test = keyword(&mut ctx, &runtime, "TEST");
    let test_not = keyword(&mut ctx, &runtime, "TEST-NOT");
    let start = keyword(&mut ctx, &runtime, "START");
    let end = keyword(&mut ctx, &runtime, "END");
    let count = keyword(&mut ctx, &runtime, "COUNT");
    let from_end = keyword(&mut ctx, &runtime, "FROM-END");
    let key = keyword(&mut ctx, &runtime, "KEY");

    for name in ["FIND", "POSITION", "COUNT", "REMOVE", "SUBSTITUTE"] {
        let args = if name == "SUBSTITUTE" {
            vec![three(), one, source, start]
        } else {
            vec![one, source, start]
        };
        assert_eq!(
            call(&runtime, &mut ctx, name, &args),
            Err(ObjectError::TypeError),
            "{name} missing START value"
        );
    }
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
            &[one, source, end, Word::character(u32::from('x'))]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "COUNT",
            &[one, source, count, Word::fixnum(-1)]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "POSITION", &[one, source, from_end]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "FIND", &[one, source, key]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "FIND", &[one, source, test]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "FIND", &[one, source, test_not]),
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
        call(&runtime, &mut ctx, "FIND", &[one, source, test, one]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "FIND", &[one, source, key, one]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "FIND", &[one, Word::fixnum(7)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "FIND",
            &[one, source, start, Word::fixnum(2), end, Word::fixnum(1)]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "FIND",
            &[one, source, end, Word::fixnum(9)]
        ),
        Err(ObjectError::TypeError)
    );

    let dotted = dotted(&runtime, &mut ctx, one, two);
    for name in ["SEARCH", "MISMATCH"] {
        assert_eq!(
            call(&runtime, &mut ctx, name, &[source, dotted]),
            Err(ObjectError::TypeError),
            "{name} dotted right sequence"
        );
        assert_eq!(
            call(&runtime, &mut ctx, name, &[Word::fixnum(7), source]),
            Err(ObjectError::TypeError),
            "{name} non-sequence left"
        );
        assert_eq!(
            call(&runtime, &mut ctx, name, &[source]),
            Err(ObjectError::TypeError),
            "{name} missing right sequence"
        );
    }
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SEARCH",
            &[source, source, start, Word::fixnum(-1)]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MISMATCH",
            &[source, source, test, equal, test_not, equal]
        ),
        Err(ObjectError::TypeError)
    );

    let atom = function(&runtime, &mut ctx, "ATOM");
    for name in [
        "FIND-IF",
        "FIND-IF-NOT",
        "POSITION-IF",
        "POSITION-IF-NOT",
        "COUNT-IF",
        "COUNT-IF-NOT",
    ] {
        assert_eq!(
            call(&runtime, &mut ctx, name, &[one, source]),
            Err(ObjectError::TypeError),
            "{name} invalid predicate"
        );
        assert_eq!(
            call(&runtime, &mut ctx, name, &[atom, dotted]),
            Err(ObjectError::TypeError),
            "{name} dotted sequence"
        );
    }
}

#[test]
fn destructive_sequence_operations_reject_bad_ranges_and_element_types() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let source = list(&runtime, &mut ctx, &[one, two]);
    let vector = ncl_object::make_simple_vector(&mut ctx, &runtime, &[one, two]).unwrap();
    let text = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();
    let start = keyword(&mut ctx, &runtime, "START");
    let end = keyword(&mut ctx, &runtime, "END");
    let from_end = keyword(&mut ctx, &runtime, "FROM-END");

    for name in ["FILL", "REPLACE"] {
        assert_eq!(
            call(&runtime, &mut ctx, name, &[]),
            Err(ObjectError::TypeError),
            "{name} missing required args"
        );
        assert_eq!(
            call(&runtime, &mut ctx, name, &[one]),
            Err(ObjectError::TypeError),
            "{name} missing sequence"
        );
    }
    assert_eq!(
        call(&runtime, &mut ctx, "FILL", &[source, one, start]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "FILL",
            &[source, one, start, Word::fixnum(-1)]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "FILL",
            &[source, one, start, Word::fixnum(2), end, Word::fixnum(1)]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "FILL",
            &[source, one, start, Word::fixnum(0), end, Word::fixnum(3)]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "FILL",
            &[source, one, from_end, Word::TRUE]
        ),
        Ok(source)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "FILL",
            &[source, Word::character(u32::from('x'))]
        ),
        Ok(source)
    );
    assert_eq!(
        list_values(&ctx, source),
        vec![Word::character(u32::from('x')); 2]
    );

    assert_eq!(
        call(&runtime, &mut ctx, "FILL", &[text, one]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "FILL",
            &[vector, one, start, Word::fixnum(2), end, Word::fixnum(1)]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "REPLACE",
            &[source, vector, start, Word::fixnum(2), end, Word::fixnum(1)]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "REPLACE", &[source, text]),
        Ok(source)
    );
    assert_eq!(
        list_values(&ctx, source),
        vec![
            Word::character(u32::from('a')),
            Word::character(u32::from('b'))
        ]
    );
    assert_eq!(
        call(&runtime, &mut ctx, "REPLACE", &[text, source]),
        Ok(text)
    );
}

#[test]
fn higher_order_and_reduce_errors_preserve_exact_boundaries() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let source = list(&runtime, &mut ctx, &[one, two]);
    let car = function(&runtime, &mut ctx, "CAR");
    let list_fn = function(&runtime, &mut ctx, "LIST");
    let equal = function(&runtime, &mut ctx, "EQUAL");
    let start = keyword(&mut ctx, &runtime, "START");
    let end = keyword(&mut ctx, &runtime, "END");
    let initial = keyword(&mut ctx, &runtime, "INITIAL-VALUE");
    let key = keyword(&mut ctx, &runtime, "KEY");
    let unknown = keyword(&mut ctx, &runtime, "UNKNOWN-REDUCE-OPTION");

    for name in ["MAPCAR", "MAPC", "MAPLIST", "MAPL", "MAPCAN", "MAPCON"] {
        assert_eq!(
            call(&runtime, &mut ctx, name, &[one, source]),
            Err(ObjectError::TypeError),
            "{name} invalid function"
        );
        assert_eq!(
            call(&runtime, &mut ctx, name, &[car, one]),
            Err(ObjectError::TypeError),
            "{name} invalid sequence"
        );
    }
    assert_eq!(
        call(&runtime, &mut ctx, "MAP", &[source, one, source]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MAP", &[source, list_fn, one]),
        Err(ObjectError::TypeError)
    );
    let text = ncl_object::make_string(&mut ctx, &runtime, &['a']).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "MAP", &[text, list_fn, source]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MAP-INTO", &[one, car, source]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MAP-INTO", &[source, one, source]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MAP-INTO", &[source, car, one]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "EVERY", &[one, source]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SOME", &[car, one]),
        Err(ObjectError::TypeError)
    );

    assert_eq!(
        call(&runtime, &mut ctx, "REDUCE", &[one, source]),
        Err(ObjectError::TypeError)
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
                start,
                Word::fixnum(1),
                end,
                Word::fixnum(0)
            ]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "REDUCE", &[list_fn, source, key]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "REDUCE",
            &[list_fn, source, unknown, one]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "REDUCE",
            &[list_fn, Word::NIL, initial, one]
        ),
        Ok(one)
    );
    let reduced = call(
        &runtime,
        &mut ctx,
        "REDUCE",
        &[list_fn, source, initial, one],
    )
    .unwrap();
    let reduced_values = list_values(&ctx, reduced);
    assert_eq!(reduced_values.len(), 2);
    assert_eq!(list_values(&ctx, reduced_values[0]), vec![one, one]);
    assert_eq!(reduced_values[1], two);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "REDUCE",
            &[list_fn, source, initial, one, key, car]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "REDUCE", &[list_fn, source, key, equal]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn ordered_sets_and_association_walkers_reject_malformed_inputs() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let source = list(&runtime, &mut ctx, &[one, two]);
    let dotted = dotted(&runtime, &mut ctx, one, two);
    let equal = function(&runtime, &mut ctx, "EQUAL");
    let car = function(&runtime, &mut ctx, "CAR");
    let test = keyword(&mut ctx, &runtime, "TEST");
    let test_not = keyword(&mut ctx, &runtime, "TEST-NOT");
    let key = keyword(&mut ctx, &runtime, "KEY");

    for name in [
        "UNION",
        "INTERSECTION",
        "SET-DIFFERENCE",
        "SET-EXCLUSIVE-OR",
        "SUBSETP",
    ] {
        assert_eq!(
            call(&runtime, &mut ctx, name, &[dotted, source]),
            Err(ObjectError::TypeError),
            "{name} dotted first"
        );
        assert_eq!(
            call(&runtime, &mut ctx, name, &[source, dotted]),
            Err(ObjectError::TypeError),
            "{name} dotted second"
        );
        assert_eq!(
            call(&runtime, &mut ctx, name, &[source, source, test]),
            Err(ObjectError::TypeError),
            "{name} missing option value"
        );
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                name,
                &[source, source, test, equal, test_not, equal]
            ),
            Err(ObjectError::TypeError),
            "{name} conflicting tests"
        );
        assert_eq!(
            call(&runtime, &mut ctx, name, &[source, source, test, one]),
            Err(ObjectError::TypeError),
            "{name} invalid test function"
        );
    }
    assert_eq!(
        call(&runtime, &mut ctx, "UNION", &[source, source, key, one]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ADJOIN", &[three, dotted]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ADJOIN", &[three, source, key]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ADJOIN",
            &[three, source, key, car, test, equal, test_not, equal]
        ),
        Err(ObjectError::TypeError)
    );

    for name in ["MEMBER", "ASSOC", "RASSOC"] {
        assert_eq!(
            call(&runtime, &mut ctx, name, &[three, dotted]),
            Err(ObjectError::TypeError),
            "{name} dotted input"
        );
        assert_eq!(
            call(&runtime, &mut ctx, name, &[one, source, test]),
            Err(ObjectError::TypeError),
            "{name} missing test value"
        );
        assert_eq!(
            call(&runtime, &mut ctx, name, &[one, source, test, one]),
            Err(ObjectError::TypeError),
            "{name} invalid test function"
        );
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                name,
                &[one, source, test, equal, test_not, equal]
            ),
            Err(ObjectError::TypeError),
            "{name} conflicting tests"
        );
    }
    let malformed_pair = list(&runtime, &mut ctx, &[one, dotted]);
    let malformed_alist = list(&runtime, &mut ctx, &[malformed_pair]);
    assert_eq!(
        call(&runtime, &mut ctx, "ASSOC", &[one, malformed_alist]),
        Ok(malformed_pair)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "RASSOC", &[one, malformed_alist]),
        Ok(Word::NIL)
    );

    assert_eq!(
        call(&runtime, &mut ctx, "SORT", &[dotted, equal]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SORT", &[source, one]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "STABLE-SORT", &[source, one]),
        Err(ObjectError::TypeError)
    );
    let vector = ncl_object::make_simple_vector(&mut ctx, &runtime, &[two, one]).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "SORT", &[vector, one]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SORT", &[vector, car, key]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn empty_and_non_sequence_results_remain_exact() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let atom = function(&runtime, &mut ctx, "ATOM");
    let list_fn = function(&runtime, &mut ctx, "LIST");
    let empty = list(&runtime, &mut ctx, &[]);
    let vector = ncl_object::make_simple_vector(&mut ctx, &runtime, &[]).unwrap();

    assert_eq!(
        call(&runtime, &mut ctx, "MAP", &[empty, list_fn]),
        Ok(Word::NIL)
    );
    let mapped_vector = call(&runtime, &mut ctx, "MAP", &[vector, list_fn]).unwrap();
    assert_eq!(vector_values(&ctx, mapped_vector), Vec::<Word>::new());
    assert_eq!(
        call(&runtime, &mut ctx, "MAPCAR", &[list_fn, empty]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MAPC", &[list_fn, empty]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MAPLIST", &[list_fn, empty]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MAPL", &[list_fn, empty]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MAPCAN", &[list_fn, empty]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MAPCON", &[list_fn, empty]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "EVERY", &[atom, empty]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SOME", &[atom, empty]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "NOTANY", &[atom, empty]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "NOTEVERY", &[atom, empty]),
        Ok(Word::NIL)
    );
    let initial_value = keyword(&mut ctx, &runtime, "INITIAL-VALUE");
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "REDUCE",
            &[list_fn, empty, initial_value, one]
        ),
        Ok(one)
    );
    let appended = call(&runtime, &mut ctx, "APPEND", &[empty, empty]).unwrap();
    assert_eq!(list_values(&ctx, appended), Vec::<Word>::new());
}
