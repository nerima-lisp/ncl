#![allow(clippy::unwrap_used, missing_docs)]

use ncl_object::{FunctionObject, ObjectError, Runtime, ThreadContext, Word};

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
    let function =
        FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap();
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

fn dotted(runtime: &Runtime, ctx: &mut ThreadContext, car: Word, cdr: Word) -> Word {
    call(runtime, ctx, "CONS", &[car, cdr]).unwrap()
}

fn function(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> Word {
    FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap())
        .unwrap()
        .as_word()
}

#[test]
fn list_and_registration_branches_return_table_values_or_errors() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let property_list = list(&runtime, &mut ctx, &[one, two, three, Word::fixnum(4)]);
    let malformed_pair = dotted(&runtime, &mut ctx, one, two);
    let malformed_value_cell = dotted(&runtime, &mut ctx, two, three);
    let malformed_tail = dotted(&runtime, &mut ctx, one, malformed_value_cell);

    let cases = [
        (
            "GETF hit",
            call(&runtime, &mut ctx, "GETF", &[property_list, one]),
            Ok(two),
        ),
        (
            "GETF default",
            call(
                &runtime,
                &mut ctx,
                "GETF",
                &[property_list, Word::fixnum(9), three],
            ),
            Ok(three),
        ),
        (
            "GETF dotted value cell",
            call(&runtime, &mut ctx, "GETF", &[malformed_pair, one]),
            Err(ObjectError::TypeError),
        ),
        (
            "GETF dotted cursor",
            call(
                &runtime,
                &mut ctx,
                "GETF",
                &[malformed_tail, Word::fixnum(9)],
            ),
            Err(ObjectError::TypeError),
        ),
        (
            "ENDP nil",
            call(&runtime, &mut ctx, "ENDP", &[Word::NIL]),
            Ok(Word::TRUE),
        ),
        (
            "ENDP cons",
            call(&runtime, &mut ctx, "ENDP", &[property_list]),
            Ok(Word::NIL),
        ),
        (
            "ENDP atom",
            call(&runtime, &mut ctx, "ENDP", &[one]),
            Err(ObjectError::TypeError),
        ),
    ];
    for (name, actual, expected) in cases {
        assert_eq!(actual, expected, "{name}");
    }

    let text = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();
    let vector = ncl_object::make_simple_vector(&mut ctx, &runtime, &[one, two]).unwrap();
    let reversed_text = call(&runtime, &mut ctx, "REVERSE", &[text]).unwrap();
    assert_eq!(
        (0..2)
            .map(|index| ncl_object::string_ref(&ctx, reversed_text, index).unwrap())
            .collect::<String>(),
        "ba"
    );
    let reversed_vector = call(&runtime, &mut ctx, "REVERSE", &[vector]).unwrap();
    assert_eq!(
        (0..2)
            .map(|index| ncl_object::simple_vector_ref(&ctx, reversed_vector, index).unwrap())
            .collect::<Vec<_>>(),
        vec![two, one]
    );
    assert_eq!(
        call(&runtime, &mut ctx, "NREVERSE", &[vector]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn copy_tree_and_higher_order_empty_branches_preserve_exact_values() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let nested_item = list(&runtime, &mut ctx, &[one]);
    let nested = list(&runtime, &mut ctx, &[nested_item, two]);
    let copied = call(&runtime, &mut ctx, "COPY-TREE", &[nested]).unwrap();
    assert_ne!(copied, nested);
    let copied_item = ncl_object::car(&ctx, copied).unwrap();
    assert_ne!(copied_item, ncl_object::car(&ctx, nested).unwrap());
    assert_eq!(ncl_object::car(&ctx, copied_item), Ok(one));
    assert_eq!(
        ncl_object::cdr(&ctx, copied).and_then(|tail| ncl_object::car(&ctx, tail)),
        Ok(two)
    );

    let atom = function(&runtime, &mut ctx, "ATOM");
    let empty_map = call(&runtime, &mut ctx, "MAP", &[Word::NIL, atom, Word::NIL]);
    assert_eq!(empty_map, Ok(Word::NIL));
    for (name, expected) in [
        ("EVERY", Word::TRUE),
        ("SOME", Word::NIL),
        ("NOTANY", Word::TRUE),
        ("NOTEVERY", Word::NIL),
    ] {
        assert_eq!(
            call(&runtime, &mut ctx, name, &[atom, Word::NIL]),
            Ok(expected),
            "{name}"
        );
    }

    let string = ncl_object::make_string(&mut ctx, &runtime, &['x']).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "MAP-INTO", &[string, atom, Word::NIL]),
        Err(ObjectError::TypeError)
    );
}
