#![allow(missing_docs)]
#![allow(
    clippy::unwrap_used,
    reason = "tests assert on initialization protocol"
)]

use ncl_object::{
    FunctionObject, Instance, Package, Runtime, ThreadContext, Word, make_simple_vector, pop_root,
    push_root, slot_ref, slot_set,
};

#[path = "../src/initialization.rs"]
mod initialization;

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_clos::register(&runtime).unwrap();
    initialization::register(&runtime).unwrap();
    (runtime, ctx)
}

fn function(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    package: &str,
    name: &str,
) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, package, name).unwrap()).unwrap()
}

fn class_with_slots(ctx: &mut ThreadContext, runtime: &Runtime, keys: &[Word]) -> Word {
    let slots = make_simple_vector(ctx, runtime, keys).unwrap();
    ncl_clos::make_class(ctx, runtime, Word::NIL, Word::NIL, slots, Word::fixnum(0)).unwrap()
}

#[test]
fn make_instance_applies_initargs_and_initialize_instance_returns_instance() {
    let (runtime, mut ctx) = setup();
    let key = Word::fixnum(17);
    let class = class_with_slots(&mut ctx, &runtime, &[key]);
    let make = function(&runtime, &mut ctx, "COMMON-LISP", "MAKE-INSTANCE");
    let instance = runtime
        .call_builtin(&mut ctx, make, &[class, key, Word::TRUE])
        .unwrap();
    let slot_value = function(&runtime, &mut ctx, "COMMON-LISP", "SLOT-VALUE");
    assert_eq!(
        runtime
            .call_builtin(&mut ctx, slot_value, &[instance, Word::fixnum(0)])
            .unwrap(),
        Word::TRUE
    );

    let initialize = function(&runtime, &mut ctx, "COMMON-LISP", "INITIALIZE-INSTANCE");
    let result = runtime
        .call_builtin(&mut ctx, initialize, &[instance, key, Word::NIL])
        .unwrap();
    assert_eq!(result, instance);
    assert_eq!(
        runtime
            .call_builtin(&mut ctx, slot_value, &[instance, Word::fixnum(0)])
            .unwrap(),
        Word::NIL
    );
}

#[test]
fn initialization_paths_survive_gc_stress_and_strict_forwarding() {
    let (runtime, mut ctx) = setup();
    let key = Word::fixnum(71);
    let mut class = class_with_slots(&mut ctx, &runtime, &[key]);
    let class_token = push_root(&mut ctx, &mut class);
    let make = function(&runtime, &mut ctx, "COMMON-LISP", "MAKE-INSTANCE");
    let mut make_word = make.as_word();
    let make_token = push_root(&mut ctx, &mut make_word);
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);

    let instance = runtime
        .call_builtin(
            &mut ctx,
            FunctionObject::try_from(make_word).unwrap(),
            &[class, key, Word::TRUE],
        )
        .unwrap();
    let mut instance = instance;
    let instance_token = push_root(&mut ctx, &mut instance);
    assert_eq!(
        slot_ref(&ctx, Instance::from_word(instance), 0),
        Ok(Word::TRUE)
    );
    slot_set(&mut ctx, Instance::from_word(instance), 0, Word::NIL).unwrap();
    assert_eq!(
        slot_ref(&ctx, Instance::from_word(instance), 0),
        Ok(Word::NIL)
    );
    assert!(pop_root(&mut ctx, instance_token));
    assert!(pop_root(&mut ctx, make_token));
    assert!(pop_root(&mut ctx, class_token));
}

#[test]
fn make_instance_symbol_initarg_survives_gc_stress_and_strict_forwarding() {
    let (runtime, mut ctx) = setup();
    let package = runtime.find_package(&ctx, "KEYWORD").unwrap();
    let (mut key, _) = Package::from_word(package)
        .intern(&mut ctx, &runtime, "N26-SYMBOL-INITARG")
        .unwrap();
    let key_token = push_root(&mut ctx, &mut key);
    let mut class = class_with_slots(&mut ctx, &runtime, &[key]);
    let class_token = push_root(&mut ctx, &mut class);
    let make = function(&runtime, &mut ctx, "COMMON-LISP", "MAKE-INSTANCE");
    let mut make_word = make.as_word();
    let make_token = push_root(&mut ctx, &mut make_word);
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);

    let mut instance = runtime
        .call_builtin(
            &mut ctx,
            FunctionObject::try_from(make_word).unwrap(),
            &[class, key, Word::TRUE],
        )
        .unwrap();
    let instance_token = push_root(&mut ctx, &mut instance);
    assert_eq!(
        slot_ref(&ctx, Instance::from_word(instance), 0),
        Ok(Word::TRUE)
    );

    assert!(pop_root(&mut ctx, instance_token));
    assert!(pop_root(&mut ctx, make_token));
    assert!(pop_root(&mut ctx, class_token));
    assert!(pop_root(&mut ctx, key_token));
}

