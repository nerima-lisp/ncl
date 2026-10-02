#![allow(clippy::similar_names, missing_docs, clippy::unwrap_used)]

use std::collections::HashMap;

use ncl_object::{FunctionObject, ObjectError, Package, Runtime, ThreadContext, Word};

fn setup() -> (Runtime, ThreadContext, HashMap<String, FunctionObject>) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_sequences::register(&runtime).unwrap();
    let names = [
        "ADJOIN",
        "ASSOC",
        "COUNT",
        "EVERY",
        "FIND",
        "INTERSECTION",
        "LIST",
        "MAP",
        "MAP-INTO",
        "MAPC",
        "MAPCAN",
        "MAPCAR",
        "MAPCON",
        "MAPL",
        "MAPLIST",
        "MEMBER",
        "NOTANY",
        "NOTEVERY",
        "REDUCE",
        "REMOVE",
        "REMOVE-IF",
        "REMOVE-IF-NOT",
        "RASSOC",
        "SET-DIFFERENCE",
        "SET-EXCLUSIVE-OR",
        "SORT",
        "STABLE-SORT",
        "SUBSETP",
        "SUBSTITUTE",
        "SUBSTITUTE-IF",
        "SUBSTITUTE-IF-NOT",
        "SOME",
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

#[test]
fn sequence_entry_points_report_exact_missing_argument_errors() {
    let (runtime, mut ctx, functions) = setup();
    let names = [
        "ADJOIN",
        "ASSOC",
        "EVERY",
        "INTERSECTION",
        "MAP",
        "MAP-INTO",
        "MAPC",
        "MAPCAN",
        "MAPCAR",
        "MAPCON",
        "MAPL",
        "MAPLIST",
        "MEMBER",
        "NOTANY",
        "NOTEVERY",
        "REDUCE",
        "REMOVE",
        "REMOVE-IF",
        "REMOVE-IF-NOT",
        "RASSOC",
        "SET-DIFFERENCE",
        "SET-EXCLUSIVE-OR",
        "SORT",
        "STABLE-SORT",
        "SUBSETP",
        "SUBSTITUTE",
        "SUBSTITUTE-IF",
        "SUBSTITUTE-IF-NOT",
        "SOME",
        "UNION",
    ];
    for name in names {
        assert_eq!(
            call(&runtime, &mut ctx, &functions, name, &[]),
            Err(ObjectError::TypeError),
            "{name} must reject missing required arguments"
        );
    }
}

#[test]
fn regular_selection_transformations_return_exact_values_for_test_not_and_count() {
    let (runtime, mut ctx, functions) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let nine = Word::fixnum(9);
    let source = list(&runtime, &mut ctx, &functions, &[one, two, two, three]);
    let equal = runtime.function(&mut ctx, "COMMON-LISP", "EQUAL").unwrap();
    let equal = FunctionObject::try_from(equal).unwrap().as_word();
    let test_not = keyword(&mut ctx, &runtime, "TEST-NOT");
    let from_end = keyword(&mut ctx, &runtime, "FROM-END");
    let count = keyword(&mut ctx, &runtime, "COUNT");

    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "FIND",
            &[two, source, test_not, equal],
        ),
        Ok(one)
    );
    let removed = call(
        &runtime,
        &mut ctx,
        &functions,
        "REMOVE",
        &[two, source, from_end, Word::TRUE, count, one],
    )
    .unwrap();
    assert_eq!(list_values(&ctx, removed), vec![one, two, three]);
    let substituted = call(
        &runtime,
        &mut ctx,
        &functions,
        "SUBSTITUTE",
        &[nine, two, source, count, one],
    )
    .unwrap();
    assert_eq!(list_values(&ctx, substituted), vec![one, nine, two, three]);
}

#[test]
fn map_sort_and_order_set_options_return_concrete_values_and_errors() {
    let (runtime, mut ctx, functions) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let source = list(&runtime, &mut ctx, &functions, &[three, one, two]);
    let list_function = functions["LIST"].as_word();
    let vector = ncl_object::make_simple_vector(&mut ctx, &runtime, &[Word::NIL; 3]).unwrap();
    let mapped = call(
        &runtime,
        &mut ctx,
        &functions,
        "MAP",
        &[vector, list_function, source],
    )
    .unwrap();
    let mapped_first = ncl_object::simple_vector_ref(&ctx, mapped, 0).unwrap();
    assert_eq!(list_values(&ctx, mapped_first), vec![three]);
    let text = ncl_object::make_string(&mut ctx, &runtime, &['x']).unwrap();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "MAP",
            &[text, list_function, source],
        ),
        Err(ObjectError::TypeError)
    );

    let sorted = call(
        &runtime,
        &mut ctx,
        &functions,
        "STABLE-SORT",
        &[source, list_function],
    )
    .unwrap();
    assert_eq!(list_values(&ctx, sorted), vec![two, one, three]);

    let pair_one = list(&runtime, &mut ctx, &functions, &[one, Word::fixnum(10)]);
    let pair_two = list(&runtime, &mut ctx, &functions, &[two, Word::fixnum(20)]);
    let pairs = list(&runtime, &mut ctx, &functions, &[pair_one, pair_two]);
    let key = keyword(&mut ctx, &runtime, "KEY");
    let test = keyword(&mut ctx, &runtime, "TEST");
    let car = runtime.function(&mut ctx, "COMMON-LISP", "CAR").unwrap();
    let car = FunctionObject::try_from(car).unwrap().as_word();
    let equal = runtime.function(&mut ctx, "COMMON-LISP", "EQUAL").unwrap();
    let equal = FunctionObject::try_from(equal).unwrap().as_word();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "ADJOIN",
            &[pair_one, pairs, key, car, test, equal],
        ),
        Ok(pairs)
    );
    let test_not = keyword(&mut ctx, &runtime, "TEST-NOT");
    let member_list = list(&runtime, &mut ctx, &functions, &[one, two]);
    let member_tail = call(
        &runtime,
        &mut ctx,
        &functions,
        "MEMBER",
        &[one, member_list, test_not, equal],
    );
    let expected_tail = ncl_object::cdr(&ctx, member_list).unwrap();
    assert_eq!(member_tail, Ok(expected_tail));
}
