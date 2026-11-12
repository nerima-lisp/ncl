//! GC-stress coverage for sequence callbacks and set operations.

use std::collections::HashSet;

use ncl_object::{FunctionObject, Package, RootSlot, Runtime, ThreadContext, Word};

fn call(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    functions: &HashSet<String>,
    name: &str,
    args: &[Word],
) -> Word {
    assert!(functions.contains(name), "missing builtin {name}");
    let function = runtime
        .function(ctx, "COMMON-LISP", name)
        .unwrap_or_else(|| panic!("missing builtin {name}"));
    let function =
        FunctionObject::try_from(function).unwrap_or_else(|_| panic!("{name} is not a function"));
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        let rooted_args = roots.iter().map(|root| root.get()).collect::<Vec<_>>();
        runtime.call_builtin(ctx, function, &rooted_args)
    })
    .unwrap_or_else(|error| panic!("{name} failed: {error:?}"))
}

fn assert_list(ctx: &ThreadContext, list: Word, expected: &[Word]) {
    let mut cursor = list;
    for &value in expected {
        assert!(cursor.is_cons());
        assert_eq!(ncl_object::car(ctx, cursor), Ok(value));
        cursor = ncl_object::cdr(ctx, cursor).unwrap_or_else(|error| panic!("cdr: {error:?}"));
    }
    assert_eq!(cursor, Word::NIL);
}

fn assert_map_without_result_type(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    functions: &HashSet<String>,
    function: Word,
    source: Word,
) {
    assert_eq!(
        call(
            runtime,
            ctx,
            functions,
            "MAP",
            &[Word::NIL, function, source]
        ),
        Word::NIL
    );
}

fn setup() -> (Runtime, ThreadContext, HashSet<String>) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context: {error:?}"));
    ncl_lib_sequences::register(&runtime).unwrap_or_else(|error| panic!("sequences: {error:?}"));
    let names = [
        "LIST",
        "MAPCAR",
        "MAP",
        "MAP-INTO",
        "REDUCE",
        "SORT",
        "UNION",
        "EVERY",
        "INTERSECTION",
        "FIND",
        "SUBSTITUTE",
        "CAR",
        "CONS",
    ];
    (runtime, ctx, names.into_iter().map(str::to_owned).collect())
}