#[test]
fn shared_initialize_survives_gc_stress_and_strict_forwarding() {
    let (runtime, mut ctx) = setup();
    let key = Word::fixnum(72);
    let class = class_with_slots(&mut ctx, &runtime, &[key]);
    let instance = ncl_clos::make_instance(&mut ctx, &runtime, class, &[Word::UNBOUND]).unwrap();
    let shared = function(&runtime, &mut ctx, "COMMON-LISP", "SHARED-INITIALIZE");
    let mut instance_word = instance;
    let instance_token = push_root(&mut ctx, &mut instance_word);
    let mut shared_word = shared.as_word();
    let shared_token = push_root(&mut ctx, &mut shared_word);
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    assert_eq!(
        runtime
            .call_builtin(
                &mut ctx,
                FunctionObject::try_from(shared_word).unwrap(),
                &[instance_word, key, Word::TRUE],
            )
            .unwrap(),
        instance_word
    );
    assert_eq!(
        slot_ref(&ctx, Instance::from_word(instance_word), 0),
        Ok(Word::TRUE)
    );
    assert!(pop_root(&mut ctx, shared_token));
    assert!(pop_root(&mut ctx, instance_token));
}

#[test]
fn class_definition_survives_gc_stress_and_strict_forwarding() {
    let (runtime, mut ctx) = setup();
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    let class = class_with_slots(&mut ctx, &runtime, &[Word::fixnum(73)]);
    let mut class_word = class;
    let class_token = push_root(&mut ctx, &mut class_word);
    let instance =
        ncl_clos::make_instance(&mut ctx, &runtime, class_word, &[Word::UNBOUND]).unwrap();
    assert_eq!(
        slot_ref(&ctx, Instance::from_word(instance), 0),
        Ok(Word::UNBOUND)
    );
    assert!(pop_root(&mut ctx, class_token));
}

