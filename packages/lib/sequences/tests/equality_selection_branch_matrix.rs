#![allow(missing_docs, clippy::too_many_lines, clippy::unwrap_used)]

use std::collections::HashMap;

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, ObjectError, Package, Runtime, ThreadContext,
    Word, make_array, make_complex, make_double, make_simple_vector, make_specialized_array,
    make_string,
};

fn setup() -> (Runtime, ThreadContext, HashMap<String, FunctionObject>) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_sequences::register(&runtime).unwrap();
    let functions = [
        "COUNT", "EQUAL", "EQUALP", "FIND", "LIST", "MISMATCH", "POSITION", "SEARCH",
    ]
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
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

#[test]
fn equality_branch_matrix_compares_mixed_arrays_and_numeric_fallbacks() {
    let (runtime, mut ctx, functions) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let text = make_string(&mut ctx, &runtime, &['a', 'b']).unwrap();
    let vector = make_simple_vector(
        &mut ctx,
        &runtime,
        &[
            Word::character(u32::from('A')),
            Word::character(u32::from('B')),
        ],
    )
    .unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUALP", &[text, vector]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUAL", &[text, vector]),
        Ok(Word::NIL)
    );

    let options = ArrayOptions {
        element_type: ArrayElementType::T,
        initial_element: one,
        adjustable: false,
        fill_pointer: None,
        displaced_to: None,
        displaced_index_offset: 0,
    };
    let general_left = make_array(&mut ctx, &runtime, &[2], options).unwrap();
    let general_right = make_array(&mut ctx, &runtime, &[2], options).unwrap();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUAL",
            &[general_left, general_right],
        ),
        Ok(Word::NIL)
    );

    let bits = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[one, Word::fixnum(0)],
    )
    .unwrap();
    let integers = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Unsigned,
        &[one, Word::fixnum(0)],
    )
    .unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUAL", &[bits, integers]),
        Ok(Word::NIL)
    );

    let double = make_double(&mut ctx, &runtime, 2.0).unwrap().as_word();
    let complex = make_complex(&mut ctx, &runtime, two, Word::fixnum(1))
        .unwrap()
        .as_word();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUALP", &[double, complex]),
        Ok(Word::NIL)
    );
    let same_complex = make_complex(&mut ctx, &runtime, two, Word::fixnum(1))
        .unwrap()
        .as_word();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUALP",
            &[complex, same_complex],
        ),
        Ok(Word::TRUE)
    );

    let left = HashTable::new(&mut ctx, &runtime, HashTest::Equalp, Weakness::None)
        .unwrap()
        .as_word();
    let right = HashTable::new(&mut ctx, &runtime, HashTest::Equalp, Weakness::None)
        .unwrap()
        .as_word();
    HashTable::from_word(left)
        .insert(&mut ctx, &runtime, one, two)
        .unwrap();
    HashTable::from_word(right)
        .insert(&mut ctx, &runtime, one, two)
        .unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUALP", &[left, right]),
        Ok(Word::TRUE)
    );
    HashTable::from_word(right)
        .insert(&mut ctx, &runtime, Word::fixnum(3), two)
        .unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUALP", &[left, right]),
        Ok(Word::NIL)
    );
}

#[test]
fn selection_branch_matrix_returns_exact_keyed_test_not_and_empty_results() {
    let (runtime, mut ctx, functions) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);
    let source = list(&runtime, &mut ctx, &functions, &[one, two, three, two]);
    let test_not = keyword(&mut ctx, &runtime, "TEST-NOT");
    let key = keyword(&mut ctx, &runtime, "KEY");
    let car = FunctionObject::try_from(runtime.function(&mut ctx, "COMMON-LISP", "CAR").unwrap())
        .unwrap()
        .as_word();
    let nested_one = list(&runtime, &mut ctx, &functions, &[one]);
    let nested_two = list(&runtime, &mut ctx, &functions, &[two]);
    let nested = list(&runtime, &mut ctx, &functions, &[nested_one, nested_two]);
    let equal =
        FunctionObject::try_from(runtime.function(&mut ctx, "COMMON-LISP", "EQUAL").unwrap())
            .unwrap()
            .as_word();

    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "FIND",
            &[two, nested, key, car, test_not, equal],
        ),
        Ok(nested_one)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "COUNT",
            &[two, source, test_not, equal],
        ),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "POSITION",
            &[three, source, test_not, equal],
        ),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "SEARCH",
            &[nested, source, test_not, equal],
        ),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "MISMATCH",
            &[source, source, test_not, equal],
        ),
        Ok(Word::fixnum(0))
    );

    let empty = ncl_object::make_simple_vector(&mut ctx, &runtime, &[]).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "SEARCH", &[empty, source]),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "MISMATCH", &[empty, source]),
        Ok(Word::fixnum(0))
    );
    let start = keyword(&mut ctx, &runtime, "START");
    let end = keyword(&mut ctx, &runtime, "END");
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "FIND",
            &[one, source, start, Word::fixnum(4), end, Word::fixnum(2)],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "COUNT",
            &[one, source, start, Word::fixnum(1), end, Word::fixnum(3)],
        ),
        Ok(Word::fixnum(0))
    );
}
