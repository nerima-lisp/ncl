//! Tests for the registered EQUAL and EQUALP builtins.

use std::collections::HashMap;

use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, Runtime, ThreadContext, Word,
    array_row_major_set, make_array, make_simple_vector, make_specialized_array, make_string,
};

fn setup() -> (Runtime, ThreadContext, HashMap<String, FunctionObject>) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context: {error:?}"));
    ncl_lib_sequences::register(&runtime).unwrap_or_else(|error| panic!("sequences: {error:?}"));
    let functions = ["EQUAL", "EQUALP"]
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

fn call(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    functions: &HashMap<String, FunctionObject>,
    name: &str,
    args: &[Word],
) -> Word {
    let function = *functions
        .get(name)
        .unwrap_or_else(|| panic!("missing builtin {name}"));
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        let rooted_args = roots.iter().map(|root| **root).collect::<Vec<_>>();
        runtime.call_builtin(ctx, function, &rooted_args)
    })
    .unwrap_or_else(|error| panic!("{name} failed: {error:?}"))
}

fn make_general_array(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    dimensions: &[usize],
    element_type: ArrayElementType,
    fill_pointer: Option<usize>,
    values: &[Word],
) -> Word {
    let mut array = make_array(
        ctx,
        runtime,
        dimensions,
        ArrayOptions {
            element_type,
            initial_element: if matches!(element_type, ArrayElementType::Bit) {
                Word::fixnum(0)
            } else if matches!(
                element_type,
                ArrayElementType::Character | ArrayElementType::BaseChar
            ) {
                Word::character(0)
            } else {
                Word::NIL
            },
            adjustable: false,
            fill_pointer,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap_or_else(|error| panic!("general array: {error:?}"));
    let root = ncl_object::push_root(ctx, &mut array);
    for (index, value) in values.iter().copied().enumerate() {
        array_row_major_set(ctx, array, index, value)
            .unwrap_or_else(|error| panic!("set row-major element: {error:?}"));
    }
    assert!(ncl_object::pop_root(ctx, root));
    array
}

#[test]
fn equalp_compares_active_elements_of_fill_pointer_vectors() {
    let (runtime, mut ctx, functions) = setup();
    let active_values = [1, 1, 1, 3, 5, 7].map(Word::fixnum);
    let capacity_values = [1, 1, 1, 3, 5, 7, 2, 6].map(Word::fixnum);
    let mut simple = make_simple_vector(&mut ctx, &runtime, &active_values)
        .unwrap_or_else(|error| panic!("simple vector: {error:?}"));
    let simple_root = ncl_object::push_root(&mut ctx, &mut simple);

    let mut active_vector = make_general_array(
        &mut ctx,
        &runtime,
        &[8],
        ArrayElementType::T,
        Some(active_values.len()),
        &capacity_values,
    );
    let active_root = ncl_object::push_root(&mut ctx, &mut active_vector);

    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUALP",
            &[simple, active_vector],
        ),
        Word::TRUE
    );

    assert!(ncl_object::pop_root(&mut ctx, active_root));
    assert!(ncl_object::pop_root(&mut ctx, simple_root));
}

#[test]
fn equalp_requires_matching_fill_pointers_for_general_vectors() {
    let (runtime, mut ctx, functions) = setup();
    let capacity_values = [1, 1, 1, 3, 5, 7, 2, 6].map(Word::fixnum);
    let mut six_active = make_general_array(
        &mut ctx,
        &runtime,
        &[8],
        ArrayElementType::T,
        Some(6),
        &capacity_values,
    );
    let six_active_root = ncl_object::push_root(&mut ctx, &mut six_active);
    let mut five_active = make_general_array(
        &mut ctx,
        &runtime,
        &[8],
        ArrayElementType::T,
        Some(5),
        &capacity_values,
    );
    let five_active_root = ncl_object::push_root(&mut ctx, &mut five_active);

    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUALP",
            &[six_active, five_active],
        ),
        Word::NIL
    );

    assert!(ncl_object::pop_root(&mut ctx, five_active_root));
    assert!(ncl_object::pop_root(&mut ctx, six_active_root));
}

