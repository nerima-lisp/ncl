#![allow(missing_docs, clippy::unwrap_used)]

use std::collections::HashMap;

use ncl_object::{FunctionObject, ObjectError, Runtime, ThreadContext, Word};

fn setup() -> (Runtime, ThreadContext, HashMap<String, FunctionObject>) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_sequences::register(&runtime).unwrap();
    let names = ["COUNT", "FIND", "LIST", "MISMATCH", "POSITION", "SEARCH"];
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

fn keyword(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
    let package = runtime.find_package(ctx, "KEYWORD").unwrap();
    ncl_object::Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

#[test]
fn selection_no_match_and_length_boundaries_return_documented_values() {
    let (runtime, mut ctx, functions) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let source = list(&runtime, &mut ctx, &functions, &[one, two, three]);
    let absent = Word::fixnum(9);

    assert_eq!(
        call(&runtime, &mut ctx, &functions, "FIND", &[absent, source]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "POSITION",
            &[absent, source]
        ),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "COUNT", &[absent, source]),
        Ok(Word::fixnum(0))
    );
    let shorter = list(&runtime, &mut ctx, &functions, &[one, two]);
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "SEARCH", &[source, shorter],),
        Ok(Word::NIL)
    );
    let other = list(&runtime, &mut ctx, &functions, &[three, one, two]);
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "SEARCH", &[source, other]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "MISMATCH",
            &[source, source]
        ),
        Ok(Word::NIL)
    );
    let shorter = list(&runtime, &mut ctx, &functions, &[one, two]);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "MISMATCH",
            &[source, shorter]
        ),
        Ok(Word::fixnum(2))
    );

    let start = keyword(&mut ctx, &runtime, "START");
    let end = keyword(&mut ctx, &runtime, "END");
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "FIND",
            &[one, source, start, Word::fixnum(1), end, Word::fixnum(1)],
        ),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "POSITION",
            &[one, source, start, Word::fixnum(3), end, Word::fixnum(2)],
        ),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn selection_reads_string_and_vector_elements_with_exact_indices() {
    let (runtime, mut ctx, functions) = setup();
    let text = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b', 'a']).unwrap();
    let vector = ncl_object::make_simple_vector(
        &mut ctx,
        &runtime,
        &[Word::fixnum(4), Word::fixnum(5), Word::fixnum(4)],
    )
    .unwrap();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "POSITION",
            &[Word::character(u32::from('b')), text],
        ),
        Ok(Word::fixnum(1))
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "COUNT",
            &[Word::fixnum(4), vector],
        ),
        Ok(Word::fixnum(2))
    );
    let empty = ncl_object::make_simple_vector(&mut ctx, &runtime, &[]).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "SEARCH", &[empty, empty]),
        Ok(Word::fixnum(0))
    );
}

#[test]
fn search_and_mismatch_return_exact_values_for_empty_and_test_not_cases() {
    let (runtime, mut ctx, functions) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let left = list(&runtime, &mut ctx, &functions, &[one, two]);
    let right = list(&runtime, &mut ctx, &functions, &[one, two, one]);
    let empty = ncl_object::make_simple_vector(&mut ctx, &runtime, &[]).unwrap();
    let test_not = keyword(&mut ctx, &runtime, "TEST-NOT");
    let equal = runtime.function(&mut ctx, "COMMON-LISP", "EQUAL").unwrap();
    let equal = FunctionObject::try_from(equal).unwrap().as_word();

    assert_eq!(
        call(&runtime, &mut ctx, &functions, "SEARCH", &[left, right]),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "MISMATCH", &[left, right]),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "SEARCH",
            &[empty, right, test_not, equal],
        ),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "MISMATCH",
            &[empty, right, test_not, equal],
        ),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "SEARCH",
            &[empty, empty, test_not, equal],
        ),
        Ok(Word::fixnum(0))
    );
}
