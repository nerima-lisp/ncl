//! GC-stress coverage for sequence callbacks and set operations.

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

#[test]
fn sequence_callbacks_run_through_gc_stress_paths() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context: {error:?}"));
    ncl_lib_sequences::register(&runtime).unwrap_or_else(|error| panic!("sequences: {error:?}"));
    let names = [
        "LIST",
        "MAPCAR",
        "MAP-INTO",
        "REDUCE",
        "SORT",
        "UNION",
        "SUBSTITUTE",
        "CAR",
        "CONS",
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
    let substituted = call(
        &runtime,
        &mut ctx,
        &functions,
        "SUBSTITUTE",
        &[Word::fixnum(9), Word::fixnum(2), source, test_keyword, cons],
    );
    assert_list(&mut ctx, substituted, &[Word::fixnum(9); 3]);
    assert!(ncl_object::pop_root(&mut ctx, test_keyword_root));
    assert!(ncl_object::pop_root(&mut ctx, cons_root));
    assert!(ncl_object::pop_root(&mut ctx, car_root));
    assert!(ncl_object::pop_root(&mut ctx, destination_root));
    assert!(ncl_object::pop_root(&mut ctx, source_root));
    assert!(ncl_object::pop_root(&mut ctx, three_root));
    assert!(ncl_object::pop_root(&mut ctx, two_root));
    assert!(ncl_object::pop_root(&mut ctx, one_root));
}
