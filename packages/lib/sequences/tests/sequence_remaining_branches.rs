#![allow(missing_docs, clippy::too_many_lines, clippy::unwrap_used)]

use ncl_object::{
    FunctionObject, ObjectError, Package, Runtime, ThreadContext, Word, make_simple_vector,
    make_specialized_array, make_string,
};

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

fn function(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> Word {
    let function = runtime.function(ctx, "COMMON-LISP", name).unwrap();
    FunctionObject::try_from(function).unwrap().as_word()
}

fn list(runtime: &Runtime, ctx: &mut ThreadContext, values: &[Word]) -> Word {
    call(runtime, ctx, "LIST", values).unwrap()
}

fn list_values(ctx: &ThreadContext, mut value: Word) -> Vec<Word> {
    let mut values = Vec::new();
    while value != Word::NIL {
        values.push(ncl_object::car(ctx, value).unwrap());
        value = ncl_object::cdr(ctx, value).unwrap();
    }
    values
}

fn keyword(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
    let package = runtime.find_package(ctx, "KEYWORD").unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

#[test]
fn selection_ranges_and_substitution_return_exact_values() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let four = Word::fixnum(4);
    let replacement = Word::fixnum(9);
    let source = list(&runtime, &mut ctx, &[one, two, one, three, two]);
    let start = keyword(&mut ctx, &runtime, "START");
    let end = keyword(&mut ctx, &runtime, "END");
    let from_end = keyword(&mut ctx, &runtime, "FROM-END");
    let count = keyword(&mut ctx, &runtime, "COUNT");
    let test = keyword(&mut ctx, &runtime, "TEST");
    let equal = function(&runtime, &mut ctx, "EQUAL");
    let atom = function(&runtime, &mut ctx, "ATOM");

    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SUBSTITUTE",
            &[replacement, two, source, start, one, end, four, test, equal],
        )
        .map(|value| list_values(&ctx, value)),
        Ok(vec![one, replacement, one, three, two])
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SUBSTITUTE-IF-NOT",
            &[replacement, atom, source, from_end, Word::TRUE, count, two],
        )
        .map(|value| list_values(&ctx, value)),
        Ok(vec![one, two, one, three, two])
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "COUNT",
            &[one, source, start, one, end, four, test, equal],
        ),
        Ok(Word::fixnum(1))
    );
    assert_eq!(
        call(&runtime, &mut ctx, "FIND", &[Word::fixnum(8), source]),
        Ok(Word::NIL)
    );
    let short = list(&runtime, &mut ctx, &[one, two]);
    let long = list(&runtime, &mut ctx, &[one, two, three]);
    assert_eq!(
        call(&runtime, &mut ctx, "SEARCH", &[long, short]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MISMATCH", &[long, short]),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "POSITION",
            &[one, source, start, Word::fixnum(4), end, Word::fixnum(2)],
        ),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn equality_distinguishes_cons_arrays_and_specialized_bits() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let left = list(&runtime, &mut ctx, &[one, two]);
    let right = list(&runtime, &mut ctx, &[one, two]);
    let different = list(&runtime, &mut ctx, &[one, three]);
    assert_eq!(
        call(&runtime, &mut ctx, "EQUAL", &[left, right]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "EQUAL", &[left, different]),
        Ok(Word::NIL)
    );

    let vector_left = make_simple_vector(&mut ctx, &runtime, &[one, two]).unwrap();
    let vector_right = make_simple_vector(&mut ctx, &runtime, &[one, two]).unwrap();
    let vector_other = make_simple_vector(&mut ctx, &runtime, &[one, three]).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "EQUALP", &[vector_left, vector_right]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "EQUALP", &[vector_left, vector_other]),
        Ok(Word::NIL)
    );

    let bits_left = make_specialized_array(
        &mut ctx,
        &runtime,
        ncl_object::ArrayElementType::Bit,
        &[one, Word::fixnum(0), one],
    )
    .unwrap();
    let bits_right = make_specialized_array(
        &mut ctx,
        &runtime,
        ncl_object::ArrayElementType::Bit,
        &[one, Word::fixnum(0), one],
    )
    .unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "EQUAL", &[bits_left, bits_right]),
        Ok(Word::TRUE)
    );

    let text = make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();
    let upper = make_string(&mut ctx, &runtime, &['A', 'B']).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "EQUAL", &[text, upper]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "EQUALP", &[text, upper]),
        Ok(Word::TRUE)
    );
}

#[test]
fn order_and_list_destructive_operations_preserve_contracts() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let four = Word::fixnum(4);
    let first = list(&runtime, &mut ctx, &[one, two]);
    let second = list(&runtime, &mut ctx, &[three, four]);

    assert_eq!(
        call(&runtime, &mut ctx, "NCONC", &[first, Word::NIL, second]),
        Ok(first)
    );
    assert_eq!(list_values(&ctx, first), vec![one, two, three, four]);
    assert_eq!(
        call(&runtime, &mut ctx, "RPLACA", &[first, Word::fixnum(9)]),
        Ok(first)
    );
    assert_eq!(
        list_values(&ctx, first),
        vec![Word::fixnum(9), two, three, four]
    );

    let reversed = list(&runtime, &mut ctx, &[one, two, three]);
    assert_eq!(
        call(&runtime, &mut ctx, "NREVERSE", &[reversed]).map(|value| list_values(&ctx, value)),
        Ok(vec![three, two, one])
    );
    assert_eq!(
        call(&runtime, &mut ctx, "APPEND", &[first, Word::NIL])
            .map(|value| list_values(&ctx, value)),
        Ok(vec![Word::fixnum(9), two, three, four])
    );

    let key = keyword(&mut ctx, &runtime, "KEY");
    let car = function(&runtime, &mut ctx, "CAR");
    let comparator = function(&runtime, &mut ctx, "CONS");
    let keyed_one = list(&runtime, &mut ctx, &[one, Word::fixnum(10)]);
    let keyed_two = list(&runtime, &mut ctx, &[one, Word::fixnum(20)]);
    let keyed = list(&runtime, &mut ctx, &[keyed_two, keyed_one]);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "STABLE-SORT",
            &[keyed, comparator, key, car],
        ),
        Ok(keyed)
    );
    assert_eq!(list_values(&ctx, keyed), vec![keyed_one, keyed_two]);
    assert_eq!(
        call(&runtime, &mut ctx, "UNION", &[first, second, key, car],),
        Err(ObjectError::TypeError)
    );
}
