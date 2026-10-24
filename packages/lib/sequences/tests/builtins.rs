//! Integration coverage for the registered sequence builtins.

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

fn assert_list(ctx: &ThreadContext, list: Word, expected: &[Word]) {
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
    functions: &HashSet<String>,
    values: &[Word],
    f: impl FnOnce(&mut ThreadContext, ncl_object::RootSlot<'_>),
) {
    let list = call(runtime, ctx, functions, "LIST", values);
    ncl_object::with_roots(ctx, &[list], |ctx, roots| {
        f(ctx, *roots.first().ok_or(ncl_object::ObjectError::Layout)?);
        Ok(())
    })
    .unwrap_or_else(|error| panic!("rooted list: {error:?}"));
}
fn probe(runtime: &Runtime, ctx: &mut ThreadContext, functions: &HashSet<String>, name: &str) {
    assert!(functions.contains(name), "missing builtin {name}");
    let function = runtime
        .function(ctx, "COMMON-LISP", name)
        .unwrap_or_else(|| panic!("missing builtin {name}"));
    let function =
        FunctionObject::try_from(function).unwrap_or_else(|_| panic!("{name} is not a function"));
    let _ = ncl_object::with_roots(ctx, &[function.as_word(), Word::NIL], |ctx, roots| {
        let function =
            FunctionObject::try_from(**roots.first().ok_or(ncl_object::ObjectError::Layout)?)?;
        runtime.call_builtin(
            ctx,
            function,
            &[**roots.get(1).ok_or(ncl_object::ObjectError::Layout)?],
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
        "REMOVE-DUPLICATES",
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
            FunctionObject::try_from(word).unwrap_or_else(|_| panic!("{name} is not a function"));
            name.to_owned()
        })
        .collect::<HashSet<_>>();
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

    with_list(&runtime, &mut ctx, &functions, &values, |ctx, list| {
        assert_eq!(call(&runtime, ctx, &functions, "CAR", &[list.get()]), one);
        let cdr = call(&runtime, ctx, &functions, "CDR", &[list.get()]);
        assert_list(ctx, cdr, &[two, three, four, five]);
        assert_eq!(
            call(
                &runtime,
                ctx,
                &functions,
                "NTH",
                &[Word::fixnum(2), list.get()]
            ),
            three
        );
        let nthcdr = call(
            &runtime,
            ctx,
            &functions,
            "NTHCDR",
            &[Word::fixnum(3), list.get()],
        );
        assert_list(ctx, nthcdr, &[four, five]);
        assert_eq!(
            call(&runtime, ctx, &functions, "LIST-LENGTH", &[list.get()]),
            Word::fixnum(5)
        );
        assert_eq!(call(&runtime, ctx, &functions, "FIRST", &[list.get()]), one);
        assert_eq!(
            call(&runtime, ctx, &functions, "SECOND", &[list.get()]),
            two
        );
        assert_eq!(
            call(&runtime, ctx, &functions, "THIRD", &[list.get()]),
            three
        );
        assert_eq!(
            call(&runtime, ctx, &functions, "FOURTH", &[list.get()]),
            four
        );
        assert_eq!(
            call(&runtime, ctx, &functions, "FIFTH", &[list.get()]),
            five
        );
        assert_eq!(
            call(&runtime, ctx, &functions, "SIXTH", &[list.get()]),
            Word::NIL
        );
        assert_eq!(
            call(&runtime, ctx, &functions, "SEVENTH", &[list.get()]),
            Word::NIL
        );
        assert_eq!(
            call(&runtime, ctx, &functions, "EIGHTH", &[list.get()]),
            Word::NIL
        );
        assert_eq!(
            call(&runtime, ctx, &functions, "NINTH", &[list.get()]),
            Word::NIL
        );
        assert_eq!(
            call(&runtime, ctx, &functions, "TENTH", &[list.get()]),
            Word::NIL
        );
    });
    with_list(&runtime, &mut ctx, &functions, &[one], |ctx, list| {
        let mut result = call(
            &runtime,
            ctx,
            &functions,
            "CONS",
            &[Word::fixnum(0), list.get()],
        );
        let root = ncl_object::push_root(ctx, &mut result);
        assert_eq!(
            call(&runtime, ctx, &functions, "CAR", &[result]),
            Word::fixnum(0)
        );
        assert_eq!(
            ncl_object::cdr(ctx, result).and_then(|cdr| ncl_object::car(ctx, cdr)),
            Ok(one)
        );
        assert!(ncl_object::pop_root(ctx, root));
    });
    with_list(&runtime, &mut ctx, &functions, &[one], |ctx, list| {
        assert_eq!(
            call(&runtime, ctx, &functions, "RPLACA", &[list.get(), two]),
            list.get()
        );
        assert_eq!(call(&runtime, ctx, &functions, "CAR", &[list.get()]), two);
        assert_eq!(
            call(
                &runtime,
                ctx,
                &functions,
                "RPLACD",
                &[list.get(), Word::NIL]
            ),
            list.get()
        );
        assert_eq!(ncl_object::cdr(ctx, list.get()), Ok(Word::NIL));
    });
    with_list(&runtime, &mut ctx, &functions, &values, |ctx, list| {
        let mut result = call(&runtime, ctx, &functions, "COPY-LIST", &[list.get()]);
        let root = ncl_object::push_root(ctx, &mut result);
        assert_list(ctx, result, &values);
        assert_ne!(result, list.get());
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
    assert_list(&ctx, list_star, &[one, two, three]);
    assert!(ncl_object::pop_root(&mut ctx, list_star_root));
    with_list(
        &runtime,
        &mut ctx,
        &functions,
        &[one, two],
        |ctx, left_root| {
            let mut right = call(&runtime, ctx, &functions, "LIST", &[three]);
            let right_root = ncl_object::push_root(ctx, &mut right);
            let left = left_root.get();
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
            let left = left_root.get();
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
    with_list(&runtime, &mut ctx, &functions, &values, |ctx, list| {
        assert_eq!(
            call(&runtime, ctx, &functions, "LENGTH", &[list.get()]),
            Word::fixnum(5)
        );
        let mut result = call(&runtime, ctx, &functions, "COPY-SEQ", &[list.get()]);
        let root = ncl_object::push_root(ctx, &mut result);
        assert_list(ctx, result, &values);
        assert!(ncl_object::pop_root(ctx, root));
    });

    with_list(&runtime, &mut ctx, &functions, &values, |ctx, list| {
        let mut result = call(&runtime, ctx, &functions, "REVERSE", &[list.get()]);
        let root = ncl_object::push_root(ctx, &mut result);
        assert_list(ctx, result, &[five, four, three, two, one]);
        assert!(ncl_object::pop_root(ctx, root));
    });

    with_list(&runtime, &mut ctx, &functions, &values, |ctx, list| {
        let mut result = call(&runtime, ctx, &functions, "NREVERSE", &[list.get()]);
        let root = ncl_object::push_root(ctx, &mut result);
        assert_list(ctx, result, &[five, four, three, two, one]);
        assert!(ncl_object::pop_root(ctx, root));
    });

    with_list(&runtime, &mut ctx, &functions, &values, |ctx, list| {
        assert_eq!(
            call(
                &runtime,
                ctx,
                &functions,
                "ELT",
                &[list.get(), Word::fixnum(3)]
            ),
            four
        );
        let mut result = call(
            &runtime,
            ctx,
            &functions,
            "SUBSEQ",
            &[list.get(), Word::fixnum(1), Word::fixnum(4)],
        );
        let root = ncl_object::push_root(ctx, &mut result);
        assert_list(ctx, result, &[two, three, four]);
        assert!(ncl_object::pop_root(ctx, root));
    });

    with_list(&runtime, &mut ctx, &functions, &values, |ctx, list| {
        assert_eq!(
            call(
                &runtime,
                ctx,
                &functions,
                "FILL",
                &[list.get(), Word::fixnum(9)]
            ),
            list.get()
        );
        assert_list(ctx, list.get(), &[Word::fixnum(9); 5]);
    });

    with_list(
        &runtime,
        &mut ctx,
        &functions,
        &[one, two, three, four],
        |ctx, destination_root| {
            let mut source = call(&runtime, ctx, &functions, "LIST", &values);
            let source_root = ncl_object::push_root(ctx, &mut source);
            let destination = destination_root.get();
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
fn remove_duplicates_keeps_the_last_occurrence() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context: {error:?}"));
    ncl_lib_sequences::register(&runtime).unwrap_or_else(|error| panic!("sequences: {error:?}"));
    let functions = ["LIST", "REMOVE-DUPLICATES"]
        .into_iter()
        .map(|name| {
            let word = runtime
                .function(&mut ctx, "COMMON-LISP", name)
                .unwrap_or_else(|| panic!("missing builtin {name}"));
            FunctionObject::try_from(word).unwrap_or_else(|_| panic!("{name} is not a function"));
            name.to_owned()
        })
        .collect::<HashSet<_>>();
    let input = call(
        &runtime,
        &mut ctx,
        &functions,
        "LIST",
        &[Word::fixnum(1), Word::fixnum(2), Word::fixnum(1)],
    );
    let result = ncl_object::with_roots(&mut ctx, &[input], |ctx, roots| {
        let input = roots.first().ok_or(ncl_object::ObjectError::Layout)?.get();
        Ok(call(
            &runtime,
            ctx,
            &functions,
            "REMOVE-DUPLICATES",
            &[input],
        ))
    })
    .unwrap_or_else(|error| panic!("remove duplicates: {error:?}"));
    assert_list(&ctx, result, &[Word::fixnum(2), Word::fixnum(1)]);
}
