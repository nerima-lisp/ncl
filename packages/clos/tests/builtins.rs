#![allow(missing_docs)]
#![allow(clippy::unwrap_used, reason = "tests assert on builtin registration")]

use ncl_object::{
    BuiltinConvention, FunctionObject, Package, Runtime, ThreadContext, Word, make_cons,
    make_simple_vector, make_string,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_clos::register(&runtime).unwrap();
    (runtime, ctx)
}

fn function(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap()
}

#[test]
fn class_of_and_class_name_are_bound_builtins() {
    let (runtime, mut ctx) = setup();
    let class_of = function(&runtime, &mut ctx, "CLASS-OF");
    let integer = runtime.class(&mut ctx, "INTEGER").unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, class_of, &[Word::fixnum(7)]),
        Ok(integer)
    );

    let class_name = function(&runtime, &mut ctx, "CLASS-NAME");
    let name = runtime
        .call_builtin(&mut ctx, class_name, &[integer])
        .unwrap();
    assert_ne!(name, Word::UNBOUND);
}

#[test]
fn class_of_reports_builtin_object_families() {
    let (runtime, mut ctx) = setup();
    let class_of = function(&runtime, &mut ctx, "CLASS-OF");
    let package = runtime.find_package(&ctx, "COMMON-LISP").unwrap();
    let symbol = Package::from_word(package)
        .intern(&mut ctx, &runtime, "CLOS-COVERAGE-SYMBOL")
        .unwrap()
        .0;
    let string = make_string(&mut ctx, &runtime, &['x']).unwrap();
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::NIL]).unwrap();
    let cons = make_cons(&mut ctx, &runtime, Word::TRUE, Word::NIL).unwrap();
    let function_word = function(&runtime, &mut ctx, "CLASS-OF").as_word();
    for (object, class_name) in [
        (Word::NIL, "NULL"),
        (Word::character(u32::from('x')), "CHARACTER"),
        (symbol, "SYMBOL"),
        (string, "STRING"),
        (vector, "SIMPLE-VECTOR"),
        (cons, "CONS"),
        (function_word, "FUNCTION"),
        (package, "PACKAGE"),
    ] {
        assert_eq!(
            runtime.call_builtin(&mut ctx, class_of, &[object]),
            Ok(runtime.class(&mut ctx, class_name).unwrap()),
            "class-of must identify {class_name}"
        );
    }
}

#[test]
fn find_class_and_typep_accept_symbol_and_descriptor_designators() {
    let (runtime, mut ctx) = setup();
    let package = runtime.find_package(&ctx, "COMMON-LISP").unwrap();
    let integer_symbol = Package::from_word(package)
        .intern(&mut ctx, &runtime, "INTEGER")
        .unwrap()
        .0;
    let integer = runtime.class(&mut ctx, "INTEGER").unwrap();
    let find_class = function(&runtime, &mut ctx, "FIND-CLASS");
    assert_eq!(
        runtime.call_builtin(&mut ctx, find_class, &[integer_symbol]),
        Ok(integer)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, find_class, &[integer]),
        Ok(integer)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, find_class, &[Word::fixnum(99)]),
        Err(ncl_object::ObjectError::TypeError)
    );

    let typep = function(&runtime, &mut ctx, "TYPEP");
    assert_eq!(
        runtime.call_builtin(&mut ctx, typep, &[Word::fixnum(7), integer_symbol]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, typep, &[Word::character(u32::from('x')), integer]),
        Ok(Word::NIL)
    );
}

