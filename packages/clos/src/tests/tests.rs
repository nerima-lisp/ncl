#![allow(
    clippy::all,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::similar_names,
    clippy::indexing_slicing
)]
use super::*;
use ncl_object::{make_cons, make_simple_vector};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("context");
    (runtime, ctx)
}

#[test]
fn class_precedence_list_reads_cons_parent_and_stops_at_nil() {
    let (runtime, mut ctx) = setup();
    let parent = make_simple_vector(
        &mut ctx,
        &runtime,
        &[Word::fixnum(1), Word::NIL, Word::NIL, Word::fixnum(0)],
    )
    .expect("parent");
    let parent_list = make_cons(&mut ctx, &runtime, parent, Word::NIL).expect("parent list");
    let child = make_simple_vector(
        &mut ctx,
        &runtime,
        &[Word::fixnum(2), parent_list, Word::NIL, Word::fixnum(0)],
    )
    .expect("child");

    let precedence = class_precedence_list(&mut ctx, &runtime, child).expect("precedence");
    assert_eq!(simple_vector_length(&ctx, precedence), Ok(2));
    assert_eq!(simple_vector_ref(&ctx, precedence, 0), Ok(child));
    assert_eq!(simple_vector_ref(&ctx, precedence, 1), Ok(parent));
}

#[test]
fn class_slots_selects_effective_field_only_for_extended_descriptors() {
    let (runtime, mut ctx) = setup();
    let direct = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(10)]).expect("direct");
    let effective = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(20)]).expect("effective");
    let legacy = make_simple_vector(
        &mut ctx,
        &runtime,
        &[Word::fixnum(0), Word::NIL, direct, Word::fixnum(0)],
    )
    .expect("legacy");
    let extended = make_simple_vector(
        &mut ctx,
        &runtime,
        &[
            Word::fixnum(0),
            Word::NIL,
            direct,
            Word::fixnum(0),
            effective,
        ],
    )
    .expect("extended");

    assert_eq!(
        class_slots_builtin_value(&mut ctx, &runtime, legacy),
        direct
    );
    assert_eq!(
        class_slots_builtin_value(&mut ctx, &runtime, extended),
        effective
    );
}

fn class_slots_builtin_value(ctx: &mut ThreadContext, runtime: &Runtime, class: Word) -> Word {
    let arguments = [class];
    let args = BuiltinArgs::new(&arguments);
    let mut values = MultipleValues::new();
    class_slots_builtin(ctx, runtime, &args, &mut values).expect("class slots")
}

#[test]
fn location_and_instance_checks_return_type_errors_for_invalid_values() {
    let (runtime, mut ctx) = setup();
    let malformed_slot =
        make_simple_vector(&mut ctx, &runtime, &[Word::NIL, Word::NIL]).expect("slot");
    assert_eq!(
        location_arg(&ctx, malformed_slot),
        Err(ObjectError::TypeError)
    );
    assert_eq!(instance_arg(&ctx, Word::NIL), Err(ObjectError::TypeError));
}

#[test]
fn every_mop_callback_rejects_missing_required_arguments() {
    let (runtime, mut ctx) = setup();
    let args = BuiltinArgs::new(&[]);
    let mut values = MultipleValues::new();

    for descriptor in builtin_descriptors() {
        assert_eq!(
            (descriptor.callback)(&mut ctx, &runtime, &args, &mut values),
            Err(ObjectError::TypeError),
            "{} requires its declared arguments",
            descriptor.name.as_str()
        );
    }
}

#[test]
fn class_precedence_list_rejects_a_non_vector_descriptor() {
    let (runtime, mut ctx) = setup();

    assert_eq!(
        class_precedence_list(&mut ctx, &runtime, Word::NIL),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn slot_callbacks_cover_bound_unbound_mutation_and_eql_payload() {
    let (runtime, mut ctx) = setup();
    let class = make_simple_vector(
        &mut ctx,
        &runtime,
        &[Word::fixnum(1), Word::NIL, Word::NIL, Word::NIL, Word::NIL],
    )
    .expect("class");
    let location = Fixnum::try_from_word(Word::fixnum(0)).expect("fixnum location");
    let slot =
        make_slot_descriptor(&mut ctx, &runtime, Word::fixnum(2), Some(location)).expect("slot");
    let eql = make_eql_specializer(&mut ctx, &runtime, Word::fixnum(3)).expect("eql");
    let instance =
        ncl_object::make_instance(&mut ctx, &runtime, class, &[Word::UNBOUND]).expect("instance");
    let arguments = [class, instance.as_word(), slot];
    let args = BuiltinArgs::new(&arguments);
    let mut values = MultipleValues::new();

    assert_eq!(
        slot_definition_name_builtin(&mut ctx, &runtime, &BuiltinArgs::new(&[slot]), &mut values),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        slot_definition_location_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[slot]),
            &mut values
        ),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        slot_value_using_class_builtin(&mut ctx, &runtime, &args, &mut values),
        Ok(Word::UNBOUND)
    );
    assert_eq!(
        slot_boundp_using_class_builtin(&mut ctx, &runtime, &args, &mut values),
        Ok(Word::NIL)
    );
    slot_set(&mut ctx, instance, 0, Word::TRUE).expect("set slot");
    assert_eq!(
        slot_boundp_using_class_builtin(&mut ctx, &runtime, &args, &mut values),
        Ok(Word::TRUE)
    );
    assert_eq!(
        slot_makunbound_using_class_builtin(&mut ctx, &runtime, &args, &mut values),
        Ok(instance.as_word())
    );
    assert_eq!(
        eql_specializer_object_builtin(&mut ctx, &runtime, &BuiltinArgs::new(&[eql]), &mut values),
        Ok(Word::fixnum(3))
    );
}
