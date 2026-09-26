#![allow(missing_docs)]

use ncl_object::{
    ArrayElementType, ArrayOptions, ObjectError, Package, Runtime, ThreadContext, Word,
    bignum_limbs, bignum_sign, code_constants, code_debug, code_entry, code_size, code_stack_map,
    complex_imag, complex_real, function_code, function_entry, function_lambda_list, function_name,
    instance_class, make_array, make_bignum_from_i128, make_code_object, make_complex, make_double,
    make_instance, make_ratio, make_readtable, make_simple_fun, make_specialized_array,
    make_stream, make_string, make_structure, ratio_denominator, ratio_numerator, readtable_case,
    readtable_dispatch, readtable_syntax, set_symbol_constant, set_symbol_macro,
    set_symbol_package_locked, set_symbol_special, set_symbol_value, slot_ref, slot_set,
    specialized_array_element_type, specialized_array_ref, specialized_array_set, stream_direction,
    stream_element_type, stream_external_format, stream_implementation, stream_state,
    structure_layout, structure_ref, structure_set, symbol_flags, symbol_is_constant,
    symbol_is_macro, symbol_is_package_locked, symbol_is_special, symbol_name, symbol_value,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut context = ThreadContext::new();
    context
        .register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    (runtime, context)
}

#[test]
fn object_constructors_and_accessors_preserve_payloads() {
    let (runtime, mut context) = setup();
    let name = make_string(&mut context, &runtime, &['F', 'N']).unwrap_or(Word::NIL);
    let lambda = make_string(&mut context, &runtime, &['X']).unwrap_or(Word::NIL);
    let code = make_code_object(
        &mut context,
        &runtime,
        17,
        3,
        Word::fixnum(1),
        Word::fixnum(2),
        Word::fixnum(3),
    )
    .unwrap_or_else(|error| panic!("code: {error:?}"));
    assert_eq!(code_entry(&context, code), Ok(Word::fixnum(17)));
    assert_eq!(code_size(&context, code), Ok(Word::fixnum(3)));
    assert_eq!(code_constants(&context, code), Ok(Word::fixnum(1)));
    assert_eq!(code_stack_map(&context, code), Ok(Word::fixnum(2)));
    assert_eq!(code_debug(&context, code), Ok(Word::fixnum(3)));
    assert_eq!(
        ncl_object::code_slot(&context, code, 9),
        Err(ObjectError::Storage(
            ncl_sys::StorageCondition::ThreadNotRegistered
        ))
    );

    let function = make_simple_fun(&mut context, &runtime, 17, name, lambda, code)
        .unwrap_or_else(|error| panic!("function: {error:?}"));
    assert_eq!(function_entry(&context, function), Ok(17));
    assert_eq!(function_name(&context, function), Ok(name));
    assert_eq!(function_lambda_list(&context, function), Ok(lambda));
    assert_eq!(function_code(&context, function), Ok(code));
    assert_eq!(
        ncl_object::closure_ref(&context, function, 0),
        Err(ObjectError::TypeError)
    );

    let closure = ncl_object::make_closure(
        &mut context,
        &runtime,
        19,
        name,
        lambda,
        code,
        &[Word::fixnum(7), Word::TRUE],
    )
    .unwrap_or_else(|error| panic!("closure: {error:?}"));
    assert_eq!(
        ncl_object::closure_ref(&context, closure, 1),
        Ok(Word::TRUE)
    );
    assert_eq!(
        ncl_object::closure_ref(&context, closure, 2),
        Err(ObjectError::Storage(
            ncl_sys::StorageCondition::ThreadNotRegistered
        ))
    );

    let instance = make_instance(&mut context, &runtime, Word::fixnum(9), &[Word::fixnum(1)])
        .unwrap_or_else(|error| panic!("instance: {error:?}"));
    assert_eq!(instance_class(&context, instance), Ok(Word::fixnum(9)));
    assert_eq!(slot_ref(&context, instance, 0), Ok(Word::fixnum(1)));
    assert_eq!(slot_set(&mut context, instance, 0, Word::fixnum(8)), Ok(()));
    assert_eq!(slot_ref(&context, instance, 0), Ok(Word::fixnum(8)));
    assert_eq!(slot_ref(&context, instance, 1), Err(ObjectError::TypeError));
}

