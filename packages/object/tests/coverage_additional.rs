#![allow(missing_docs)]

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    array_dimensions, array_row_major_ref, array_row_major_set, bignum_limbs, bignum_sign, car,
    cdr, classify, classify_object, complex_imag, complex_real, double_value, function_code,
    function_lambda_list, make_array, make_bignum_from_i128, make_closure, make_code_object,
    make_complex, make_cons, make_double, make_instance, make_ratio, make_readtable,
    make_simple_fun, make_simple_vector, make_stream, make_string, make_structure, make_symbol,
    pop_root, push_root, ratio_denominator, ratio_numerator, rplaca, rplacd, set_symbol_constant,
    set_symbol_macro, set_symbol_package_locked, set_symbol_special, set_symbol_value,
    simple_vector_length, simple_vector_set, slot_ref, slot_set, specialized_array_element_type,
    specialized_array_set, stream_direction, stream_element_type, stream_external_format,
    stream_implementation, stream_state, string_length, string_ref, string_set, structure_layout,
    structure_ref, structure_set, symbol_function, symbol_is_constant, symbol_is_macro,
    symbol_is_package_locked, symbol_is_special, symbol_package, symbol_plist, Arity,
    ArrayElementType, ArrayOptions, Builtin, BuiltinArgs, BuiltinConvention, BuiltinImplementation,
    BuiltinName, BuiltinPackage, FunctionObject, LambdaList, MultipleValues, ObjectError,
    ObjectRef, ObjectType, Runtime, ThreadContext, WordView,
};
use ncl_sys::Word;

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut context = ThreadContext::new();
    context
        .register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    (runtime, context)
}

fn callback(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    values.push(Word::fixnum(i64::try_from(args.len()).unwrap_or(i64::MAX)));
    args.required(0)
}

#[test]
fn builtin_value_objects_and_failure_paths_are_observable() {
    let (runtime, mut context) = context();
    let identifier = ncl_object::BuiltinIdentifier::new(
        BuiltinPackage::new("NCL-COVERAGE"),
        BuiltinName::new("CALL"),
    );
    let descriptor = Builtin {
        lambda_list: LambdaList::new("value"),
        convention: BuiltinConvention::Direct(Arity::exact(1)),
    };
    assert_eq!(descriptor.lambda_list.as_str(), "value");
    assert_eq!(descriptor.convention.arity(), Some(Arity::exact(1)));
    assert!(descriptor.convention.direct());
    let implementation = BuiltinImplementation::direct(descriptor, callback).with_entry(1234);
    let function = runtime
        .register_builtin(&mut context, identifier, implementation)
        .unwrap_or_else(|error| panic!("builtin: {error:?}"));
    assert_eq!(runtime.builtin_descriptor(function), Some(descriptor));
    assert_eq!(
        runtime.call_builtin(&mut context, function, &[]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut context, function, &[Word::fixnum(8)]),
        Ok(Word::fixnum(8))
    );
    assert_eq!(context.values(), &[Word::fixnum(1)]);
    assert_eq!(
        runtime.call_builtin(&mut context, FunctionObject::from(Word::UNBOUND), &[]),
        Err(ObjectError::Unbound)
    );
    assert_eq!(
        runtime.call_builtin(&mut context, FunctionObject::from(Word::fixnum(1)), &[]),
        Err(ObjectError::Unbound)
    );
    let argument_words = [Word::fixnum(4)];
    let args = BuiltinArgs::new(&argument_words);
    assert_eq!(args.get(0), Some(Word::fixnum(4)));
    assert_eq!(args.get(1), None);
    assert!(!args.is_empty());
    assert_eq!(
        BuiltinArgs::new(&[]).required(0),
        Err(ObjectError::TypeError)
    );
    let mut values = MultipleValues::new();
    assert!(values.is_empty());
    values.set(&[Word::fixnum(1)]);
    values.push(Word::fixnum(2));
    assert_eq!(values.as_slice(), &[Word::fixnum(1), Word::fixnum(2)]);
    assert_eq!(values.len(), 2);
    values.clear();
    assert!(values.is_empty());
}

