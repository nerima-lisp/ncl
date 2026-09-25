#![allow(missing_docs)]

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ArrayElementType, ArrayOptions, FindStatus, ObjectError, Package, Runtime, ThreadContext,
    WordView, array_dimensions, array_row_major_ref, array_row_major_set, bignum_limbs,
    bignum_sign, classify_object, complex_imag, complex_real, double_value, make_array,
    make_bignum_from_i128, make_complex, make_cons, make_double, make_ratio, make_simple_vector,
    make_string, ratio_denominator, ratio_numerator, specialized_array_element_type,
    specialized_array_ref, specialized_array_set, symbol_name,
};
use ncl_sys::Word;

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut context = ThreadContext::new();
    context
        .register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    (runtime, context)
}

#[test]
fn hash_modes_cover_numeric_string_cons_and_iteration_contracts() {
    let (runtime, mut context) = setup();
    let negative = make_bignum_from_i128(&mut context, &runtime, -0x1_0000_0001)
        .unwrap_or_else(|error| panic!("bignum: {error:?}"));
    let same_negative = make_bignum_from_i128(&mut context, &runtime, -0x1_0000_0001)
        .unwrap_or_else(|error| panic!("bignum: {error:?}"));
    let double = make_double(&mut context, &runtime, 3.5)
        .unwrap_or_else(|error| panic!("double: {error:?}"));
    let same_double = make_double(&mut context, &runtime, 3.5)
        .unwrap_or_else(|error| panic!("double: {error:?}"));
    let ratio = make_ratio(&mut context, &runtime, negative.into(), double.into())
        .unwrap_or_else(|error| panic!("ratio: {error:?}"));
    let same_ratio = make_ratio(
        &mut context,
        &runtime,
        same_negative.into(),
        same_double.into(),
    )
    .unwrap_or_else(|error| panic!("ratio: {error:?}"));
    let complex = make_complex(&mut context, &runtime, ratio.into(), negative.into())
        .unwrap_or_else(|error| panic!("complex: {error:?}"));
    let same_complex = make_complex(
        &mut context,
        &runtime,
        same_ratio.into(),
        same_negative.into(),
    )
    .unwrap_or_else(|error| panic!("complex: {error:?}"));
    let lower = make_string(&mut context, &runtime, &['a', 'b']).unwrap_or(Word::NIL);
    let upper = make_string(&mut context, &runtime, &['A', 'B']).unwrap_or(Word::NIL);
    let left = make_cons(&mut context, &runtime, lower, Word::fixnum(4)).unwrap_or(Word::NIL);
    let right = make_cons(&mut context, &runtime, upper, Word::fixnum(4)).unwrap_or(Word::NIL);

    for (test, key, equal_key) in [
        (HashTest::Eq, Word::fixnum(1), Word::fixnum(1)),
        (HashTest::Eql, negative.into(), same_negative.into()),
        (HashTest::Equal, ratio.into(), same_ratio.into()),
        (HashTest::Equalp, left, right),
    ] {
        let table = HashTable::new(&mut context, &runtime, test, Weakness::None)
            .unwrap_or_else(|error| panic!("table: {error:?}"));
        assert_eq!(table.test(&context), Ok(test));
        assert_eq!(table.weakness(&context), Ok(Weakness::None));
        assert_eq!(table.count(&context), Ok(0));
        assert!(table.capacity(&context).unwrap_or(0) >= 8);
        assert!(
            table
                .insert(&mut context, &runtime, key, Word::fixnum(9))
                .is_ok()
        );
        assert_eq!(
            table.get(&mut context, equal_key),
            Ok(Some(Word::fixnum(9)))
        );
        let mut entries = Vec::new();
        assert!(
            table
                .for_each_entry(&context, |stored_key, value| entries
                    .push((stored_key, value)))
                .is_ok()
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(
            table.remove(&mut context, &runtime, equal_key),
            Ok(Some(Word::fixnum(9)))
        );
        assert_eq!(table.remove(&mut context, &runtime, equal_key), Ok(None));
    }
    assert_eq!(bignum_sign(&context, negative), Ok(true));
    assert_eq!(bignum_limbs(&context, negative), Ok(vec![1, 1]));
    assert_eq!(double_value(&context, double), Ok(3.5));
    assert_eq!(ratio_numerator(&context, ratio), Ok(negative.into()));
    assert_eq!(ratio_denominator(&context, ratio), Ok(double.into()));
    assert_eq!(complex_real(&context, complex), Ok(ratio.into()));
    assert_eq!(complex_imag(&context, complex), Ok(negative.into()));
    assert_eq!(
        WordView::from(classify_object(&context, same_complex.into())).as_word(),
        same_complex.into()
    );
}

#[test]
fn package_registry_and_visibility_operations_cover_all_statuses() {
    let (runtime, mut context) = setup();
    let producer = Package::from_word(
        runtime
            .ensure_package(&mut context, "PRODUCER")
            .unwrap_or_else(|error| panic!("producer: {error:?}")),
    );
    let consumer = Package::new(&mut context, &runtime, "CONSUMER")
        .unwrap_or_else(|error| panic!("consumer: {error:?}"));
    let nickname = make_string(&mut context, &runtime, &['P', 'R', 'O']).unwrap_or(Word::NIL);
    assert!(
        producer
            .add_nickname(&mut context, &runtime, nickname)
            .is_ok()
    );
    assert!(
        !producer
            .add_nickname(&mut context, &runtime, nickname)
            .unwrap_or(true)
    );
    assert_eq!(
        runtime.find_package(&context, "PRO"),
        Some(producer.as_word())
    );
    assert_eq!(runtime.find_package(&context, "MISSING"), None);
    let name = make_string(&mut context, &runtime, &['X']).unwrap_or(Word::NIL);
    let (symbol, status) = producer
        .intern(&mut context, &runtime, "X")
        .unwrap_or_else(|error| panic!("intern: {error:?}"));
    assert_eq!(status, FindStatus::Internal);
    assert_eq!(
        producer.find_symbol(&mut context, name),
        Ok(Some((symbol, FindStatus::Internal)))
    );
    assert!(
        producer
            .export(&mut context, &runtime, name)
            .unwrap_or(false)
    );
    assert_eq!(
        producer.find_symbol(&mut context, name),
        Ok(Some((symbol, FindStatus::External)))
    );
    assert!(
        producer
            .export(&mut context, &runtime, name)
            .unwrap_or(false)
    );
    assert!(
        producer
            .unexport(&mut context, &runtime, name)
            .unwrap_or(false)
    );
    assert!(
        !producer
            .unexport(&mut context, &runtime, name)
            .unwrap_or(true)
    );
    assert!(
        producer
            .use_package(&mut context, &runtime, producer.as_word())
            .is_ok()
    );
    assert!(
        !producer
            .use_package(&mut context, &runtime, producer.as_word())
            .unwrap_or(true)
    );
    let inherited_name = make_string(&mut context, &runtime, &['Y']).unwrap_or(Word::NIL);
    let (inherited, _) = producer
        .intern(&mut context, &runtime, "Y")
        .unwrap_or_else(|error| panic!("intern: {error:?}"));
    assert!(
        producer
            .export(&mut context, &runtime, inherited_name)
            .unwrap_or(false)
    );
    assert!(
        consumer
            .use_package(&mut context, &runtime, producer.as_word())
            .unwrap_or(false)
    );
    assert_eq!(
        consumer.find_symbol(&mut context, inherited_name),
        Ok(Some((inherited, FindStatus::Inherited)))
    );
    assert!(
        consumer
            .import(&mut context, &runtime, inherited_name, inherited)
            .is_ok()
    );
    assert_eq!(
        consumer.find_symbol(&mut context, inherited_name),
        Ok(Some((inherited, FindStatus::Internal)))
    );
    assert!(
        consumer
            .unuse_package(&mut context, producer.as_word())
            .unwrap_or(false)
    );
    assert!(
        !consumer
            .unuse_package(&mut context, producer.as_word())
            .unwrap_or(true)
    );
    assert!(consumer.shadow(&mut context, &runtime, name).is_ok());
    assert!(consumer.shadow(&mut context, &runtime, name).is_ok());
    assert_ne!(
        consumer.shadowing_symbols(&context).unwrap_or(Word::NIL),
        Word::NIL
    );
    let generated = consumer.gensym(&mut context, &runtime).unwrap_or(Word::NIL);
    assert!(symbol_name(&context, generated).is_ok());
    assert_eq!(
        runtime.ensure_package(&mut context, "PRODUCER"),
        Ok(producer.as_word())
    );
    assert!(runtime.gc_config().dynamic_space_size > 0);
    runtime.add_feature("NCL-COVERAGE");
    runtime.add_feature("NCL-COVERAGE");
    assert_eq!(runtime.features(), vec![String::from("NCL-COVERAGE")]);
    assert!(
        runtime
            .define_class(&mut context, "COVERAGE", Word::TRUE)
            .is_ok()
    );
    assert_eq!(runtime.class(&mut context, "COVERAGE"), Some(Word::TRUE));
    assert_eq!(runtime.class(&mut context, "UNKNOWN"), None);
}

#[test]
fn specialized_and_displaced_arrays_cover_element_validation() {
    let (runtime, mut context) = setup();
    let cases = [
        (ArrayElementType::Bit, Word::fixnum(1)),
        (ArrayElementType::Character, Word::character(65)),
        (ArrayElementType::BaseChar, Word::character(66)),
        (ArrayElementType::Fixnum, Word::fixnum(3)),
        (ArrayElementType::Signed, Word::fixnum(-3)),
        (ArrayElementType::Unsigned, Word::fixnum(3)),
        (ArrayElementType::SingleFloat, Word::TRUE),
        (ArrayElementType::DoubleFloat, Word::TRUE),
    ];
    for (element_type, value) in cases {
        let array =
            ncl_object::make_specialized_array(&mut context, &runtime, element_type, &[value])
                .unwrap_or_else(|error| panic!("specialized array: {error:?}"));
        assert_eq!(
            specialized_array_element_type(&context, array),
            Ok(element_type)
        );
        assert_eq!(specialized_array_ref(&context, array, 0), Ok(value));
        assert!(specialized_array_set(&mut context, array, 0, value).is_ok());
        assert_eq!(
            specialized_array_ref(&context, array, 1),
            Err(ObjectError::TypeError)
        );
    }
    assert_eq!(
        ncl_object::make_specialized_array(&mut context, &runtime, ArrayElementType::T, &[]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        ncl_object::make_specialized_array(
            &mut context,
            &runtime,
            ArrayElementType::Bit,
            &[Word::fixnum(2)]
        ),
        Err(ObjectError::TypeError)
    );
    let target = ncl_object::make_specialized_array(
        &mut context,
        &runtime,
        ArrayElementType::Fixnum,
        &[Word::fixnum(7), Word::fixnum(8)],
    )
    .unwrap_or(Word::NIL);
    let displaced = make_array(
        &mut context,
        &runtime,
        &[1],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: false,
            fill_pointer: None,
            displaced_to: Some(target),
            displaced_index_offset: 1,
        },
    )
    .unwrap_or(Word::NIL);
    assert_eq!(
        array_row_major_ref(&context, displaced, 0),
        Ok(Word::fixnum(8))
    );
    assert!(array_row_major_set(&mut context, displaced, 0, Word::fixnum(11)).is_ok());
    assert_eq!(
        specialized_array_ref(&context, target, 1),
        Ok(Word::fixnum(11))
    );
    assert_eq!(
        array_dimensions(&context, Word::NIL),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        array_row_major_ref(&context, displaced, 1),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn registry_and_root_error_values_are_stable() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut context = ThreadContext::new();
    let mut value = Word::fixnum(1);
    assert!(ncl_object::try_push_root(&mut context, &mut value).is_ok());
    assert_eq!(
        context.collect(false),
        Err(ObjectError::Storage(
            ncl_sys::StorageCondition::ThreadNotRegistered
        ))
    );
    context.set_gc_stress(true);
    assert_eq!(
        ncl_object::allocate(&mut context, &runtime, 1, 0),
        Err(ObjectError::Storage(
            ncl_sys::StorageCondition::ThreadNotRegistered
        ))
    );
    context.set_gc_stress(false);
    assert_eq!(
        ncl_object::classify_object(&context, Word::NIL),
        ncl_object::ObjectRef::Symbol(Word::NIL)
    );
}

#[test]
fn malformed_metadata_and_recursive_equal_values_return_domain_errors() {
    let (runtime, mut context) = setup();
    let table = HashTable::new(&mut context, &runtime, HashTest::Eq, Weakness::None)
        .unwrap_or_else(|error| panic!("table: {error:?}"));
    assert!(
        context
            .write_object_slot(table.into(), 0, Word::fixnum(99))
            .is_ok()
    );
    assert_eq!(table.test(&context), Err(ObjectError::Layout));
    assert!(
        context
            .write_object_slot(table.into(), 0, Word::fixnum(0))
            .is_ok()
    );
    assert!(
        context
            .write_object_slot(table.into(), 1, Word::fixnum(99))
            .is_ok()
    );
    assert_eq!(table.weakness(&context), Err(ObjectError::Layout));
    let specialized = ncl_object::make_specialized_array(
        &mut context,
        &runtime,
        ArrayElementType::Fixnum,
        &[Word::fixnum(1)],
    )
    .unwrap_or(Word::NIL);
    assert!(
        context
            .write_object_slot(specialized, 0, Word::fixnum(99))
            .is_ok()
    );
    assert_eq!(
        specialized_array_element_type(&context, specialized),
        Err(ObjectError::Layout)
    );
    assert_eq!(
        specialized_array_ref(&context, specialized, 0),
        Err(ObjectError::Layout)
    );
    let string = make_string(&mut context, &runtime, &['x']).unwrap_or(Word::NIL);
    assert_eq!(
        ncl_object::string_ref(&context, string, 2),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        ncl_object::simple_vector_ref(&context, Word::NIL, 0),
        Err(ObjectError::TypeError)
    );
    let cycle = make_cons(&mut context, &runtime, Word::fixnum(1), Word::NIL).unwrap_or(Word::NIL);
    assert!(ncl_object::rplacd(&mut context, cycle, cycle).is_ok());
    let recursive = HashTable::new(&mut context, &runtime, HashTest::Equal, Weakness::None)
        .unwrap_or_else(|error| panic!("table: {error:?}"));
    assert!(
        recursive
            .insert(&mut context, &runtime, cycle, Word::TRUE)
            .is_err()
    );
}

#[test]
fn arrays_reject_invalid_options_and_displacement_targets() {
    let (runtime, mut context) = setup();
    let options = |fill_pointer, displaced_to, offset| ArrayOptions {
        element_type: ArrayElementType::T,
        initial_element: Word::NIL,
        adjustable: false,
        fill_pointer,
        displaced_to,
        displaced_index_offset: offset,
    };
    assert_eq!(
        make_array(&mut context, &runtime, &[1], options(Some(2), None, 0)),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        make_array(&mut context, &runtime, &[1, 1], options(Some(1), None, 0)),
        Err(ObjectError::TypeError)
    );
    assert!(
        make_array(
            &mut context,
            &runtime,
            &[2],
            options(None, Some(Word::TRUE), 0)
        )
        .is_ok()
    );
    let empty =
        make_array(&mut context, &runtime, &[0], options(None, None, 0)).unwrap_or(Word::NIL);
    assert_eq!(
        array_row_major_ref(&context, empty, 0),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        array_row_major_set(&mut context, empty, 0, Word::NIL),
        Err(ObjectError::TypeError)
    );
    let vector =
        make_simple_vector(&mut context, &runtime, &[Word::fixnum(1)]).unwrap_or(Word::NIL);
    let displaced = make_array(&mut context, &runtime, &[1], options(None, Some(vector), 1))
        .unwrap_or(Word::NIL);
    assert_eq!(
        array_row_major_ref(&context, displaced, 0),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        array_row_major_set(&mut context, displaced, 0, Word::NIL),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn package_lists_remove_non_head_entries_and_reject_conflicts() {
    let (runtime, mut context) = setup();
    let first = Package::new(&mut context, &runtime, "FIRST").unwrap_or_else(|_| panic!("first"));
    let second =
        Package::new(&mut context, &runtime, "SECOND").unwrap_or_else(|_| panic!("second"));
    let third = Package::new(&mut context, &runtime, "THIRD").unwrap_or_else(|_| panic!("third"));
    assert!(
        first
            .use_package(&mut context, &runtime, second.as_word())
            .unwrap_or(false)
    );
    assert!(
        first
            .use_package(&mut context, &runtime, third.as_word())
            .unwrap_or(false)
    );
    assert!(
        first
            .unuse_package(&mut context, third.as_word())
            .unwrap_or(false)
    );
    assert!(
        first
            .unuse_package(&mut context, second.as_word())
            .unwrap_or(false)
    );
    let name = make_string(&mut context, &runtime, &['C']).unwrap_or(Word::NIL);
    let (left, _) = second
        .intern(&mut context, &runtime, "C")
        .unwrap_or_else(|_| panic!("left"));
    let (right, _) = third
        .intern(&mut context, &runtime, "C")
        .unwrap_or_else(|_| panic!("right"));
    assert!(second.export(&mut context, &runtime, name).unwrap_or(false));
    assert!(third.export(&mut context, &runtime, name).unwrap_or(false));
    assert!(first.import(&mut context, &runtime, name, left).is_ok());
    assert_eq!(
        first.import(&mut context, &runtime, name, right),
        Err(ObjectError::PackageConflict)
    );
    assert!(second.shadow(&mut context, &runtime, name).is_ok());
    assert!(
        second
            .unintern(&mut context, &runtime, name)
            .unwrap_or(false)
    );
    assert_eq!(second.find_symbol(&mut context, name), Ok(None));
}

#[test]
fn control_state_and_instance_boundaries_are_explicit() {
    let (runtime, mut context) = setup();
    assert!(!context.take_non_local_exit());
    context.set_non_local_exit(true);
    assert!(context.take_non_local_exit());
    assert!(!context.take_non_local_exit());
    context.set_control_pointers(Some(1), None, Some(3));
    assert_eq!(context.control_pointers(), (Some(1), None, Some(3)));
    let instance =
        ncl_object::make_instance(&mut context, &runtime, Word::fixnum(7), &[Word::fixnum(1)])
            .unwrap_or_else(|error| panic!("instance: {error:?}"));
    assert_eq!(
        ncl_object::instance_class(&context, instance),
        Ok(Word::fixnum(7))
    );
    assert_eq!(
        ncl_object::slot_ref(&context, instance, 1),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        ncl_object::slot_set(&mut context, instance, 1, Word::NIL),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        context.write_object_slot(Word::NIL, 0, Word::TRUE),
        Err(ObjectError::Storage(
            ncl_sys::StorageCondition::ThreadNotRegistered
        ))
    );
}
