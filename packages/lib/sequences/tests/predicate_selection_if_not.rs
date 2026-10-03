#![allow(missing_docs, clippy::unwrap_used)]

use std::collections::HashMap;

use ncl_object::{FunctionObject, ObjectError, Package, Runtime, ThreadContext, Word};

fn setup() -> (Runtime, ThreadContext, HashMap<String, FunctionObject>) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_sequences::register(&runtime).unwrap();
    let names = [
        "LIST",
        "CONSP",
        "FIND-IF-NOT",
        "POSITION-IF-NOT",
        "COUNT-IF-NOT",
        "REMOVE-IF-NOT",
        "SUBSTITUTE-IF-NOT",
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
fn if_not_variants_return_selected_values_and_respect_direction() {
    let (runtime, mut ctx, functions) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let four = Word::fixnum(4);
    let first_cons = list(&runtime, &mut ctx, &functions, &[two]);
    let second_cons = list(&runtime, &mut ctx, &functions, &[four]);
    let sequence = list(
        &runtime,
        &mut ctx,
        &functions,
        &[one, first_cons, three, second_cons],
    );
    let predicate = functions["CONSP"].as_word();
    let args = [predicate, sequence];
    let from_end = keyword(&mut ctx, &runtime, "FROM-END");

    assert_eq!(
        call(&runtime, &mut ctx, &functions, "FIND-IF-NOT", &args),
        Ok(one)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "FIND-IF-NOT",
            &[predicate, sequence, from_end, Word::TRUE],
        ),
        Ok(three)
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "POSITION-IF-NOT", &args),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "POSITION-IF-NOT",
            &[predicate, sequence, from_end, Word::TRUE],
        ),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "COUNT-IF-NOT", &args),
        Ok(Word::fixnum(2))
    );

    let removed = call(&runtime, &mut ctx, &functions, "REMOVE-IF-NOT", &args).unwrap();
    assert_eq!(list_values(&ctx, removed), vec![first_cons, second_cons]);
    let substituted = call(
        &runtime,
        &mut ctx,
        &functions,
        "SUBSTITUTE-IF-NOT",
        &[Word::fixnum(9), predicate, sequence],
    )
    .unwrap();
    assert_eq!(
        list_values(&ctx, substituted),
        vec![Word::fixnum(9), first_cons, Word::fixnum(9), second_cons]
    );
}

#[test]
fn if_not_variants_reject_invalid_ranges_and_conflicting_tests() {
    let (runtime, mut ctx, functions) = setup();
    let sequence = list(
        &runtime,
        &mut ctx,
        &functions,
        &[Word::fixnum(1), Word::fixnum(2)],
    );
    let predicate = functions["CONSP"].as_word();
    let start = keyword(&mut ctx, &runtime, "START");
    let end = keyword(&mut ctx, &runtime, "END");
    let test = keyword(&mut ctx, &runtime, "TEST");
    let test_not = keyword(&mut ctx, &runtime, "TEST-NOT");

    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "FIND-IF-NOT",
            &[predicate, sequence, start, Word::TRUE],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "POSITION-IF-NOT",
            &[predicate, sequence, end, Word::fixnum(3)],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "COUNT-IF-NOT",
            &[
                predicate,
                sequence,
                test,
                functions["CONSP"].as_word(),
                test_not,
                functions["CONSP"].as_word(),
            ],
        ),
        Err(ObjectError::TypeError)
    );
}