#[test]
fn equalp_requires_matching_dimensions_and_elements() {
    let (runtime, mut ctx, functions) = setup();
    let values = [1, 2, 3, 4, 5, 6].map(Word::fixnum);
    let mut vector = make_simple_vector(&mut ctx, &runtime, &values)
        .unwrap_or_else(|error| panic!("simple vector: {error:?}"));
    let vector_root = ncl_object::push_root(&mut ctx, &mut vector);
    let mut matrix = make_general_array(
        &mut ctx,
        &runtime,
        &[2, 3],
        ArrayElementType::T,
        None,
        &values,
    );
    let matrix_root = ncl_object::push_root(&mut ctx, &mut matrix);
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUALP", &[vector, matrix]),
        Word::NIL
    );

    let different_values = [1, 2, 3, 4, 5, 0].map(Word::fixnum);
    let mut different = make_simple_vector(&mut ctx, &runtime, &different_values)
        .unwrap_or_else(|error| panic!("different simple vector: {error:?}"));
    let different_root = ncl_object::push_root(&mut ctx, &mut different);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUALP",
            &[vector, different],
        ),
        Word::NIL
    );

    assert!(ncl_object::pop_root(&mut ctx, different_root));
    assert!(ncl_object::pop_root(&mut ctx, matrix_root));
    assert!(ncl_object::pop_root(&mut ctx, vector_root));
}

#[test]
fn equalp_compares_string_and_vector_arrays_case_insensitively() {
    let (runtime, mut ctx, functions) = setup();
    let mut string = make_string(&mut ctx, &runtime, &['F', 'o', 'O'])
        .unwrap_or_else(|error| panic!("string: {error:?}"));
    let string_root = ncl_object::push_root(&mut ctx, &mut string);
    let mut vector = make_simple_vector(
        &mut ctx,
        &runtime,
        &[
            Word::character(u32::from('f')),
            Word::character(u32::from('O')),
            Word::character(u32::from('o')),
        ],
    )
    .unwrap_or_else(|error| panic!("character vector: {error:?}"));
    let vector_root = ncl_object::push_root(&mut ctx, &mut vector);

    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUALP", &[string, vector]),
        Word::TRUE
    );

    assert!(ncl_object::pop_root(&mut ctx, vector_root));
    assert!(ncl_object::pop_root(&mut ctx, string_root));
}

#[test]
fn equal_compares_bit_vectors_element_by_element() {
    let (runtime, mut ctx, functions) = setup();
    let mut left = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(1), Word::fixnum(0), Word::fixnum(1)],
    )
    .unwrap_or_else(|error| panic!("left bit vector: {error:?}"));
    let left_root = ncl_object::push_root(&mut ctx, &mut left);
    let mut same = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(1), Word::fixnum(0), Word::fixnum(1)],
    )
    .unwrap_or_else(|error| panic!("matching bit vector: {error:?}"));
    let same_root = ncl_object::push_root(&mut ctx, &mut same);
    let mut different = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(1), Word::fixnum(1), Word::fixnum(1)],
    )
    .unwrap_or_else(|error| panic!("different bit vector: {error:?}"));
    let different_root = ncl_object::push_root(&mut ctx, &mut different);

    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUAL", &[left, same]),
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUAL", &[left, different]),
        Word::NIL
    );

    assert!(ncl_object::pop_root(&mut ctx, different_root));
    assert!(ncl_object::pop_root(&mut ctx, same_root));
    assert!(ncl_object::pop_root(&mut ctx, left_root));
}