#[test]
fn numbers_arrays_structures_and_descriptor_slots_are_observable() {
    let (runtime, mut context) = setup();
    assert_number_payloads(&runtime, &mut context);
    assert_array_payloads(&runtime, &mut context);
    assert_structure_and_descriptor_payloads(&runtime, &mut context);
}

fn assert_number_payloads(runtime: &Runtime, context: &mut ThreadContext) {
    let big = make_bignum_from_i128(context, runtime, -((1_i128 << 40) + 5))
        .unwrap_or_else(|error| panic!("bignum: {error:?}"));
    assert!(bignum_sign(context, big).unwrap_or(false));
    assert_eq!(bignum_limbs(context, big), Ok(vec![5, 256]));
    let ratio = make_ratio(context, runtime, Word::fixnum(2), Word::fixnum(3))
        .unwrap_or_else(|error| panic!("ratio: {error:?}"));
    assert_eq!(ratio_numerator(context, ratio), Ok(Word::fixnum(2)));
    assert_eq!(ratio_denominator(context, ratio), Ok(Word::fixnum(3)));
    let double =
        make_double(context, runtime, 1.25).unwrap_or_else(|error| panic!("double: {error:?}"));
    assert_eq!(ncl_object::double_value(context, double), Ok(1.25));
    let complex = make_complex(context, runtime, Word::fixnum(4), Word::fixnum(5))
        .unwrap_or_else(|error| panic!("complex: {error:?}"));
    assert_eq!(complex_real(context, complex), Ok(Word::fixnum(4)));
    assert_eq!(complex_imag(context, complex), Ok(Word::fixnum(5)));
}

fn assert_array_payloads(runtime: &Runtime, context: &mut ThreadContext) {
    let bits = make_specialized_array(
        context,
        runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(0), Word::fixnum(1)],
    )
    .unwrap_or(Word::NIL);
    assert_eq!(
        specialized_array_element_type(context, bits),
        Ok(ArrayElementType::Bit)
    );
    assert_eq!(specialized_array_ref(context, bits, 1), Ok(Word::fixnum(1)));
    assert_eq!(
        specialized_array_set(context, bits, 0, Word::fixnum(1)),
        Ok(())
    );
    assert_eq!(
        specialized_array_set(context, bits, 0, Word::fixnum(2)),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        make_specialized_array(context, runtime, ArrayElementType::T, &[]),
        Err(ObjectError::TypeError)
    );

    let array = make_array(
        context,
        runtime,
        &[2],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::fixnum(6),
            adjustable: true,
            fill_pointer: Some(1),
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap_or(Word::NIL);
    assert_eq!(ncl_object::array_dimensions(context, array), Ok(vec![2]));
    assert_eq!(
        ncl_object::array_row_major_ref(context, array, 0),
        Ok(Word::fixnum(6))
    );
    assert_eq!(
        ncl_object::array_row_major_set(context, array, 1, Word::TRUE),
        Ok(())
    );
    assert_eq!(
        ncl_object::array_row_major_ref(context, array, 1),
        Ok(Word::TRUE)
    );
}

