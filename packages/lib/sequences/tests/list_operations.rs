//! Tests for registered list-operation builtins.

use std::collections::HashMap;

use ncl_object::{FunctionObject, ObjectError, ObjectRef, Runtime, ThreadContext, Word};

fn setup() -> (Runtime, ThreadContext, HashMap<String, FunctionObject>) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context: {error:?}"));
    ncl_lib_sequences::register(&runtime).unwrap_or_else(|error| panic!("sequences: {error:?}"));
    let functions = ["APPEND", "CONS", "LIST", "LISTP", "NCONC", "NREVERSE"]
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
        .collect();
    (runtime, ctx, functions)
}

fn call_result(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    functions: &HashMap<String, FunctionObject>,
    name: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let function = *functions
        .get(name)
        .unwrap_or_else(|| panic!("missing builtin {name}"));
    // Builtins can allocate, so keep their arguments rooted for the call.
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        let rooted_args = roots.iter().map(|root| **root).collect::<Vec<_>>();
        runtime.call_builtin(ctx, function, &rooted_args)
    })
}

fn call(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    functions: &HashMap<String, FunctionObject>,
    name: &str,
    args: &[Word],
) -> Word {
    call_result(runtime, ctx, functions, name, args)
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

#[test]
fn listp_accepts_a_dotted_cons() {
    let (runtime, mut ctx, functions) = setup();
    let dotted = call(
        &runtime,
        &mut ctx,
        &functions,
        "CONS",
        &[Word::fixnum(1), Word::fixnum(2)],
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "LISTP", &[dotted]),
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "LISTP", &[Word::fixnum(1)],),
        Word::NIL
    );
}

#[test]
fn nreverse_rejects_a_dotted_sequence() {
    let (runtime, mut ctx, functions) = setup();
    let dotted = call(
        &runtime,
        &mut ctx,
        &functions,
        "CONS",
        &[Word::fixnum(1), Word::fixnum(2)],
    );
    assert!(matches!(
        call_result(&runtime, &mut ctx, &functions, "NREVERSE", &[dotted]),
        Err(ObjectError::TypeError)
    ));
}

#[test]
fn nreverse_reverses_simple_vectors() {
    let (runtime, mut ctx, functions) = setup();
    let vector = ncl_object::make_simple_vector(
        &mut ctx,
        &runtime,
        &[Word::fixnum(1), Word::fixnum(2), Word::fixnum(3)],
    )
    .unwrap_or_else(|error| panic!("vector: {error:?}"));

    let reversed = call(&runtime, &mut ctx, &functions, "NREVERSE", &[vector]);

    assert!(matches!(
        ncl_object::classify_object(&ctx, reversed),
        ObjectRef::SimpleVector(_)
    ));
    assert_eq!(ncl_object::simple_vector_length(&ctx, reversed), Ok(3));
    assert_eq!(
        (0..3)
            .map(|index| ncl_object::simple_vector_ref(&ctx, reversed, index))
            .collect::<Result<Vec<_>, _>>(),
        Ok(vec![Word::fixnum(3), Word::fixnum(2), Word::fixnum(1)])
    );
}

#[test]
fn nreverse_reverses_strings() {
    let (runtime, mut ctx, functions) = setup();
    let string = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b', 'c'])
        .unwrap_or_else(|error| panic!("string: {error:?}"));

    let reversed = call(&runtime, &mut ctx, &functions, "NREVERSE", &[string]);

    assert!(matches!(
        ncl_object::classify_object(&ctx, reversed),
        ObjectRef::String(_)
    ));
    assert_eq!(ncl_object::string_length(&ctx, reversed), Ok(3));
    assert_eq!(
        (0..3)
            .map(|index| ncl_object::string_ref(&ctx, reversed, index))
            .collect::<Result<Vec<_>, _>>(),
        Ok(vec!['c', 'b', 'a'])
    );
}

