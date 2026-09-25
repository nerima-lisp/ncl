#![allow(missing_docs)]

use ncl_lib_hash_arrays::array::{Dimension, Dimensions, RowMajorIndex};
use ncl_object::{
    make_array, make_specialized_array, ArrayElementType, ArrayOptions, BuiltinPackage,
    FunctionObject, Runtime, ThreadContext, Word,
};

fn runtime_and_context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("create runtime");
    let mut context = ThreadContext::new();
    context.register(&runtime).expect("register runtime");
    (runtime, context)
}

fn function(runtime: &Runtime, context: &mut ThreadContext, name: &str) -> FunctionObject {
    let word = runtime
        .function(context, BuiltinPackage::CommonLisp.as_str(), name)
        .expect("builtin function");
    FunctionObject::try_from(word).expect("function object")
}

#[test]
fn dimensions_produce_checked_row_major_offsets() {
    let dimensions = Dimensions::new([Dimension::new(2), Dimension::new(3)]).unwrap();

    assert_eq!(dimensions.rank(), 2);
    assert_eq!(dimensions.total_size().unwrap(), 6);
    assert_eq!(
        dimensions
            .row_major_index(&[Dimension::new(1), Dimension::new(2)])
            .unwrap()
            .value(),
        5
    );
}

#[test]
fn checked_offsets_reject_the_total_size() {
    assert!(RowMajorIndex::new(6, 6).is_err());
}

#[test]
fn runtime_call_builtin_covers_aref_row_major_and_svref_boundaries() {
    let (runtime, mut context) = runtime_and_context();
    ncl_lib_hash_arrays::array::register(&mut context, &runtime).expect("register arrays");
    let array = make_array(
        &mut context,
        &runtime,
        &[2, 2],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .expect("array");
    let aref = function(&runtime, &mut context, "AREF");
    assert_eq!(
        runtime.call_builtin(
            &mut context,
            aref,
            &[array, Word::fixnum(1), Word::fixnum(0)]
        ),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut context,
            aref,
            &[array, Word::fixnum(2), Word::fixnum(0)]
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    let row_major = function(&runtime, &mut context, "ROW-MAJOR-AREF");
    assert_eq!(
        runtime.call_builtin(&mut context, row_major, &[array, Word::fixnum(4)]),
        Err(ncl_object::ObjectError::TypeError)
    );
    let svref = function(&runtime, &mut context, "SVREF");
    assert_eq!(
        runtime.call_builtin(&mut context, svref, &[array, Word::fixnum(0)]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn runtime_call_builtin_covers_bit_operations_and_type_errors() {
    let (runtime, mut context) = runtime_and_context();
    ncl_lib_hash_arrays::array::register(&mut context, &runtime).expect("register arrays");
    let left = make_specialized_array(
        &mut context,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(1), Word::fixnum(0)],
    )
    .expect("left");
    let right = make_specialized_array(
        &mut context,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(1), Word::fixnum(1)],
    )
    .expect("right");
    let bit_and = function(&runtime, &mut context, "BIT-AND");
    let result = runtime
        .call_builtin(&mut context, bit_and, &[left, right])
        .expect("bit and");
    let bit = function(&runtime, &mut context, "BIT");
    assert_eq!(
        runtime.call_builtin(&mut context, bit, &[result, Word::fixnum(1)]),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        runtime.call_builtin(&mut context, bit, &[Word::fixnum(3), Word::fixnum(0)]),
        Err(ncl_object::ObjectError::TypeError)
    );
}
