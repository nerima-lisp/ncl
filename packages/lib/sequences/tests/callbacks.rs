//! GC-stress coverage for sequence callbacks and set operations.

use std::collections::HashSet;

use ncl_object::{FunctionObject, Package, Runtime, ThreadContext, Word};

fn call(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    functions: &HashSet<String>,
    name: &str,
    args: &[Word],
) -> Word {
    assert!(functions.contains(name), "missing builtin {name}");
    ncl_object::with_roots(ctx, args, |ctx, arg_roots| {
        let function = runtime
            .function(ctx, "COMMON-LISP", name)
            .ok_or(ncl_object::ObjectError::Unbound)?;
        let function = FunctionObject::try_from(function)?;
        let args = arg_roots.iter().map(|root| **root).collect::<Vec<_>>();
        let mut rooted_values = Vec::with_capacity(args.len() + 1);
        rooted_values.push(function.as_word());
        rooted_values.extend_from_slice(&args);
        ncl_object::with_roots(ctx, &rooted_values, |ctx, roots| {
            let function =
                FunctionObject::try_from(**roots.first().ok_or(ncl_object::ObjectError::Layout)?)?;
            let rooted_args = roots
                .get(1..)
                .ok_or(ncl_object::ObjectError::Layout)?
                .iter()
                .map(|root| **root)
                .collect::<Vec<_>>();
            runtime.call_builtin(ctx, function, &rooted_args)
        })
    })
    .unwrap_or_else(|error| panic!("{name} failed: {error:?}"))
}

fn function_word(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> Word {
    let word = runtime
        .function(ctx, "COMMON-LISP", name)
        .unwrap_or_else(|| panic!("missing {name}"));
    FunctionObject::try_from(word)
        .unwrap_or_else(|_| panic!("{name} is not a function"))
        .as_word()
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
        "MAP",
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
            FunctionObject::try_from(word).unwrap_or_else(|_| panic!("{name} is not a function"));
            name.to_owned()
        })
        .collect::<HashSet<_>>();
    let keyword_package = runtime
        .find_package(&ctx, "KEYWORD")
        .unwrap_or_else(|| panic!("missing KEYWORD package"));
    let (mut test_keyword, _) = Package::from_word(keyword_package)
        .intern(&mut ctx, &runtime, "TEST")
        .unwrap_or_else(|error| panic!("intern TEST: {error:?}"));
    let test_keyword_root = ncl_object::push_root(&mut ctx, &mut test_keyword);
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
    let mut car = function_word(&runtime, &mut ctx, "CAR");
    let car_root = ncl_object::push_root(&mut ctx, &mut car);
    let mut cons = function_word(&runtime, &mut ctx, "CONS");
    let cons_root = ncl_object::push_root(&mut ctx, &mut cons);
    let mapped = call(&runtime, &mut ctx, &functions, "MAPCAR", &[car, source]);
    assert_list(&ctx, mapped, &values);
    assert_map_without_result_type(&runtime, &mut ctx, &functions, car, source);
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
    assert_list(&ctx, destination, &values);
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
    assert_eq!(ncl_object::cdr(&ctx, union), Ok(Word::NIL));
    let substituted = call(
        &runtime,
        &mut ctx,
        &functions,
        "SUBSTITUTE",
        &[Word::fixnum(9), Word::fixnum(2), source, test_keyword, cons],
    );
    assert_list(&ctx, substituted, &[Word::fixnum(9); 3]);
    assert!(ncl_object::pop_root(&mut ctx, cons_root));
    assert!(ncl_object::pop_root(&mut ctx, car_root));
    assert!(ncl_object::pop_root(&mut ctx, destination_root));
    assert!(ncl_object::pop_root(&mut ctx, source_root));
    assert!(ncl_object::pop_root(&mut ctx, three_root));
    assert!(ncl_object::pop_root(&mut ctx, two_root));
    assert!(ncl_object::pop_root(&mut ctx, one_root));
    assert!(ncl_object::pop_root(&mut ctx, test_keyword_root));
}
