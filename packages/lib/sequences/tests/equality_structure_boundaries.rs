#![allow(missing_docs, clippy::unwrap_used)]

use std::collections::HashMap;

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, ObjectError, Runtime, ThreadContext, Word,
    make_array, make_complex, make_double, make_ratio,
};

fn setup() -> (Runtime, ThreadContext, HashMap<String, FunctionObject>) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_sequences::register(&runtime).unwrap();
    let names = ["EQUAL", "EQUALP"];
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

#[test]
fn equalp_compares_complex_components_and_hash_table_entries() {
    let (runtime, mut ctx, functions) = setup();
    let complex = make_complex(&mut ctx, &runtime, Word::fixnum(2), Word::fixnum(3))
        .unwrap()
        .as_word();
    let same = make_complex(&mut ctx, &runtime, Word::fixnum(2), Word::fixnum(3))
        .unwrap()
        .as_word();
    let different = make_complex(&mut ctx, &runtime, Word::fixnum(2), Word::fixnum(4))
        .unwrap()
        .as_word();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUALP", &[complex, same]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUALP",
            &[complex, different],
        ),
        Ok(Word::NIL)
    );

    let left = HashTable::new(&mut ctx, &runtime, HashTest::Equalp, Weakness::None)
        .unwrap()
        .as_word();
    let right = HashTable::new(&mut ctx, &runtime, HashTest::Equalp, Weakness::None)
        .unwrap()
        .as_word();
    HashTable::from_word(left)
        .insert(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(10))
        .unwrap();
    HashTable::from_word(right)
        .insert(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(10))
        .unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUALP", &[left, right]),
        Ok(Word::TRUE)
    );
    HashTable::from_word(right)
        .insert(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(11))
        .unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUALP", &[left, right]),
        Ok(Word::NIL)
    );
    let different_key = HashTable::new(&mut ctx, &runtime, HashTest::Equalp, Weakness::None)
        .unwrap()
        .as_word();
    HashTable::from_word(different_key)
        .insert(&mut ctx, &runtime, Word::fixnum(2), Word::fixnum(10))
        .unwrap();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUALP",
            &[left, different_key],
        ),
        Ok(Word::NIL)
    );
}

#[test]
fn equality_returns_exact_values_for_depth_cross_type_and_general_array_edges() {
    let (runtime, mut ctx, functions) = setup();
    let build_deep = |ctx: &mut ThreadContext| {
        let mut value = Word::NIL;
        for _ in 0..66 {
            value = ncl_object::make_cons(ctx, &runtime, Word::fixnum(1), value).unwrap();
        }
        value
    };
    let left = build_deep(&mut ctx);
    let right = build_deep(&mut ctx);
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUAL", &[left, right]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUALP", &[left, right]),
        Ok(Word::NIL)
    );

    let ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(2), Word::fixnum(3))
        .unwrap()
        .as_word();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUAL",
            &[Word::fixnum(2), ratio],
        ),
        Ok(Word::NIL)
    );
    let complex = make_complex(&mut ctx, &runtime, Word::fixnum(4), Word::fixnum(0))
        .unwrap()
        .as_word();
    let double = make_double(&mut ctx, &runtime, 4.0).unwrap().as_word();
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUALP", &[double, complex],),
        Ok(Word::TRUE)
    );

    let left_table = HashTable::new(&mut ctx, &runtime, HashTest::Eq, Weakness::None)
        .unwrap()
        .as_word();
    let right_table = HashTable::new(&mut ctx, &runtime, HashTest::Equalp, Weakness::None)
        .unwrap()
        .as_word();
    HashTable::from_word(left_table)
        .insert(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2))
        .unwrap();
    HashTable::from_word(right_table)
        .insert(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2))
        .unwrap();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUALP",
            &[left_table, right_table],
        ),
        Ok(Word::NIL)
    );

    let options = ArrayOptions {
        element_type: ArrayElementType::T,
        initial_element: Word::fixnum(7),
        adjustable: false,
        fill_pointer: None,
        displaced_to: None,
        displaced_index_offset: 0,
    };
    let left_array = make_array(&mut ctx, &runtime, &[2], options).unwrap();
    let right_array = make_array(&mut ctx, &runtime, &[2], options).unwrap();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUALP",
            &[left_array, right_array],
        ),
        Ok(Word::TRUE)
    );
}
