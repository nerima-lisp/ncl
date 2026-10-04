#![allow(clippy::unwrap_used, clippy::unreadable_literal)]

use ncl_object::{FunctionObject, Runtime, ThreadContext, Word};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    super::register(&runtime).unwrap();
    (runtime, ctx)
}

fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> Word {
    let function =
        FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap();
    runtime.call_builtin(ctx, function, args).unwrap()
}

fn list(runtime: &Runtime, ctx: &mut ThreadContext, values: &[Word]) -> Word {
    call(runtime, ctx, "LIST", values)
}

fn list_values(ctx: &ThreadContext, mut value: Word) -> Vec<Word> {
    let mut result = Vec::new();
    while value != Word::NIL {
        result.push(ncl_object::car(ctx, value).unwrap());
        value = ncl_object::cdr(ctx, value).unwrap();
    }
    result
}

#[test]
fn sequence_ranges_and_keys_return_exact_values() {
    let (runtime, mut ctx) = setup();
    let values = [
        Word::fixnum(4),
        Word::fixnum(1),
        Word::fixnum(3),
        Word::fixnum(2),
    ];
    let source = list(&runtime, &mut ctx, &values);
    let start = Word::fixnum(1);
    let end = Word::fixnum(3);
    assert_eq!(
        call(&runtime, &mut ctx, "LENGTH", &[source]),
        Word::fixnum(4)
    );
    assert_eq!(call(&runtime, &mut ctx, "ELT", &[source, start]), values[1]);
    let slice = call(&runtime, &mut ctx, "SUBSEQ", &[source, start, end]);
    assert_eq!(list_values(&ctx, slice), values[1..3]);
    let copied = call(&runtime, &mut ctx, "COPY-SEQ", &[source]);
    assert_eq!(list_values(&ctx, copied), values);
    let reversed = call(&runtime, &mut ctx, "REVERSE", &[source]);
    assert_eq!(
        list_values(&ctx, reversed),
        [values[3], values[2], values[1], values[0]]
    );
}

#[test]
fn selection_and_set_operations_cover_empty_and_duplicate_cases() {
    let (runtime, mut ctx) = setup();
    let one = list(&runtime, &mut ctx, &[Word::fixnum(1)]);
    let left = list(
        &runtime,
        &mut ctx,
        &[Word::fixnum(1), Word::fixnum(2), Word::fixnum(2)],
    );
    let right = list(&runtime, &mut ctx, &[Word::fixnum(2), Word::fixnum(3)]);
    let removed = call(&runtime, &mut ctx, "REMOVE", &[Word::fixnum(2), left]);
    assert_eq!(list_values(&ctx, removed), [Word::fixnum(1)]);
    let unchanged = call(&runtime, &mut ctx, "REMOVE", &[Word::fixnum(9), left]);
    assert_eq!(
        list_values(&ctx, unchanged),
        [Word::fixnum(1), Word::fixnum(2), Word::fixnum(2)]
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SUBSETP", &[one, left]),
        Word::TRUE
    );
    let intersection = call(&runtime, &mut ctx, "INTERSECTION", &[left, right]);
    assert_eq!(list_values(&ctx, intersection), [Word::fixnum(2)]);
    let difference = call(&runtime, &mut ctx, "SET-DIFFERENCE", &[left, right]);
    assert_eq!(list_values(&ctx, difference), [Word::fixnum(1)]);
    assert!(call(&runtime, &mut ctx, "UNION", &[left, right]).is_cons());
}

#[test]
fn higher_order_sequence_operations_assert_results() {
    let (runtime, mut ctx) = setup();
    let first = list(&runtime, &mut ctx, &[Word::fixnum(1)]);
    let second = list(&runtime, &mut ctx, &[Word::fixnum(2)]);
    let third = list(&runtime, &mut ctx, &[Word::fixnum(3)]);
    let source = list(&runtime, &mut ctx, &[first, second, third]);
    let car = FunctionObject::try_from(runtime.function(&mut ctx, "COMMON-LISP", "CAR").unwrap())
        .unwrap()
        .as_word();
    let mapped = call(&runtime, &mut ctx, "MAPCAR", &[car, source]);
    assert_eq!(
        list_values(&ctx, mapped),
        [Word::fixnum(1), Word::fixnum(2), Word::fixnum(3)]
    );
    assert_eq!(
        call(&runtime, &mut ctx, "EVERY", &[car, source]),
        Word::TRUE
    );
    assert_eq!(call(&runtime, &mut ctx, "SOME", &[car, source]), Word::TRUE);
}
