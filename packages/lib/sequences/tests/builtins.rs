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
    runtime
        .call_builtin(ctx, function, args)
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
}