fn assert_structure_and_descriptor_payloads(runtime: &Runtime, context: &mut ThreadContext) {
    let layout = runtime
        .register_structure_layout(1)
        .unwrap_or_else(|_| panic!("layout"));
    let structure =
        make_structure(context, runtime, layout, &[Word::fixnum(10)]).unwrap_or(Word::NIL);
    assert_eq!(structure_layout(context, structure), Ok(layout));
    assert_eq!(structure_ref(context, structure, 0), Ok(Word::fixnum(10)));
    assert_eq!(
        structure_set(context, structure, 0, Word::fixnum(11)),
        Ok(())
    );
    assert_eq!(structure_ref(context, structure, 0), Ok(Word::fixnum(11)));

    let readtable = make_readtable(
        context,
        runtime,
        Word::fixnum(1),
        Word::fixnum(2),
        Word::fixnum(3),
    )
    .unwrap_or_else(|_| panic!("readtable"));
    assert_eq!(readtable_syntax(context, readtable), Ok(Word::fixnum(1)));
    assert_eq!(readtable_dispatch(context, readtable), Ok(Word::fixnum(2)));
    assert_eq!(readtable_case(context, readtable), Ok(Word::fixnum(3)));
    let stream = make_stream(
        context,
        runtime,
        Word::fixnum(1),
        Word::fixnum(2),
        Word::fixnum(3),
        Word::fixnum(4),
        Word::fixnum(5),
    )
    .unwrap_or_else(|_| panic!("stream"));
    assert_eq!(stream_direction(context, stream), Ok(Word::fixnum(1)));
    assert_eq!(stream_element_type(context, stream), Ok(Word::fixnum(2)));
    assert_eq!(stream_external_format(context, stream), Ok(Word::fixnum(3)));
    assert_eq!(stream_state(context, stream), Ok(Word::fixnum(4)));
    assert_eq!(stream_implementation(context, stream), Ok(Word::fixnum(5)));
}

#[test]
fn symbols_registry_and_control_state_report_mutations() {
    let (runtime, mut context) = setup();
    let package_word = runtime
        .ensure_package(&mut context, "N05")
        .unwrap_or(Word::NIL);
    let package = Package::from_word(package_word);
    let (symbol, _) = package
        .intern(&mut context, &runtime, "VALUE")
        .unwrap_or((Word::NIL, ncl_object::FindStatus::Internal));
    let value = Word::fixnum(42);
    assert_eq!(
        symbol_name(&context, symbol).and_then(|word| ncl_object::string_ref(&context, word, 0)),
        Ok('V')
    );
    assert_eq!(set_symbol_value(&mut context, symbol, value), Ok(()));
    assert_eq!(symbol_value(&context, symbol), Ok(value));
    for (set, query, bit) in [
        (
            set_symbol_special as fn(&mut ThreadContext, Word, bool) -> Result<(), ObjectError>,
            symbol_is_special as fn(&ThreadContext, Word) -> Result<bool, ObjectError>,
            1,
        ),
        (set_symbol_constant, symbol_is_constant, 2),
        (set_symbol_macro, symbol_is_macro, 4),
        (set_symbol_package_locked, symbol_is_package_locked, 8),
    ] {
        assert_eq!(set(&mut context, symbol, true), Ok(()));
        assert!(query(&context, symbol).unwrap_or(false));
        assert_ne!(symbol_flags(&context, symbol).unwrap_or(0) & bit, 0);
        assert_eq!(set(&mut context, symbol, false), Ok(()));
        assert!(!query(&context, symbol).unwrap_or(true));
    }
    assert_eq!(runtime.find_package(&context, "N05"), Some(package_word));
    assert_eq!(runtime.define_class(&mut context, "C", Word::TRUE), Ok(()));
    assert_eq!(runtime.class(&mut context, "C"), Some(Word::TRUE));
    runtime.add_feature("N05");
    runtime.add_feature("N05");
    assert_eq!(runtime.features(), vec!["N05"]);

    context.set_non_local_exit(true);
    assert!(context.take_non_local_exit());
    assert!(!context.take_non_local_exit());
    context.set_control_pointers(Some(1), Some(2), None);
    assert_eq!(context.control_pointers(), (Some(1), Some(2), None));
    assert_eq!(
        context.write_object_slot(Word::NIL, 0, Word::NIL),
        Err(ObjectError::Storage(
            ncl_sys::StorageCondition::ThreadNotRegistered
        ))
    );
}
