use super::*;

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
