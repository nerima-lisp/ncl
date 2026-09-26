//! Integration coverage for the registered sequence builtins.

use std::collections::HashMap;

use ncl_object::{FunctionObject, Package, Runtime, ThreadContext, Word};

fn call(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    functions: &HashMap<String, FunctionObject>,
    name: &str,
    args: &[Word],
) -> Word {
    let function = *functions
        .get(name)
        .unwrap_or_else(|| panic!("missing builtin {name}"));
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        let rooted_args = roots.iter().map(|root| **root).collect::<Vec<_>>();
        runtime.call_builtin(ctx, function, &rooted_args)
    })
    .unwrap_or_else(|error| panic!("{name} failed: {error:?}"))
}

fn assert_list(ctx: &mut ThreadContext, list: Word, expected: &[Word]) {
    let mut cursor = list;
    for &value in expected {
        assert!(cursor.is_cons());
        assert_eq!(ncl_object::car(ctx, cursor), Ok(value));
        cursor = ncl_object::cdr(ctx, cursor).unwrap_or_else(|error| panic!("cdr: {error:?}"));
    }
    assert_eq!(cursor, Word::NIL);
}

fn with_list(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    functions: &HashMap<String, FunctionObject>,
    values: &[Word],
    f: impl FnOnce(&mut ThreadContext, &mut Word),
) {
    let mut list = call(runtime, ctx, functions, "LIST", values);
    let root = ncl_object::push_root(ctx, &mut list);
    f(ctx, &mut list);
    assert!(ncl_object::pop_root(ctx, root));
}
fn probe(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    functions: &HashMap<String, FunctionObject>,
    name: &str,
) {
    let function = *functions
        .get(name)
        .unwrap_or_else(|| panic!("missing builtin {name}"));
    let _ = ncl_object::with_roots(ctx, &[Word::NIL], |ctx, roots| {
        runtime.call_builtin(
            ctx,
            function,
            &[**roots.first().ok_or(ncl_object::ObjectError::Layout)?],
        )
    });
}
#[test]
#[allow(clippy::too_many_lines)]
fn registered_builtins_survive_gc_stress_and_strict_forwarding() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context: {error:?}"));
    ncl_lib_sequences::register(&runtime).unwrap_or_else(|error| panic!("sequences: {error:?}"));

    let names = [
        "ATOM",
        "CONSP",
        "LISTP",
        "ENDP",
        "CAR",
        "CDR",
        "CONS",
        "RPLACA",
        "RPLACD",
        "COPY-LIST",
        "NTH",
        "NTHCDR",
        "LIST-LENGTH",
        "FIRST",
        "SECOND",
        "THIRD",
        "FOURTH",
        "FIFTH",
        "SIXTH",
        "SEVENTH",
        "EIGHTH",
        "NINTH",
        "TENTH",
        "LIST",
        "APPEND",
        "NCONC",
        "LIST*",
        "CONCATENATE",
        "LENGTH",
        "COPY-SEQ",
        "REVERSE",
        "NREVERSE",
        "ELT",
        "SUBSEQ",
        "FILL",
        "REPLACE",
        "MAPCAR",
        "MAPC",
        "MAPLIST",
        "MAPL",
        "MAPCAN",
        "MAPCON",
        "MAP",
        "MAP-INTO",
        "REDUCE",
        "EVERY",
        "SOME",
        "NOTANY",
        "NOTEVERY",
        "REMOVE",
        "SUBSTITUTE",
        "SORT",
        "STABLE-SORT",
        "UNION",
        "INTERSECTION",
        "SET-DIFFERENCE",
        "SET-EXCLUSIVE-OR",
        "SUBSETP",
        "ADJOIN",
        "ASSOC",
        "RASSOC",
        "MEMBER",
    ];
    ctx.set_gc_stress(false);
    let functions = names
        .into_iter()
        .map(|name| {
            let word = runtime
                .function(&mut ctx, "COMMON-LISP", name)
                .unwrap_or_else(|| panic!("missing builtin {name}"));
            (
                name.to_owned(),
                FunctionObject::try_from(word)
                    .unwrap_or_else(|_| panic!("{name} is not a function")),
            )
        })
        .collect::<HashMap<_, _>>();
    let package = runtime
        .find_package(&ctx, "COMMON-LISP")
        .unwrap_or_else(|| panic!("missing COMMON-LISP package"));
    let (mut list_symbol, _) = Package::from_word(package)
        .intern(&mut ctx, &runtime, "LIST")
        .unwrap_or_else(|error| panic!("intern LIST: {error:?}"));
    let list_symbol_root = ncl_object::push_root(&mut ctx, &mut list_symbol);
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let four = Word::fixnum(4);
    let five = Word::fixnum(5);
    let values = [one, two, three, four, five];
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "ATOM", &[one]),
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "CONSP", &[one]),
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "LISTP", &[Word::NIL]),
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "ENDP", &[Word::NIL]),
        Word::TRUE
    );

    with_list(&runtime, &mut ctx, &functions, &values, |ctx, list_root| {
        let list = *list_root;
        assert_eq!(call(&runtime, ctx, &functions, "CAR", &[list]), one);
        let cdr = call(&runtime, ctx, &functions, "CDR", &[list]);
        assert_list(ctx, cdr, &[two, three, four, five]);
        assert_eq!(
            call(&runtime, ctx, &functions, "NTH", &[Word::fixnum(2), list]),
            three
        );
        let nthcdr = call(
            &runtime,
            ctx,
            &functions,
            "NTHCDR",
            &[Word::fixnum(3), list],
        );
        assert_list(ctx, nthcdr, &[four, five]);
        assert_eq!(
            call(&runtime, ctx, &functions, "LIST-LENGTH", &[list]),
            Word::fixnum(5)
        );
        assert_eq!(call(&runtime, ctx, &functions, "FIRST", &[list]), one);
        assert_eq!(call(&runtime, ctx, &functions, "SECOND", &[list]), two);
        assert_eq!(call(&runtime, ctx, &functions, "THIRD", &[list]), three);
        assert_eq!(call(&runtime, ctx, &functions, "FOURTH", &[list]), four);
        assert_eq!(call(&runtime, ctx, &functions, "FIFTH", &[list]), five);
        assert_eq!(call(&runtime, ctx, &functions, "SIXTH", &[list]), Word::NIL);
        assert_eq!(
            call(&runtime, ctx, &functions, "SEVENTH", &[list]),
            Word::NIL
        );
        assert_eq!(
            call(&runtime, ctx, &functions, "EIGHTH", &[list]),
            Word::NIL
        );
        assert_eq!(call(&runtime, ctx, &functions, "NINTH", &[list]), Word::NIL);
        assert_eq!(call(&runtime, ctx, &functions, "TENTH", &[list]), Word::NIL);
    });
    with_list(&runtime, &mut ctx, &functions, &[one], |ctx, list_root| {
        let mut result = call(
            &runtime,
            ctx,
            &functions,
            "CONS",
            &[Word::fixnum(0), *list_root],
        );
        let root = ncl_object::push_root(ctx, &mut result);
        let list = *list_root;
        assert_eq!(
            call(&runtime, ctx, &functions, "CAR", &[result]),
            Word::fixnum(0)
        );
        assert_eq!(ncl_object::cdr(ctx, result), Ok(list));
        assert!(ncl_object::pop_root(ctx, root));
    });
    with_list(&runtime, &mut ctx, &functions, &[one], |ctx, list_root| {
        let list = *list_root;
        assert_eq!(
            call(&runtime, ctx, &functions, "RPLACA", &[list, two]),
            list
        );
        assert_eq!(call(&runtime, ctx, &functions, "CAR", &[list]), two);
        assert_eq!(
            call(&runtime, ctx, &functions, "RPLACD", &[list, Word::NIL]),
            list
        );
        assert_eq!(ncl_object::cdr(ctx, list), Ok(Word::NIL));
    });
    with_list(&runtime, &mut ctx, &functions, &values, |ctx, list_root| {
        let list = *list_root;
        let mut result = call(&runtime, ctx, &functions, "COPY-LIST", &[list]);
        let root = ncl_object::push_root(ctx, &mut result);
        assert_list(ctx, result, &values);
        assert_ne!(result, list);
        assert!(ncl_object::pop_root(ctx, root));
    });
    let mut list_star = call(
        &runtime,
        &mut ctx,
        &functions,
        "LIST*",
        &[one, two, three, Word::NIL],
    );
    let list_star_root = ncl_object::push_root(&mut ctx, &mut list_star);
    assert_list(&mut ctx, list_star, &[one, two, three]);
    assert!(ncl_object::pop_root(&mut ctx, list_star_root));
    with_list(
        &runtime,
        &mut ctx,
        &functions,
        &[one, two],
        |ctx, left_root| {
            let mut right = call(&runtime, ctx, &functions, "LIST", &[three]);
            let right_root = ncl_object::push_root(ctx, &mut right);
            let left = *left_root;
            let mut result = call(&runtime, ctx, &functions, "APPEND", &[left, right]);
            let result_root = ncl_object::push_root(ctx, &mut result);
            assert_list(ctx, result, &[one, two, three]);
            assert!(ncl_object::pop_root(ctx, result_root));
            assert!(ncl_object::pop_root(ctx, right_root));
        },
    );
    with_list(
        &runtime,
        &mut ctx,
        &functions,
        &[one, two],
        |ctx, left_root| {
            let mut right = call(&runtime, ctx, &functions, "LIST", &[three]);
            let right_root = ncl_object::push_root(ctx, &mut right);
            let left = *left_root;
            let result = call(&runtime, ctx, &functions, "NCONC", &[left, right]);
            assert_list(ctx, result, &[one, two, three]);
            assert!(ncl_object::pop_root(ctx, right_root));
        },
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "CONCATENATE",
            &[list_symbol]
        ),
        Word::NIL
    );
    with_list(&runtime, &mut ctx, &functions, &values, |ctx, list_root| {
        let list = *list_root;
        assert_eq!(
            call(&runtime, ctx, &functions, "LENGTH", &[list]),
            Word::fixnum(5)
        );
        let mut result = call(&runtime, ctx, &functions, "COPY-SEQ", &[list]);
        let root = ncl_object::push_root(ctx, &mut result);
        assert_list(ctx, result, &values);
        assert!(ncl_object::pop_root(ctx, root));
    });

    with_list(&runtime, &mut ctx, &functions, &values, |ctx, list_root| {
        let list = *list_root;
        let mut result = call(&runtime, ctx, &functions, "REVERSE", &[list]);
        let root = ncl_object::push_root(ctx, &mut result);
        assert_list(ctx, result, &[five, four, three, two, one]);
        assert!(ncl_object::pop_root(ctx, root));
    });

    with_list(&runtime, &mut ctx, &functions, &values, |ctx, list_root| {
        let list = *list_root;
        let mut result = call(&runtime, ctx, &functions, "NREVERSE", &[list]);
        let root = ncl_object::push_root(ctx, &mut result);
        assert_list(ctx, result, &[five, four, three, two, one]);
        assert!(ncl_object::pop_root(ctx, root));
    });

    with_list(&runtime, &mut ctx, &functions, &values, |ctx, list_root| {
        let list = *list_root;
        assert_eq!(
            call(&runtime, ctx, &functions, "ELT", &[list, Word::fixnum(3)]),
            four
        );
        let mut result = call(
            &runtime,
            ctx,
            &functions,
            "SUBSEQ",
            &[list, Word::fixnum(1), Word::fixnum(4)],
        );
        let root = ncl_object::push_root(ctx, &mut result);
        assert_list(ctx, result, &[two, three, four]);
        assert!(ncl_object::pop_root(ctx, root));
    });

    with_list(&runtime, &mut ctx, &functions, &values, |ctx, list_root| {
        let list = *list_root;
        assert_eq!(
            call(&runtime, ctx, &functions, "FILL", &[list, Word::fixnum(9)]),
            list
        );
        assert_list(ctx, *list_root, &[Word::fixnum(9); 5]);
    });

    with_list(
        &runtime,
        &mut ctx,
        &functions,
        &[one, two, three, four],
        |ctx, destination_root| {
            let mut source = call(&runtime, ctx, &functions, "LIST", &values);
            let source_root = ncl_object::push_root(ctx, &mut source);
            let destination = *destination_root;
            assert_eq!(
                call(&runtime, ctx, &functions, "REPLACE", &[destination, source]),
                destination
            );
            assert_list(ctx, destination, &[one, two, three, four]);
            assert!(ncl_object::pop_root(ctx, source_root));
        },
    );
    assert!(ncl_object::pop_root(&mut ctx, list_symbol_root));
    for name in [
        "MAPCAR",
        "MAPC",
        "MAPLIST",
        "MAPL",
        "MAPCAN",
        "MAPCON",
        "MAP",
        "MAP-INTO",
        "REDUCE",
        "EVERY",
        "SOME",
        "NOTANY",
        "NOTEVERY",
        "REMOVE",
        "SUBSTITUTE",
        "SORT",
        "STABLE-SORT",
        "UNION",
        "INTERSECTION",
        "SET-DIFFERENCE",
        "SET-EXCLUSIVE-OR",
        "SUBSETP",
        "ADJOIN",
        "ASSOC",
        "RASSOC",
        "MEMBER",
    ] {
        probe(&runtime, &mut ctx, &functions, name);
    }
}

