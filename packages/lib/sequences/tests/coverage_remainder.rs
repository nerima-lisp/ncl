#![allow(clippy::unwrap_used, missing_docs)]

use ncl_object::{FunctionObject, ObjectError, Package, Runtime, ThreadContext, Word};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_sequences::register(&runtime).unwrap();
    (runtime, ctx)
}

fn call(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let function = runtime.function(ctx, "COMMON-LISP", name).unwrap();
    let function = FunctionObject::try_from(function).unwrap();
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        runtime.call_builtin(
            ctx,
            function,
            &roots.iter().map(|root| **root).collect::<Vec<_>>(),
        )
    })
}

fn list(runtime: &Runtime, ctx: &mut ThreadContext, values: &[Word]) -> Word {
    call(runtime, ctx, "LIST", values).unwrap()
}

fn values(ctx: &ThreadContext, mut value: Word) -> Vec<Word> {
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

fn function(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
    FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap())
        .unwrap()
        .as_word()
}

#[test]
fn order_sets_public_api_exercises_key_test_and_sort_edges() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let test = keyword(&mut ctx, &runtime, "TEST");
    let test_not = keyword(&mut ctx, &runtime, "TEST-NOT");
    let equal = function(&mut ctx, &runtime, "EQUAL");
    let car = function(&mut ctx, &runtime, "CAR");
    let key = keyword(&mut ctx, &runtime, "KEY");
    let left_one = list(&runtime, &mut ctx, &[one]);
    let left_two = list(&runtime, &mut ctx, &[two]);
    let right_two = list(&runtime, &mut ctx, &[two]);
    let right_three = list(&runtime, &mut ctx, &[three]);
    let left = list(&runtime, &mut ctx, &[left_one, left_two, left_one]);
    let right = list(&runtime, &mut ctx, &[right_two, right_three]);

    let union = call(
        &runtime,
        &mut ctx,
        "UNION",
        &[left, right, key, car, test, equal],
    )
    .unwrap();
    assert_eq!(values(&ctx, union).len(), 3);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "INTERSECTION",
            &[left, right, test, equal, test_not, equal],
        ),
        Err(ObjectError::TypeError)
    );

    let flat = list(&runtime, &mut ctx, &[one, one, two]);
    let sorted = call(&runtime, &mut ctx, "SORT", &[flat, equal]).unwrap();
    assert_eq!(values(&ctx, sorted), vec![one, one, two]);
    assert_eq!(
        call(&runtime, &mut ctx, "UNION", &[left, right, key, Word::TRUE]),
        Err(ObjectError::TypeError)
    );
}