#[test]
fn equal_is_elementwise_only_for_general_bit_vectors() {
    let (runtime, mut ctx, functions) = setup();
    let left_values = [1, 0, 1, 0, 0].map(Word::fixnum);
    let right_values = [1, 0, 1, 1, 1].map(Word::fixnum);
    let mut left = make_general_array(
        &mut ctx,
        &runtime,
        &[5],
        ArrayElementType::Bit,
        Some(3),
        &left_values,
    );
    let left_root = ncl_object::push_root(&mut ctx, &mut left);
    let mut right = make_general_array(
        &mut ctx,
        &runtime,
        &[5],
        ArrayElementType::Bit,
        Some(3),
        &right_values,
    );
    let right_root = ncl_object::push_root(&mut ctx, &mut right);

    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUAL", &[left, right]),
        Word::TRUE
    );

    let different_active_values = [1, 1, 1, 0, 0].map(Word::fixnum);
    let mut different_active = make_general_array(
        &mut ctx,
        &runtime,
        &[5],
        ArrayElementType::Bit,
        Some(3),
        &different_active_values,
    );
    let different_active_root = ncl_object::push_root(&mut ctx, &mut different_active);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUAL",
            &[left, different_active],
        ),
        Word::NIL
    );
    assert!(ncl_object::pop_root(&mut ctx, different_active_root));

    assert!(ncl_object::pop_root(&mut ctx, right_root));
    assert!(ncl_object::pop_root(&mut ctx, left_root));
}

#[test]
fn equal_compares_non_fill_pointer_bit_vectors_and_not_matrices() {
    let (runtime, mut ctx, functions) = setup();
    let plain_values = [1, 0, 1].map(Word::fixnum);
    let mut first_vector = make_general_array(
        &mut ctx,
        &runtime,
        &[3],
        ArrayElementType::Bit,
        None,
        &plain_values,
    );
    let first_vector_root = ncl_object::push_root(&mut ctx, &mut first_vector);
    let mut second_vector = make_general_array(
        &mut ctx,
        &runtime,
        &[3],
        ArrayElementType::Bit,
        None,
        &plain_values,
    );
    let second_vector_root = ncl_object::push_root(&mut ctx, &mut second_vector);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUAL",
            &[first_vector, second_vector],
        ),
        Word::TRUE
    );
    assert!(ncl_object::pop_root(&mut ctx, second_vector_root));
    assert!(ncl_object::pop_root(&mut ctx, first_vector_root));

    let matrix_values = [1, 0, 1].map(Word::fixnum);
    let mut first_matrix = make_general_array(
        &mut ctx,
        &runtime,
        &[1, 3],
        ArrayElementType::Bit,
        None,
        &matrix_values,
    );
    let first_matrix_root = ncl_object::push_root(&mut ctx, &mut first_matrix);
    let mut second_matrix = make_general_array(
        &mut ctx,
        &runtime,
        &[1, 3],
        ArrayElementType::Bit,
        None,
        &matrix_values,
    );
    let second_matrix_root = ncl_object::push_root(&mut ctx, &mut second_matrix);

    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUAL",
            &[first_matrix, second_matrix],
        ),
        Word::NIL
    );

    assert!(ncl_object::pop_root(&mut ctx, second_matrix_root));
    assert!(ncl_object::pop_root(&mut ctx, first_matrix_root));
}