#[test]
fn sequence_callbacks_run_through_gc_stress_paths() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context: {error:?}"));
    ncl_lib_sequences::register(&runtime).unwrap_or_else(|error| panic!("sequences: {error:?}"));
    let names = [
        "LIST", "MAPCAR", "MAP-INTO", "REDUCE", "SORT", "UNION", "SUBSTITUTE", "CAR", "CONS",
    ];
    let functions = names
        .into_iter()
        .map(|name| {
            let word = runtime
                .function(&mut ctx, "COMMON-LISP", name)
                .unwrap_or_else(|| panic!("missing {name}"));
            (
                name.to_owned(),
                FunctionObject::try_from(word)
                    .unwrap_or_else(|_| panic!("{name} is not a function")),
            )
        })
        .collect::<HashMap<_, _>>();
    let keyword_package = runtime
        .find_package(&ctx, "KEYWORD")
        .unwrap_or_else(|| panic!("missing KEYWORD package"));
    let (test_keyword, _) = Package::from_word(keyword_package)
        .intern(&mut ctx, &runtime, "TEST")
        .unwrap_or_else(|error| panic!("intern TEST: {error:?}"));
    let values = [Word::fixnum(1), Word::fixnum(2), Word::fixnum(3)];
    let mut one = call(&runtime, &mut ctx, &functions, "LIST", &[values[0]]);
    let one_root = ncl_object::push_root(&mut ctx, &mut one);
    let mut two = call(&runtime, &mut ctx, &functions, "LIST", &[values[1]]);
    let two_root = ncl_object::push_root(&mut ctx, &mut two);
    let mut three = call(&runtime, &mut ctx, &functions, "LIST", &[values[2]]);
    let three_root = ncl_object::push_root(&mut ctx, &mut three);
    let elements = [one, two, three];
    let mut source = call(&runtime, &mut ctx, &functions, "LIST", &elements);
    let source_root = ncl_object::push_root(&mut ctx, &mut source);
    let mut destination = call(&runtime, &mut ctx, &functions, "LIST", &values);
    let destination_root = ncl_object::push_root(&mut ctx, &mut destination);
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    // These calls exercise the Lisp callback, map-into destination, reducer,
    // sort predicate, and set :test paths under both forwarding checks.
    let mut car = functions["CAR"].as_word();
    let car_root = ncl_object::push_root(&mut ctx, &mut car);
    let mut cons = functions["CONS"].as_word();
    let cons_root = ncl_object::push_root(&mut ctx, &mut cons);
    let mut test_keyword = test_keyword;
    let test_keyword_root = ncl_object::push_root(&mut ctx, &mut test_keyword);
    let mapped = call(&runtime, &mut ctx, &functions, "MAPCAR", &[car, source]);
    assert_list(&mut ctx, mapped, &values);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "MAP-INTO",
            &[destination, car, source],
        ),
        destination
    );
    assert_list(&mut ctx, destination, &values);
    let reduced = call(&runtime, &mut ctx, &functions, "REDUCE", &[cons, source]);
    assert!(reduced.is_cons());
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "SORT", &[source, cons]),
        source
    );
    let union = call(
        &runtime,
        &mut ctx,
        &functions,
        "UNION",
        &[source, destination, test_keyword, cons],
    );
    assert!(union.is_cons());
    assert_eq!(ncl_object::cdr(&mut ctx, union), Ok(Word::NIL));
    let substituted = call(&runtime, &mut ctx, &functions, "SUBSTITUTE", &[Word::fixnum(9), Word::fixnum(2), source, test_keyword, cons]);
    assert_list(&mut ctx, substituted, &[Word::fixnum(9); 3]);
    assert!(ncl_object::pop_root(&mut ctx, test_keyword_root));
    assert!(ncl_object::pop_root(&mut ctx, cons_root));
    assert!(ncl_object::pop_root(&mut ctx, car_root));
    assert!(ncl_object::pop_root(&mut ctx, destination_root));
    assert!(ncl_object::pop_root(&mut ctx, source_root));
    assert!(ncl_object::pop_root(&mut ctx, three_root)); assert!(ncl_object::pop_root(&mut ctx, two_root)); assert!(ncl_object::pop_root(&mut ctx, one_root));
}
