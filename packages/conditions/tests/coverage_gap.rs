#![allow(
    clippy::unwrap_used,
    reason = "tests assert on condition-system behavior"
)]

//! Assertions for condition-system paths not covered by the core behavior tests.

use ncl_conditions::{
    ConditionError, ConditionIdentifier, condition_class, condition_class_name, condition_class_of,
    condition_from_lisp_error, condition_report, find_restart, make_condition, pop_restart,
    push_restart, signal,
};
use ncl_object::{
    ArithmeticError, FunctionObject, LispError, ObjectType, Package, ProgramError, Runtime,
    ThreadContext, Word, make_cons, make_string, slot_ref, string_length, string_ref,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_conditions::register(&runtime).unwrap();
    (runtime, ctx)
}

fn symbol(ctx: &mut ThreadContext, runtime: &Runtime, package: &str, name: &str) -> Word {
    let package = runtime.ensure_package(ctx, package).unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn builtin(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    package: &str,
    name: &str,
) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, package, name).unwrap()).unwrap()
}

fn class_name(ctx: &ThreadContext, class: ncl_conditions::ConditionClass) -> String {
    let name = condition_class_name(ctx, class).unwrap();
    (0..string_length(ctx, name).unwrap())
        .map(|index| string_ref(ctx, name, index).unwrap())
        .collect()
}

#[test]
fn define_condition_class_instantiates_slots_and_reports() {
    let (runtime, mut ctx) = setup();
    let name = symbol(&mut ctx, &runtime, "NCL-TEST", "COVERAGE-CONDITION");
    let initarg = symbol(&mut ctx, &runtime, "KEYWORD", "VALUE");
    let slot = make_cons(&mut ctx, &runtime, initarg, Word::NIL).unwrap();
    let slot_specs = make_cons(&mut ctx, &runtime, slot, Word::NIL).unwrap();
    let define = builtin(&runtime, &mut ctx, "NCL-EXT", "DEFINE-CONDITION-CLASS");
    let report = make_string(
        &mut ctx,
        &runtime,
        &"defined condition".chars().collect::<Vec<_>>(),
    )
    .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, define, &[name, Word::NIL, slot_specs, report],),
        Ok(name)
    );

    let make_condition = builtin(&runtime, &mut ctx, "COMMON-LISP", "MAKE-CONDITION");
    let value = Word::fixnum(17);
    let instance = runtime
        .call_builtin(&mut ctx, make_condition, &[name, initarg, value])
        .unwrap();
    let slot_ref = builtin(&runtime, &mut ctx, "NCL-EXT", "CONDITION-SLOT-REF");
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_ref, &[instance, Word::fixnum(0)]),
        Ok(value)
    );
    assert_eq!(
        condition_report(&ctx, instance).as_deref(),
        Some("defined condition")
    );
}

#[test]
fn slot_metadata_uses_initform_when_initarg_is_absent() {
    let (runtime, mut ctx) = setup();
    let type_name = symbol(&mut ctx, &runtime, "COMMON-LISP", "SIMPLE-CONDITION");
    let format_control = symbol(&mut ctx, &runtime, "KEYWORD", "FORMAT-CONTROL");
    let format = make_string(&mut ctx, &runtime, &"value ~a".chars().collect::<Vec<_>>()).unwrap();
    let make_condition = builtin(&runtime, &mut ctx, "COMMON-LISP", "MAKE-CONDITION");
    let instance = runtime
        .call_builtin(
            &mut ctx,
            make_condition,
            &[type_name, format_control, format],
        )
        .unwrap();
    assert_eq!(
        slot_ref(&ctx, ncl_object::Instance::from_word(instance), 1).unwrap(),
        Word::NIL
    );
    assert_eq!(
        condition_report(&ctx, instance).as_deref(),
        Some("value #<OBJECT>")
    );
}

#[test]
fn condition_reports_cover_simple_type_and_generic_classes() {
    let (runtime, mut ctx) = setup();
    let simple = condition_class(&mut ctx, &runtime, "SIMPLE-CONDITION").unwrap();
    let control = make_string(&mut ctx, &runtime, &"value ~a".chars().collect::<Vec<_>>()).unwrap();
    let arguments = make_cons(&mut ctx, &runtime, Word::fixnum(9), Word::NIL).unwrap();
    let simple_instance =
        make_condition(&mut ctx, &runtime, simple, &[control, arguments]).unwrap();
    assert_eq!(
        condition_report(&ctx, simple_instance).as_deref(),
        Some("value 9")
    );

    let type_error = condition_class(&mut ctx, &runtime, "TYPE-ERROR").unwrap();
    let datum = make_string(&mut ctx, &runtime, &"wrong".chars().collect::<Vec<_>>()).unwrap();
    let expected = symbol(&mut ctx, &runtime, "COMMON-LISP", "FIXNUM");
    let type_instance = make_condition(&mut ctx, &runtime, type_error, &[datum, expected]).unwrap();
    assert_eq!(
        condition_report(&ctx, type_instance).as_deref(),
        Some("The value wrong is not of type FIXNUM.")
    );

    let generic = condition_class(&mut ctx, &runtime, "PROGRAM-ERROR").unwrap();
    let generic_instance = make_condition(&mut ctx, &runtime, generic, &[]).unwrap();
    assert_eq!(
        condition_report(&ctx, generic_instance).as_deref(),
        Some("PROGRAM-ERROR condition")
    );
}

