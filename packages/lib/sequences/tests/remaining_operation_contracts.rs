#![allow(missing_docs, clippy::too_many_lines, clippy::unwrap_used)]

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

fn list_values(ctx: &ThreadContext, mut value: Word) -> Vec<Word> {
    let mut result = Vec::new();
    while value != Word::NIL {
        result.push(ncl_object::car(ctx, value).unwrap());
        value = ncl_object::cdr(ctx, value).unwrap();
    }
    result
}

fn vector_values(ctx: &ThreadContext, value: Word) -> Vec<Word> {
    (0..ncl_object::simple_vector_length(ctx, value).unwrap())
        .map(|index| ncl_object::simple_vector_ref(ctx, value, index).unwrap())
        .collect()
}

fn function(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> Word {
    FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap())
        .unwrap()
        .as_word()
}

fn keyword(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
    let package = runtime.find_package(ctx, "KEYWORD").unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

#[test]
fn fill_and_replace_report_element_and_range_contracts() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let start = keyword(&mut ctx, &runtime, "START");
    let end = keyword(&mut ctx, &runtime, "END");

    let text = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b', 'c']).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "FILL", &[text, one]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "FILL",
            &[text, Word::character(u32::from('z')), start, two, end, one],
        ),
        Err(ObjectError::TypeError)
    );

    let destination = ncl_object::make_simple_vector(&mut ctx, &runtime, &[one, one, one]).unwrap();
    let source = list(&runtime, &mut ctx, &[two, three]);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "REPLACE",
            &[destination, source, start, one, end, Word::fixnum(3)],
        ),
        Ok(destination)
    );
    assert_eq!(vector_values(&ctx, destination), vec![one, two, three]);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "REPLACE",
            &[destination, Word::fixnum(9)],
        ),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn sort_returns_same_key_order_and_rejects_invalid_shapes() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let key = keyword(&mut ctx, &runtime, "KEY");
    let car = function(&runtime, &mut ctx, "CAR");
    let lessp = function(&runtime, &mut ctx, "EQUAL");
    let first = list(&runtime, &mut ctx, &[one, three]);
    let second = list(&runtime, &mut ctx, &[one, two]);
    let third = list(&runtime, &mut ctx, &[two, three]);
    let values = list(&runtime, &mut ctx, &[first, second, third]);

    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "STABLE-SORT",
            &[values, lessp, key, car],
        )
        .map(|value| list_values(&ctx, value)),
        Ok(vec![second, first, third])
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SORT", &[Word::fixnum(7), lessp]),
        Err(ObjectError::TypeError)
    );
    let dotted = call(&runtime, &mut ctx, "CONS", &[one, two]).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "SORT", &[dotted, lessp]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn map_into_uses_shortest_length_and_rejects_destination_or_source_types() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let three = Word::fixnum(3);
    let destination = ncl_object::make_simple_vector(&mut ctx, &runtime, &[Word::NIL; 3]).unwrap();
    let source = list(&runtime, &mut ctx, &[one]);
    let atom = function(&runtime, &mut ctx, "ATOM");

    assert_eq!(
        call(&runtime, &mut ctx, "MAP-INTO", &[destination, atom, source]),
        Ok(destination)
    );
    assert_eq!(
        vector_values(&ctx, destination),
        vec![Word::TRUE, Word::NIL, Word::NIL]
    );

    let text = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "MAP-INTO", &[text, atom, source]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MAP-INTO", &[destination, atom, three]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MAP-INTO", &[destination, one, source]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn accessors_and_list_operations_reject_improper_or_non_list_inputs() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let dotted = call(&runtime, &mut ctx, "CONS", &[one, two]).unwrap();

    assert_eq!(call(&runtime, &mut ctx, "CAR", &[Word::NIL]), Ok(Word::NIL));
    assert_eq!(call(&runtime, &mut ctx, "CDR", &[Word::NIL]), Ok(Word::NIL));
    assert_eq!(
        call(&runtime, &mut ctx, "FIRST", &[dotted]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "NTHCDR", &[Word::fixnum(1), dotted]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "LIST-LENGTH", &[dotted]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "RPLACA", &[one, two]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "NCONC", &[dotted, one]),
        Err(ObjectError::TypeError)
    );
}