#[test]
fn append_returns_an_atomic_last_argument_and_rejects_dotted_prefixes() {
    let (runtime, mut ctx, functions) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let tail = Word::fixnum(9);

    assert_eq!(
        call(&runtime, &mut ctx, &functions, "APPEND", &[]),
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "APPEND", &[tail]),
        tail
    );

    let first = call(&runtime, &mut ctx, &functions, "LIST", &[one, two]);
    let mut first = first;
    let first_root = ncl_object::push_root(&mut ctx, &mut first);
    let second = call(
        &runtime,
        &mut ctx,
        &functions,
        "LIST",
        &[three, Word::fixnum(4)],
    );
    let mut second = second;
    let second_root = ncl_object::push_root(&mut ctx, &mut second);
    let result = call(
        &runtime,
        &mut ctx,
        &functions,
        "APPEND",
        &[first, second, tail],
    );
    assert_eq!(ncl_object::car(&ctx, result), Ok(one));
    let result_second =
        ncl_object::cdr(&ctx, result).unwrap_or_else(|error| panic!("cdr: {error:?}"));
    assert_eq!(ncl_object::car(&ctx, result_second), Ok(two));
    let result_third =
        ncl_object::cdr(&ctx, result_second).unwrap_or_else(|error| panic!("cdr: {error:?}"));
    assert_eq!(ncl_object::car(&ctx, result_third), Ok(three));
    let result_fourth =
        ncl_object::cdr(&ctx, result_third).unwrap_or_else(|error| panic!("cdr: {error:?}"));
    assert_eq!(ncl_object::car(&ctx, result_fourth), Ok(Word::fixnum(4)));
    assert_eq!(ncl_object::cdr(&ctx, result_fourth), Ok(tail));
    assert_list(&ctx, first, &[one, two]);
    assert_list(&ctx, second, &[three, Word::fixnum(4)]);

    let dotted = call(&runtime, &mut ctx, &functions, "CONS", &[one, tail]);
    assert!(matches!(
        call_result(
            &runtime,
            &mut ctx,
            &functions,
            "APPEND",
            &[dotted, Word::NIL]
        ),
        Err(ObjectError::TypeError)
    ));
    assert!(ncl_object::pop_root(&mut ctx, second_root));
    assert!(ncl_object::pop_root(&mut ctx, first_root));
}

#[test]
fn nconc_links_multi_cons_dotted_prefixes_and_skips_nil() {
    let (runtime, mut ctx, functions) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let four = Word::fixnum(4);
    let five = Word::fixnum(5);
    let six = Word::fixnum(6);

    assert_eq!(
        call(&runtime, &mut ctx, &functions, "NCONC", &[]),
        Word::NIL
    );
    assert_eq!(call(&runtime, &mut ctx, &functions, "NCONC", &[two]), two);

    let first_dotted_last_cons = call(
        &runtime,
        &mut ctx,
        &functions,
        "CONS",
        &[two, Word::fixnum(90)],
    );
    let first_dotted_prefix = call(
        &runtime,
        &mut ctx,
        &functions,
        "CONS",
        &[one, first_dotted_last_cons],
    );
    let mut first_dotted_prefix = first_dotted_prefix;
    let first_root = ncl_object::push_root(&mut ctx, &mut first_dotted_prefix);
    let first_dotted_last_cons =
        ncl_object::cdr(&ctx, first_dotted_prefix).unwrap_or_else(|error| panic!("cdr: {error:?}"));
    assert_eq!(
        ncl_object::cdr(&ctx, first_dotted_last_cons),
        Ok(Word::fixnum(90))
    );

    let second_dotted_last_cons = call(
        &runtime,
        &mut ctx,
        &functions,
        "CONS",
        &[four, Word::fixnum(91)],
    );
    let second_dotted_prefix = call(
        &runtime,
        &mut ctx,
        &functions,
        "CONS",
        &[three, second_dotted_last_cons],
    );
    let mut second_dotted_prefix = second_dotted_prefix;
    let second_root = ncl_object::push_root(&mut ctx, &mut second_dotted_prefix);
    let second_dotted_last_cons = ncl_object::cdr(&ctx, second_dotted_prefix)
        .unwrap_or_else(|error| panic!("cdr: {error:?}"));
    assert_eq!(
        ncl_object::cdr(&ctx, second_dotted_last_cons),
        Ok(Word::fixnum(91))
    );

    let tail = call(&runtime, &mut ctx, &functions, "LIST", &[five, six]);
    let mut tail = tail;
    let tail_root = ncl_object::push_root(&mut ctx, &mut tail);
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "NCONC", &[Word::NIL, tail]),
        tail
    );

    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "NCONC",
            &[first_dotted_prefix, Word::NIL, second_dotted_prefix, tail]
        ),
        first_dotted_prefix
    );
    let first_last_cons =
        ncl_object::cdr(&ctx, first_dotted_prefix).unwrap_or_else(|error| panic!("cdr: {error:?}"));
    let first_link =
        ncl_object::cdr(&ctx, first_last_cons).unwrap_or_else(|error| panic!("cdr: {error:?}"));
    assert_eq!(first_link, second_dotted_prefix);
    let second_last_cons = ncl_object::cdr(&ctx, second_dotted_prefix)
        .unwrap_or_else(|error| panic!("cdr: {error:?}"));
    let second_link =
        ncl_object::cdr(&ctx, second_last_cons).unwrap_or_else(|error| panic!("cdr: {error:?}"));
    assert_eq!(second_link, tail);
    assert_list(
        &ctx,
        first_dotted_prefix,
        &[one, two, three, four, five, six],
    );

    assert!(matches!(
        call_result(&runtime, &mut ctx, &functions, "NCONC", &[two, tail]),
        Err(ObjectError::TypeError)
    ));
    assert!(ncl_object::pop_root(&mut ctx, tail_root));
    assert!(ncl_object::pop_root(&mut ctx, second_root));
    assert!(ncl_object::pop_root(&mut ctx, first_root));
}
