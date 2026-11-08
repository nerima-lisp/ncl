use super::*;

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