#[test]
fn shared_initialize_rejects_odd_initargs_and_non_instance() {
    let (runtime, mut ctx) = setup();
    let shared = function(&runtime, &mut ctx, "COMMON-LISP", "SHARED-INITIALIZE");
    assert_eq!(
        runtime.call_builtin(&mut ctx, shared, &[Word::NIL, Word::fixnum(1)]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, shared, &[Word::NIL]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn make_instance_reports_undefined_initialize_instance_function_cell() {
    let (runtime, mut ctx) = setup();
    let common_lisp = runtime.find_package(&ctx, "COMMON-LISP").unwrap();
    let (initialize_name, _) = Package::from_word(common_lisp)
        .intern(&mut ctx, &runtime, "INITIALIZE-INSTANCE")
        .unwrap();
    ctx.write_object_slot(
        initialize_name,
        ncl_object::symbol_offset::FUNCTION,
        Word::UNBOUND,
    )
    .unwrap();
    let class = class_with_slots(&mut ctx, &runtime, &[Word::fixnum(111)]);
    let make = function(&runtime, &mut ctx, "COMMON-LISP", "MAKE-INSTANCE");

    assert_eq!(
        runtime.call_builtin(&mut ctx, make, &[class]),
        Err(ncl_object::ObjectError::UndefinedFunction)
    );
}

#[test]
fn initialize_instance_reports_undefined_shared_initialize_function_cell() {
    let (runtime, mut ctx) = setup();
    let common_lisp = runtime.find_package(&ctx, "COMMON-LISP").unwrap();
    let (shared_name, _) = Package::from_word(common_lisp)
        .intern(&mut ctx, &runtime, "SHARED-INITIALIZE")
        .unwrap();
    ctx.write_object_slot(
        shared_name,
        ncl_object::symbol_offset::FUNCTION,
        Word::UNBOUND,
    )
    .unwrap();
    let class = class_with_slots(&mut ctx, &runtime, &[Word::fixnum(112)]);
    let instance = ncl_clos::make_instance(&mut ctx, &runtime, class, &[Word::UNBOUND]).unwrap();
    let initialize = function(&runtime, &mut ctx, "COMMON-LISP", "INITIALIZE-INSTANCE");

    assert_eq!(
        runtime.call_builtin(&mut ctx, initialize, &[instance]),
        Err(ncl_object::ObjectError::UndefinedFunction)
    );
}

#[test]
fn initialize_slots_prefers_initarg_over_default_and_preserves_unbound_default() {
    let (runtime, mut ctx) = setup();
    let first_name = Word::fixnum(121);
    let first_initarg = Word::fixnum(122);
    let second_name = Word::fixnum(123);
    let second_initarg = Word::fixnum(124);
    let first = make_simple_vector(
        &mut ctx,
        &runtime,
        &[first_name, first_initarg, Word::fixnum(900)],
    )
    .unwrap();
    let second = make_simple_vector(
        &mut ctx,
        &runtime,
        &[second_name, second_initarg, Word::UNBOUND],
    )
    .unwrap();
    let slots = make_simple_vector(&mut ctx, &runtime, &[first, second]).unwrap();
    let class = ncl_clos::make_class(
        &mut ctx,
        &runtime,
        Word::fixnum(125),
        Word::NIL,
        slots,
        Word::fixnum(0),
    )
    .unwrap();
    let make = function(&runtime, &mut ctx, "COMMON-LISP", "MAKE-INSTANCE");
    let instance = runtime
        .call_builtin(&mut ctx, make, &[class, first_initarg, Word::fixnum(901)])
        .unwrap();

    assert_eq!(
        slot_ref(&ctx, Instance::from_word(instance), 0),
        Ok(Word::fixnum(901))
    );
    assert_eq!(
        slot_ref(&ctx, Instance::from_word(instance), 1),
        Ok(Word::UNBOUND)
    );
}

#[test]
fn make_instance_initializes_slots_inherited_through_three_generations() {
    let (runtime, mut ctx) = setup();
    let root_slot = Word::fixnum(101);
    let middle_slot = Word::fixnum(102);
    let leaf_slot = Word::fixnum(103);
    let root = class_with_slots(&mut ctx, &runtime, &[root_slot]);
    let middle_slots = make_simple_vector(&mut ctx, &runtime, &[middle_slot]).unwrap();
    let middle = ncl_clos::make_class(
        &mut ctx,
        &runtime,
        Word::fixnum(201),
        root,
        middle_slots,
        Word::fixnum(0),
    )
    .unwrap();
    let leaf_slots = make_simple_vector(&mut ctx, &runtime, &[leaf_slot]).unwrap();
    let leaf = ncl_clos::make_class(
        &mut ctx,
        &runtime,
        Word::fixnum(202),
        middle,
        leaf_slots,
        Word::fixnum(0),
    )
    .unwrap();
    let make = function(&runtime, &mut ctx, "COMMON-LISP", "MAKE-INSTANCE");
    let instance = runtime
        .call_builtin(
            &mut ctx,
            make,
            &[
                leaf,
                root_slot,
                Word::fixnum(11),
                middle_slot,
                Word::fixnum(22),
                leaf_slot,
                Word::fixnum(33),
            ],
        )
        .unwrap();
    let slot_value = function(&runtime, &mut ctx, "COMMON-LISP", "SLOT-VALUE");
    assert_eq!(
        runtime
            .call_builtin(&mut ctx, slot_value, &[instance, Word::fixnum(0)])
            .unwrap(),
        Word::fixnum(11)
    );
    assert_eq!(
        runtime
            .call_builtin(&mut ctx, slot_value, &[instance, Word::fixnum(1)])
            .unwrap(),
        Word::fixnum(22)
    );
    assert_eq!(
        runtime
            .call_builtin(&mut ctx, slot_value, &[instance, Word::fixnum(2)])
            .unwrap(),
        Word::fixnum(33)
    );
}

#[test]
fn make_instance_resolves_symbol_classes_and_applies_slot_defaults() {
    let (runtime, mut ctx) = setup();
    let slot_name = Word::fixnum(301);
    let initarg = Word::fixnum(302);
    let slot =
        make_simple_vector(&mut ctx, &runtime, &[slot_name, initarg, Word::fixnum(303)]).unwrap();
    let slots = make_simple_vector(&mut ctx, &runtime, &[slot]).unwrap();
    let class = ncl_clos::make_class(
        &mut ctx,
        &runtime,
        Word::fixnum(304),
        Word::NIL,
        slots,
        Word::fixnum(0),
    )
    .unwrap();
    runtime
        .define_class(&mut ctx, "N26-DEFAULT-CLASS", class)
        .unwrap();
    let package = runtime.find_package(&ctx, "COMMON-LISP-USER").unwrap();
    let class_symbol = Package::from_word(package)
        .intern(&mut ctx, &runtime, "N26-DEFAULT-CLASS")
        .unwrap()
        .0;
    let make = function(&runtime, &mut ctx, "COMMON-LISP", "MAKE-INSTANCE");
    let slot_value = function(&runtime, &mut ctx, "COMMON-LISP", "SLOT-VALUE");

    let default_instance = runtime
        .call_builtin(&mut ctx, make, &[class_symbol])
        .unwrap();
    assert_eq!(
        runtime
            .call_builtin(&mut ctx, slot_value, &[default_instance, Word::fixnum(0)])
            .unwrap(),
        Word::fixnum(303)
    );

    let explicit_instance = runtime
        .call_builtin(&mut ctx, make, &[class_symbol, initarg, Word::fixnum(305)])
        .unwrap();
    assert_eq!(
        runtime
            .call_builtin(&mut ctx, slot_value, &[explicit_instance, Word::fixnum(0)])
            .unwrap(),
        Word::fixnum(305)
    );
}

#[test]
fn make_instance_accepts_registered_class_vector_designator() {
    let (runtime, mut ctx) = setup();
    let common_lisp = runtime.find_package(&ctx, "COMMON-LISP").unwrap();
    let (class_name, _) = Package::from_word(common_lisp)
        .intern(&mut ctx, &runtime, "STANDARD-OBJECT")
        .unwrap();
    let make = function(&runtime, &mut ctx, "COMMON-LISP", "MAKE-INSTANCE");

    let symbol_instance = runtime.call_builtin(&mut ctx, make, &[class_name]).unwrap();
    assert!(matches!(
        ncl_object::classify_object(&ctx, symbol_instance),
        ncl_object::ObjectRef::Instance(_)
    ));

    let key = Word::fixnum(131);
    let class = class_with_slots(&mut ctx, &runtime, &[key]);
    let vector_instance = runtime
        .call_builtin(&mut ctx, make, &[class, key, Word::TRUE])
        .unwrap();
    assert_eq!(
        slot_ref(&ctx, Instance::from_word(vector_instance), 0),
        Ok(Word::TRUE)
    );
}

#[test]
fn make_instance_rejects_invalid_class_designators() {
    let (runtime, mut ctx) = setup();
    let make = function(&runtime, &mut ctx, "COMMON-LISP", "MAKE-INSTANCE");

    assert_eq!(
        runtime.call_builtin(&mut ctx, make, &[Word::fixnum(132)]),
        Err(ncl_object::ObjectError::TypeError)
    );
    let common_lisp = runtime.find_package(&ctx, "COMMON-LISP").unwrap();
    let (unknown, _) = Package::from_word(common_lisp)
        .intern(&mut ctx, &runtime, "NCL-UNKNOWN-CLASS-DESIGNATOR")
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, make, &[unknown]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn make_instance_rejects_malformed_effective_slot_descriptors() {
    let (runtime, mut ctx) = setup();
    let make = function(&runtime, &mut ctx, "COMMON-LISP", "MAKE-INSTANCE");
    let class = make_simple_vector(
        &mut ctx,
        &runtime,
        &[
            Word::fixnum(134),
            Word::NIL,
            Word::NIL,
            Word::fixnum(0),
            Word::fixnum(133),
        ],
    )
    .unwrap();

    assert_eq!(
        runtime.call_builtin(&mut ctx, make, &[class]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn make_instance_and_initialize_instance_reject_odd_initargs() {
    let (runtime, mut ctx) = setup();
    let class = class_with_slots(&mut ctx, &runtime, &[Word::fixnum(135)]);
    let make = function(&runtime, &mut ctx, "COMMON-LISP", "MAKE-INSTANCE");
    assert_eq!(
        runtime.call_builtin(&mut ctx, make, &[class, Word::fixnum(136)]),
        Err(ncl_object::ObjectError::TypeError)
    );

    let instance = ncl_clos::make_instance(&mut ctx, &runtime, class, &[Word::UNBOUND]).unwrap();
    let initialize = function(&runtime, &mut ctx, "COMMON-LISP", "INITIALIZE-INSTANCE");
    assert_eq!(
        runtime.call_builtin(&mut ctx, initialize, &[instance, Word::fixnum(137)]),
        Err(ncl_object::ObjectError::TypeError)
    );
}