#[test]
fn class_builtins_reject_unknown_designators_and_invalid_class_values() {
    let (runtime, mut ctx) = setup();
    let package = runtime.find_package(&ctx, "COMMON-LISP").unwrap();
    let unknown = Package::from_word(package)
        .intern(&mut ctx, &runtime, "CLOS-UNKNOWN-CLASS")
        .unwrap()
        .0;
    let find_class = function(&runtime, &mut ctx, "FIND-CLASS");
    assert_eq!(
        runtime.call_builtin(&mut ctx, find_class, &[unknown]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, find_class, &[Word::fixnum(1)]),
        Err(ncl_object::ObjectError::TypeError)
    );

    let class_name = function(&runtime, &mut ctx, "CLASS-NAME");
    assert_eq!(
        runtime.call_builtin(&mut ctx, class_name, &[Word::NIL]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn typep_follows_user_class_inheritance_and_rejects_bad_type_designators() {
    let (runtime, mut ctx) = setup();
    let parent_name = make_string(&mut ctx, &runtime, &['P']).unwrap();
    let child_name = make_string(&mut ctx, &runtime, &['C']).unwrap();
    let other_name = make_string(&mut ctx, &runtime, &['O']).unwrap();
    let standard_object = runtime.class(&mut ctx, "STANDARD-OBJECT").unwrap();
    let parent = ncl_clos::make_class(
        &mut ctx,
        &runtime,
        parent_name,
        standard_object,
        Word::NIL,
        Word::fixnum(0),
    )
    .unwrap();
    let child = ncl_clos::make_class(
        &mut ctx,
        &runtime,
        child_name,
        parent,
        Word::NIL,
        Word::fixnum(0),
    )
    .unwrap();
    let other = ncl_clos::make_class(
        &mut ctx,
        &runtime,
        other_name,
        standard_object,
        Word::NIL,
        Word::fixnum(0),
    )
    .unwrap();
    let instance = ncl_clos::make_instance(&mut ctx, &runtime, child, &[]).unwrap();
    let typep = function(&runtime, &mut ctx, "TYPEP");

    assert_eq!(
        runtime.call_builtin(&mut ctx, typep, &[instance, parent]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, typep, &[instance, other]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, typep, &[Word::fixnum(3), child]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, typep, &[instance, Word::fixnum(3)]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn named_slot_builtins_cover_inheritance_boundness_and_unknown_names() {
    let (runtime, mut ctx) = setup();
    let package = runtime.find_package(&ctx, "COMMON-LISP").unwrap();
    let parent_slot_name = Package::from_word(package)
        .intern(&mut ctx, &runtime, "CLOS-PARENT-SLOT")
        .unwrap()
        .0;
    let child_slot_name = Package::from_word(package)
        .intern(&mut ctx, &runtime, "CLOS-CHILD-SLOT")
        .unwrap()
        .0;
    let unknown_slot_name = Package::from_word(package)
        .intern(&mut ctx, &runtime, "CLOS-UNKNOWN-SLOT")
        .unwrap()
        .0;
    let parent_slot =
        make_simple_vector(&mut ctx, &runtime, &[parent_slot_name, Word::fixnum(0)]).unwrap();
    let child_slot =
        make_simple_vector(&mut ctx, &runtime, &[child_slot_name, Word::fixnum(1)]).unwrap();
    let standard_object = runtime.class(&mut ctx, "STANDARD-OBJECT").unwrap();
    let parent_slots = make_simple_vector(&mut ctx, &runtime, &[parent_slot]).unwrap();
    let parent = ncl_clos::make_class(
        &mut ctx,
        &runtime,
        Word::NIL,
        standard_object,
        parent_slots,
        Word::fixnum(0),
    )
    .unwrap();
    let child_slots = make_simple_vector(&mut ctx, &runtime, &[child_slot]).unwrap();
    let child = ncl_clos::make_class(
        &mut ctx,
        &runtime,
        Word::NIL,
        parent,
        child_slots,
        Word::fixnum(0),
    )
    .unwrap();
    let instance =
        ncl_clos::make_instance(&mut ctx, &runtime, child, &[Word::UNBOUND, Word::UNBOUND])
            .unwrap();
    let slot_boundp = function(&runtime, &mut ctx, "SLOT-BOUNDP");
    let slot_makunbound = function(&runtime, &mut ctx, "SLOT-MAKUNBOUND");
    let slot_value = function(&runtime, &mut ctx, "SLOT-VALUE");
    let slot_set = function(&runtime, &mut ctx, "SLOT-VALUE-SET");

    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_boundp, &[instance, parent_slot_name]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            slot_set,
            &[instance, parent_slot_name, Word::TRUE],
        ),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_value, &[instance, parent_slot_name]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_boundp, &[instance, parent_slot_name]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_makunbound, &[instance, parent_slot_name]),
        Ok(instance)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_boundp, &[instance, parent_slot_name]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            slot_set,
            &[Word::NIL, child_slot_name, Word::TRUE]
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_value, &[instance, unknown_slot_name]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn slot_builtins_round_trip_and_reject_non_instances() {
    let (runtime, mut ctx) = setup();
    let class = runtime.class(&mut ctx, "STANDARD-OBJECT").unwrap();
    let instance = ncl_clos::make_instance(&mut ctx, &runtime, class, &[Word::UNBOUND]).unwrap();
    let slot_value = function(&runtime, &mut ctx, "SLOT-VALUE");
    let slot_set = function(&runtime, &mut ctx, "SLOT-VALUE-SET");
    assert_eq!(
        runtime.builtin_descriptor(slot_set).unwrap().convention,
        BuiltinConvention::Direct(ncl_object::Arity::exact(3))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_set, &[instance, Word::fixnum(0), Word::TRUE]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_value, &[instance, Word::fixnum(0)]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_value, &[Word::NIL, Word::fixnum(0)]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn slot_exists_p_recognizes_direct_and_inherited_slots() {
    let (runtime, mut ctx) = setup();
    let inherited_name = Word::fixnum(17);
    let direct_name = Word::fixnum(23);
    let inherited_slot =
        ncl_object::make_simple_vector(&mut ctx, &runtime, &[inherited_name, Word::fixnum(0)])
            .unwrap();
    let parent_slots =
        ncl_object::make_simple_vector(&mut ctx, &runtime, &[inherited_slot]).unwrap();
    let parent = ncl_clos::make_class(
        &mut ctx,
        &runtime,
        Word::NIL,
        Word::NIL,
        parent_slots,
        Word::fixnum(0),
    )
    .unwrap();
    let child_slots = ncl_object::make_simple_vector(&mut ctx, &runtime, &[direct_name]).unwrap();
    let child = ncl_clos::make_class(
        &mut ctx,
        &runtime,
        Word::NIL,
        parent,
        child_slots,
        Word::fixnum(0),
    )
    .unwrap();
    let instance = ncl_clos::make_instance(&mut ctx, &runtime, child, &[]).unwrap();
    let slot_exists = function(&runtime, &mut ctx, "SLOT-EXISTS-P");

    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_exists, &[instance, inherited_name]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_exists, &[instance, direct_name]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_exists, &[instance, Word::fixnum(99)]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_exists, &[Word::NIL, inherited_name]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn production_table_contains_only_bound_builtins() {
    let (runtime, mut ctx) = setup();
    let production = ncl_clos::production_function_names();
    assert!(!production.is_empty());
    for identifier in production {
        let word = runtime
            .function(
                &mut ctx,
                identifier.package.as_str(),
                identifier.name.as_str(),
            )
            .unwrap_or_else(|| panic!("production builtin is not registered: {identifier:?}"));
        let function = FunctionObject::try_from(word).unwrap_or_else(|_| {
            panic!("production builtin is not a FunctionObject: {identifier:?}")
        });
        assert!(
            runtime.builtin_descriptor(function).is_some(),
            "production builtin has no descriptor: {identifier:?}"
        );
    }
}