#[test]
fn arrays_cover_mutation_displacement_and_validation() {
    let (runtime, mut context) = context();
    let string = make_string(&mut context, &runtime, &['a', 'b']).unwrap_or(Word::NIL);
    assert_eq!(string_length(&context, string), Ok(2));
    assert!(string_set(&mut context, string, 1, 'z').is_ok());
    assert_eq!(string_ref(&context, string, 1), Ok('z'));
    assert_eq!(
        string_set(&mut context, string, 2, 'x'),
        Err(ObjectError::TypeError)
    );
    let vector = make_simple_vector(&mut context, &runtime, &[Word::fixnum(1), Word::fixnum(2)])
        .unwrap_or(Word::NIL);
    assert_eq!(simple_vector_length(&context, vector), Ok(2));
    assert!(simple_vector_set(&mut context, vector, 1, Word::fixnum(3)).is_ok());
    assert_eq!(
        ncl_object::simple_vector_ref(&context, vector, 1),
        Ok(Word::fixnum(3))
    );
    assert_eq!(
        simple_vector_set(&mut context, vector, 2, Word::NIL),
        Err(ObjectError::TypeError)
    );
    let array = make_array(
        &mut context,
        &runtime,
        &[2],
        ArrayOptions {
            element_type: ArrayElementType::Character,
            initial_element: Word::character('q' as u32),
            adjustable: true,
            fill_pointer: Some(2),
            displaced_to: Some(vector),
            displaced_index_offset: 0,
        },
    )
    .unwrap_or(Word::NIL);
    assert_eq!(array_dimensions(&context, array), Ok(vec![2]));
    assert_eq!(array_row_major_ref(&context, array, 0), Ok(Word::fixnum(1)));
    assert!(array_row_major_set(&mut context, array, 1, Word::fixnum(9)).is_ok());
    assert_eq!(
        ncl_object::simple_vector_ref(&context, vector, 1),
        Ok(Word::fixnum(9))
    );
    assert_eq!(
        array_row_major_ref(&context, array, 2),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        make_array(
            &mut context,
            &runtime,
            &[],
            ArrayOptions {
                element_type: ArrayElementType::T,
                initial_element: Word::NIL,
                adjustable: false,
                fill_pointer: None,
                displaced_to: None,
                displaced_index_offset: 0,
            }
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        make_array(
            &mut context,
            &runtime,
            &[2, 2],
            ArrayOptions {
                element_type: ArrayElementType::T,
                initial_element: Word::NIL,
                adjustable: false,
                fill_pointer: Some(1),
                displaced_to: None,
                displaced_index_offset: 0,
            }
        ),
        Err(ObjectError::TypeError)
    );
    let specialized = ncl_object::make_specialized_array(
        &mut context,
        &runtime,
        ArrayElementType::Unsigned,
        &[Word::fixnum(4)],
    )
    .unwrap_or(Word::NIL);
    assert_eq!(
        specialized_array_element_type(&context, specialized),
        Ok(ArrayElementType::Unsigned)
    );
    assert!(specialized_array_set(&mut context, specialized, 0, Word::fixnum(7)).is_ok());
}

#[test]
fn cons_numbers_and_object_views_round_trip() {
    let (runtime, mut context) = context();
    let cons =
        make_cons(&mut context, &runtime, Word::fixnum(1), Word::fixnum(2)).unwrap_or(Word::NIL);
    assert_eq!(car(&mut context, cons), Ok(Word::fixnum(1)));
    assert_eq!(cdr(&mut context, cons), Ok(Word::fixnum(2)));
    assert_eq!(rplaca(&mut context, cons, Word::fixnum(3)), Ok(cons));
    assert_eq!(rplacd(&mut context, cons, Word::fixnum(4)), Ok(cons));
    assert_eq!(car(&mut context, cons), Ok(Word::fixnum(3)));
    assert_eq!(cdr(&mut context, cons), Ok(Word::fixnum(4)));
    assert_eq!(car(&mut context, Word::NIL), Ok(Word::NIL));
    let bignum = make_bignum_from_i128(&mut context, &runtime, -0x1_0000_0001)
        .unwrap_or_else(|error| panic!("bignum: {error:?}"));
    assert_eq!(bignum_sign(&context, bignum), Ok(true));
    assert_eq!(bignum_limbs(&context, bignum), Ok(vec![1, 1]));
    let positive = make_bignum_from_i128(&mut context, &runtime, 0x1_0000_0001)
        .unwrap_or_else(|error| panic!("bignum: {error:?}"));
    assert_eq!(bignum_sign(&context, positive), Ok(false));
    let double = make_double(&mut context, &runtime, 2.5)
        .unwrap_or_else(|error| panic!("double: {error:?}"));
    assert_eq!(double_value(&context, double), Ok(2.5));
    let ratio = make_ratio(&mut context, &runtime, bignum.into(), double.into())
        .unwrap_or_else(|error| panic!("ratio: {error:?}"));
    assert_eq!(ratio_numerator(&context, ratio), Ok(bignum.into()));
    assert_eq!(ratio_denominator(&context, ratio), Ok(double.into()));
    let complex = make_complex(&mut context, &runtime, ratio.into(), positive.into())
        .unwrap_or_else(|error| panic!("complex: {error:?}"));
    assert_eq!(complex_real(&context, complex), Ok(ratio.into()));
    assert_eq!(complex_imag(&context, complex), Ok(positive.into()));
    assert!(matches!(
        classify_object(&context, complex.into()),
        ObjectRef::Complex(_)
    ));
    assert_eq!(
        WordView::try_from_word(Word::fixnum(7), ObjectType::Fixnum).map(WordView::as_word),
        Ok(Word::fixnum(7))
    );
    assert_eq!(
        WordView::try_from_word(Word::TRUE, ObjectType::Character).map(WordView::as_word),
        Err(ncl_object::TypeError {
            datum: Word::TRUE,
            expected: ObjectType::Character
        })
    );
}

#[test]
fn structures_instances_streams_readtables_and_symbols_are_mutable() {
    let (runtime, mut context) = context();
    let layout = runtime
        .register_structure_layout(2)
        .unwrap_or_else(|_| panic!("layout"));
    let structure = make_structure(
        &mut context,
        &runtime,
        layout,
        &[Word::fixnum(1), Word::fixnum(2)],
    )
    .unwrap_or(Word::NIL);
    assert_eq!(structure_layout(&context, structure), Ok(layout));
    assert_eq!(structure_ref(&context, structure, 1), Ok(Word::fixnum(2)));
    assert!(structure_set(&mut context, structure, 0, Word::fixnum(8)).is_ok());
    assert_eq!(structure_ref(&context, structure, 0), Ok(Word::fixnum(8)));
    let instance = make_instance(&mut context, &runtime, Word::NIL, &[Word::fixnum(5)])
        .unwrap_or_else(|error| panic!("instance: {error:?}"));
    assert_eq!(slot_ref(&context, instance, 0), Ok(Word::fixnum(5)));
    assert!(slot_set(&mut context, instance, 0, Word::fixnum(6)).is_ok());
    assert_eq!(slot_ref(&context, instance, 0), Ok(Word::fixnum(6)));
    let stream = make_stream(
        &mut context,
        &runtime,
        Word::fixnum(1),
        Word::fixnum(2),
        Word::fixnum(3),
        Word::fixnum(4),
        Word::fixnum(5),
    )
    .unwrap_or_else(|error| panic!("stream: {error:?}"));
    assert_eq!(stream_direction(&context, stream), Ok(Word::fixnum(1)));
    assert_eq!(stream_element_type(&context, stream), Ok(Word::fixnum(2)));
    assert_eq!(
        stream_external_format(&context, stream),
        Ok(Word::fixnum(3))
    );
    assert_eq!(stream_implementation(&context, stream), Ok(Word::fixnum(5)));
    assert_eq!(stream_state(&context, stream), Ok(Word::fixnum(4)));
    let readtable = make_readtable(
        &mut context,
        &runtime,
        Word::fixnum(1),
        Word::fixnum(2),
        Word::fixnum(3),
    )
    .unwrap_or_else(|error| panic!("readtable: {error:?}"));
    assert_eq!(
        ncl_object::readtable_case(&context, readtable),
        Ok(Word::fixnum(3))
    );
    assert_eq!(
        ncl_object::readtable_dispatch(&context, readtable),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        ncl_object::readtable_syntax(&context, readtable),
        Ok(Word::fixnum(1))
    );
    let symbol = make_symbol(&mut context, &runtime, Word::NIL).unwrap_or(Word::NIL);
    assert_eq!(symbol_package(&context, symbol), Ok(Word::NIL));
    assert_eq!(symbol_plist(&context, symbol), Ok(Word::NIL));
    assert_eq!(symbol_function(&context, symbol), Ok(Word::UNBOUND));
    for setter in [
        set_symbol_special,
        set_symbol_constant,
        set_symbol_macro,
        set_symbol_package_locked,
    ] {
        assert!(setter(&mut context, symbol, true).is_ok());
    }
    assert!(symbol_is_special(&context, symbol).unwrap_or(false));
    assert!(symbol_is_constant(&context, symbol).unwrap_or(false));
    assert!(symbol_is_macro(&context, symbol).unwrap_or(false));
    assert!(symbol_is_package_locked(&context, symbol).unwrap_or(false));
    assert!(set_symbol_value(&mut context, symbol, Word::fixnum(10)).is_ok());
}

#[test]
fn roots_hash_tables_and_function_slots_report_contracts() {
    let (runtime, mut context) = context();
    let mut value = Word::fixnum(9);
    let token = push_root(&mut context, &mut value);
    assert!(pop_root(&mut context, token));
    let table = HashTable::new(&mut context, &runtime, HashTest::Equalp, Weakness::Key)
        .unwrap_or_else(|_| panic!("table"));
    assert!(table
        .insert(&mut context, &runtime, Word::fixnum(1), Word::fixnum(2))
        .is_ok());
    assert_eq!(
        table.get(&mut context, Word::fixnum(1)),
        Ok(Some(Word::fixnum(2)))
    );
    assert_eq!(
        table.remove(&mut context, &runtime, Word::fixnum(1)),
        Ok(Some(Word::fixnum(2)))
    );
    assert_eq!(table.get(&mut context, Word::fixnum(1)), Ok(None));
    let code = make_code_object(
        &mut context,
        &runtime,
        11,
        12,
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap_or_else(|error| panic!("code: {error:?}"));
    let fun = make_simple_fun(&mut context, &runtime, 13, Word::NIL, Word::NIL, code)
        .unwrap_or_else(|error| panic!("function: {error:?}"));
    assert_eq!(function_code(&context, fun), Ok(code));
    assert_eq!(function_lambda_list(&context, fun), Ok(Word::NIL));
    let closure = make_closure(
        &mut context,
        &runtime,
        14,
        Word::NIL,
        Word::NIL,
        code,
        &[Word::fixnum(1)],
    )
    .unwrap_or_else(|error| panic!("closure: {error:?}"));
    assert_eq!(
        ncl_object::closure_ref(&context, closure, 0),
        Ok(Word::fixnum(1))
    );
    assert!(ncl_object::closure_ref(&context, closure, 1).is_err());
    assert_eq!(classify(Word::character(65)), ObjectRef::Character(65));
    let mut rooted = Word::fixnum(3);
    let root = push_root(&mut context, &mut rooted);
    assert!(pop_root(&mut context, root));
    assert!(!pop_root(&mut context, root));
}