#[test]
fn equal_compares_general_character_vectors_as_strings() {
    let (runtime, mut ctx, functions) = setup();
    let mut string = make_string(&mut ctx, &runtime, &['a', 'b', 'c'])
        .unwrap_or_else(|error| panic!("string: {error:?}"));
    let string_root = ncl_object::push_root(&mut ctx, &mut string);
    let matching_values =
        ['a', 'b', 'c', 'x', 'y'].map(|character| Word::character(u32::from(character)));
    let mut matching_array = make_general_array(
        &mut ctx,
        &runtime,
        &[5],
        ArrayElementType::Character,
        Some(3),
        &matching_values,
    );
    let matching_root = ncl_object::push_root(&mut ctx, &mut matching_array);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUAL",
            &[string, matching_array]
        ),
        Word::TRUE
    );

    let other_matching_values =
        ['a', 'b', 'c', 'p', 'q'].map(|character| Word::character(u32::from(character)));
    let mut other_matching_array = make_general_array(
        &mut ctx,
        &runtime,
        &[5],
        ArrayElementType::Character,
        Some(3),
        &other_matching_values,
    );
    let other_matching_root = ncl_object::push_root(&mut ctx, &mut other_matching_array);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUAL",
            &[matching_array, other_matching_array],
        ),
        Word::TRUE
    );

    let different_values =
        ['a', 'z', 'c', 'y', 'x'].map(|character| Word::character(u32::from(character)));
    let mut different_array = make_general_array(
        &mut ctx,
        &runtime,
        &[5],
        ArrayElementType::Character,
        Some(3),
        &different_values,
    );
    let different_root = ncl_object::push_root(&mut ctx, &mut different_array);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUAL",
            &[string, different_array],
        ),
        Word::NIL
    );

    let shorter_values =
        ['a', 'b', 'x', 'y', 'z'].map(|character| Word::character(u32::from(character)));
    let mut shorter_array = make_general_array(
        &mut ctx,
        &runtime,
        &[5],
        ArrayElementType::Character,
        Some(2),
        &shorter_values,
    );
    let shorter_root = ncl_object::push_root(&mut ctx, &mut shorter_array);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUAL",
            &[string, shorter_array],
        ),
        Word::NIL
    );
    assert!(ncl_object::pop_root(&mut ctx, shorter_root));
    assert!(ncl_object::pop_root(&mut ctx, different_root));
    assert!(ncl_object::pop_root(&mut ctx, other_matching_root));
    assert!(ncl_object::pop_root(&mut ctx, matching_root));
    assert!(ncl_object::pop_root(&mut ctx, string_root));
}

#[test]
fn equal_does_not_descend_character_matrices() {
    let (runtime, mut ctx, functions) = setup();
    let matrix_values = ['a', 'b', 'c'].map(|character| Word::character(u32::from(character)));
    let mut first_matrix = make_general_array(
        &mut ctx,
        &runtime,
        &[1, 3],
        ArrayElementType::Character,
        None,
        &matrix_values,
    );
    let first_matrix_root = ncl_object::push_root(&mut ctx, &mut first_matrix);
    let mut second_matrix = make_general_array(
        &mut ctx,
        &runtime,
        &[1, 3],
        ArrayElementType::Character,
        None,
        &matrix_values,
    );
    let second_matrix_root = ncl_object::push_root(&mut ctx, &mut second_matrix);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            &functions,
            "EQUAL",
            &[first_matrix, second_matrix],
        ),
        Word::NIL
    );

    assert!(ncl_object::pop_root(&mut ctx, second_matrix_root));
    assert!(ncl_object::pop_root(&mut ctx, first_matrix_root));
}

#[test]
fn equal_uses_identity_for_other_general_arrays() {
    let (runtime, mut ctx, functions) = setup();
    let values = [Word::fixnum(1), Word::fixnum(2), Word::fixnum(3)];
    let mut first =
        make_general_array(&mut ctx, &runtime, &[3], ArrayElementType::T, None, &values);
    let first_root = ncl_object::push_root(&mut ctx, &mut first);
    let mut second =
        make_general_array(&mut ctx, &runtime, &[3], ArrayElementType::T, None, &values);
    let second_root = ncl_object::push_root(&mut ctx, &mut second);

    assert_eq!(
        call(&runtime, &mut ctx, &functions, "EQUAL", &[first, second]),
        Word::NIL
    );

    assert!(ncl_object::pop_root(&mut ctx, second_root));
    assert!(ncl_object::pop_root(&mut ctx, first_root));
}