fn intern_keyword(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> Word {
    let package = runtime
        .find_package(ctx, "KEYWORD")
        .unwrap_or_else(|| panic!("missing KEYWORD package"));
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap_or_else(|error| panic!("intern {name}: {error:?}"))
        .0
}

fn rooted_word(roots: &[RootSlot<'_>], index: usize) -> Word {
    roots
        .get(index)
        .unwrap_or_else(|| panic!("missing rooted callback test value {index}"))
        .get()
}

fn mapping_cases(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    functions: &HashSet<String>,
    roots: &[RootSlot<'_>],
) {
    let values = [Word::fixnum(1), Word::fixnum(2), Word::fixnum(3)];
    let mapped = call(
        runtime,
        ctx,
        functions,
        "MAPCAR",
        &[rooted_word(roots, 5), rooted_word(roots, 3)],
    );
    assert_list(ctx, mapped, &values);
    assert_map_without_result_type(
        runtime,
        ctx,
        functions,
        rooted_word(roots, 5),
        rooted_word(roots, 3),
    );
    assert_eq!(
        call(
            runtime,
            ctx,
            functions,
            "MAP-INTO",
            &[
                rooted_word(roots, 4),
                rooted_word(roots, 5),
                rooted_word(roots, 3),
            ],
        ),
        rooted_word(roots, 4)
    );
    assert_list(ctx, rooted_word(roots, 4), &values);
    assert_eq!(
        call(
            runtime,
            ctx,
            functions,
            "MAP",
            &[Word::NIL, rooted_word(roots, 7), rooted_word(roots, 3)],
        ),
        Word::NIL
    );
}

fn higher_order_cases(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    functions: &HashSet<String>,
    roots: &[RootSlot<'_>],
) {
    assert_eq!(
        call(
            runtime,
            ctx,
            functions,
            "EVERY",
            &[rooted_word(roots, 7), rooted_word(roots, 3)],
        ),
        Word::TRUE
    );
    let reduced = call(
        runtime,
        ctx,
        functions,
        "REDUCE",
        &[rooted_word(roots, 6), rooted_word(roots, 3)],
    );
    assert!(reduced.is_cons());
}

fn selection_cases(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    functions: &HashSet<String>,
    roots: &[RootSlot<'_>],
) {
    assert_eq!(
        call(
            runtime,
            ctx,
            functions,
            "FIND",
            &[
                rooted_word(roots, 0),
                rooted_word(roots, 3),
                rooted_word(roots, 9),
                rooted_word(roots, 7),
                rooted_word(roots, 8),
                rooted_word(roots, 6),
            ],
        ),
        rooted_word(roots, 0)
    );
    let substituted = call(
        runtime,
        ctx,
        functions,
        "SUBSTITUTE",
        &[
            Word::fixnum(9),
            Word::fixnum(2),
            rooted_word(roots, 3),
            rooted_word(roots, 8),
            rooted_word(roots, 6),
        ],
    );
    assert_list(ctx, substituted, &[Word::fixnum(9); 3]);
}

fn set_operation_cases(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    functions: &HashSet<String>,
    roots: &[RootSlot<'_>],
) {
    assert_eq!(
        call(
            runtime,
            ctx,
            functions,
            "INTERSECTION",
            &[
                rooted_word(roots, 3),
                rooted_word(roots, 4),
                rooted_word(roots, 9),
                rooted_word(roots, 7)
            ],
        ),
        Word::NIL
    );
    assert_eq!(
        call(
            runtime,
            ctx,
            functions,
            "SORT",
            &[
                rooted_word(roots, 3),
                rooted_word(roots, 6),
                rooted_word(roots, 9),
                rooted_word(roots, 7),
            ],
        ),
        rooted_word(roots, 3)
    );
    let union = call(
        runtime,
        ctx,
        functions,
        "UNION",
        &[
            rooted_word(roots, 3),
            rooted_word(roots, 4),
            rooted_word(roots, 8),
            rooted_word(roots, 6),
        ],
    );
    assert!(union.is_cons());
    assert_eq!(ncl_object::cdr(ctx, union), Ok(Word::NIL));
}

#[test]
fn sequence_callbacks_run_through_gc_stress_paths() {
    let (runtime, mut ctx, functions) = setup();
    let test_keyword = intern_keyword(&runtime, &mut ctx, "TEST");
    let key_keyword = intern_keyword(&runtime, &mut ctx, "KEY");
    let values = [Word::fixnum(1), Word::fixnum(2), Word::fixnum(3)];
    let mut one = call(&runtime, &mut ctx, &functions, "LIST", &[values[0]]);
    let one_root = ncl_object::push_root(&mut ctx, &mut one);
    let mut two = call(&runtime, &mut ctx, &functions, "LIST", &[values[1]]);
    let two_root = ncl_object::push_root(&mut ctx, &mut two);
    let mut three = call(&runtime, &mut ctx, &functions, "LIST", &[values[2]]);
    let three_root = ncl_object::push_root(&mut ctx, &mut three);
    let mut source = call(&runtime, &mut ctx, &functions, "LIST", &[one, two, three]);
    let source_root = ncl_object::push_root(&mut ctx, &mut source);
    let mut destination = call(&runtime, &mut ctx, &functions, "LIST", &values);
    let destination_root = ncl_object::push_root(&mut ctx, &mut destination);
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    let mut car = runtime
        .function(&mut ctx, "COMMON-LISP", "CAR")
        .unwrap_or_else(|| panic!("CAR should be registered"));
    let car_root = ncl_object::push_root(&mut ctx, &mut car);
    let mut cons = runtime
        .function(&mut ctx, "COMMON-LISP", "CONS")
        .unwrap_or_else(|| panic!("CONS should be registered"));
    let cons_root = ncl_object::push_root(&mut ctx, &mut cons);
    let mut list = runtime
        .function(&mut ctx, "COMMON-LISP", "LIST")
        .unwrap_or_else(|| panic!("LIST should be registered"));
    let list_root = ncl_object::push_root(&mut ctx, &mut list);
    let mut test_keyword = test_keyword;
    let test_keyword_root = ncl_object::push_root(&mut ctx, &mut test_keyword);
    let mut key_keyword = key_keyword;
    let key_keyword_root = ncl_object::push_root(&mut ctx, &mut key_keyword);
    let root_values = [
        one,
        two,
        three,
        source,
        destination,
        car,
        cons,
        list,
        test_keyword,
        key_keyword,
    ];
    ncl_object::with_roots(&mut ctx, &root_values, |ctx, roots| {
        mapping_cases(&runtime, ctx, &functions, roots);
        higher_order_cases(&runtime, ctx, &functions, roots);
        selection_cases(&runtime, ctx, &functions, roots);
        set_operation_cases(&runtime, ctx, &functions, roots);
        Ok(())
    })
    .unwrap_or_else(|error| panic!("callback roots: {error:?}"));
    assert!(ncl_object::pop_root(&mut ctx, key_keyword_root));
    assert!(ncl_object::pop_root(&mut ctx, test_keyword_root));
    assert!(ncl_object::pop_root(&mut ctx, list_root));
    assert!(ncl_object::pop_root(&mut ctx, cons_root));
    assert!(ncl_object::pop_root(&mut ctx, car_root));
    assert!(ncl_object::pop_root(&mut ctx, destination_root));
    assert!(ncl_object::pop_root(&mut ctx, source_root));
    assert!(ncl_object::pop_root(&mut ctx, three_root));
    assert!(ncl_object::pop_root(&mut ctx, two_root));
    assert!(ncl_object::pop_root(&mut ctx, one_root));
}