#[test]
fn converted_type_error_keeps_its_report_payload() {
    let (runtime, mut ctx) = setup();
    let condition = condition_from_lisp_error(
        &mut ctx,
        &runtime,
        LispError::TypeError {
            datum: Word::fixnum(7),
            expected: ObjectType::String,
        },
    )
    .unwrap();

    assert_eq!(
        class_name(&ctx, condition_class_of(&ctx, condition).unwrap()),
        "TYPE-ERROR"
    );
    assert_eq!(
        condition_report(&ctx, condition).as_deref(),
        Some("The value 7 is not of type STRING.")
    );
}

#[test]
fn condition_matching_rejects_same_length_near_miss_names() {
    let (runtime, mut ctx) = setup();
    let define = builtin(&runtime, &mut ctx, "NCL-EXT", "DEFINE-CONDITION-CLASS");
    let class_name = symbol(&mut ctx, &runtime, "NCL-TEST", "WARNINX");
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            define,
            &[class_name, Word::NIL, Word::NIL, Word::NIL],
        ),
        Ok(class_name)
    );
    let class = condition_class(&mut ctx, &runtime, "WARNINX").unwrap();
    let condition = make_condition(&mut ctx, &runtime, class, &[]).unwrap();

    assert_eq!(signal(&mut ctx, condition), Err(ConditionError::Unhandled));
}

#[test]
fn restart_matching_rejects_same_length_near_miss_names() {
    let (runtime, mut ctx) = setup();
    let active_name = make_string(
        &mut ctx,
        &runtime,
        &"SAME-LENGTH-A".chars().collect::<Vec<_>>(),
    )
    .unwrap();
    let near_miss = make_string(
        &mut ctx,
        &runtime,
        &"SAME-LENGTH-B".chars().collect::<Vec<_>>(),
    )
    .unwrap();
    let restart = push_restart(
        &mut ctx,
        &runtime,
        active_name,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap();

    assert_eq!(find_restart(&ctx, near_miss), Ok(None));
    pop_restart(&mut ctx, restart);
}

#[test]
fn simple_condition_report_preserves_directives_with_invalid_arguments() {
    let (runtime, mut ctx) = setup();
    let class = condition_class(&mut ctx, &runtime, "SIMPLE-CONDITION").unwrap();
    let control = make_string(
        &mut ctx,
        &runtime,
        &"bad ~a ~A ~x".chars().collect::<Vec<_>>(),
    )
    .unwrap();
    let condition = make_condition(&mut ctx, &runtime, class, &[control, Word::fixnum(1)]).unwrap();

    assert_eq!(
        condition_report(&ctx, condition).as_deref(),
        Some("bad ~a ~A ~x")
    );
}

#[test]
fn lisp_errors_convert_to_specific_condition_classes() {
    let (runtime, mut ctx) = setup();
    let cases = [
        (
            LispError::ProgramError(ProgramError::WrongNumberOfArguments {
                minimum: 2,
                maximum: Some(4),
            }),
            ConditionIdentifier::ProgramError,
        ),
        (
            LispError::ArithmeticError(ArithmeticError::DivisionByZero),
            ConditionIdentifier::DivisionByZero,
        ),
        (
            LispError::Object(ncl_object::ObjectError::TypeError),
            ConditionIdentifier::TypeError,
        ),
        (LispError::EndOfFile, ConditionIdentifier::EndOfFile),
    ];
    for (error, expected) in cases {
        let condition = condition_from_lisp_error(&mut ctx, &runtime, error).unwrap();
        let class = ncl_conditions::condition_class_of(&ctx, condition).unwrap();
        assert_eq!(class_name(&ctx, class), expected.name());
    }
}

#[test]
fn condition_identifier_resolves_to_registered_hierarchy_classes() {
    let (runtime, mut ctx) = setup();
    let identifiers = [
        (ConditionIdentifier::TypeError, "TYPE-ERROR"),
        (ConditionIdentifier::SimpleWarning, "SIMPLE-WARNING"),
        (ConditionIdentifier::ThreadDeadlock, "THREAD-DEADLOCK"),
    ];
    for (identifier, expected_name) in identifiers {
        let class = identifier.class(&mut ctx, &runtime).unwrap();
        assert_eq!(class_name(&ctx, class), expected_name);
        let condition = make_condition(&mut ctx, &runtime, class, &[]).unwrap();
        assert_eq!(condition_class_of(&ctx, condition).unwrap(), class);
    }
}
