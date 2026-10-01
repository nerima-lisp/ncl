#![allow(
    clippy::unwrap_used,
    reason = "tests assert on condition-system behavior"
)]

//! Wave 13 matrices for condition classes, slots, restarts, and conversions.

use ncl_conditions::{
    condition_class, condition_class_of, condition_report, find_restart, make_condition,
    pop_handler, pop_restart, push_handler, push_restart, signal_matched,
};
use ncl_object::{
    FunctionObject, LispError, ObjectError, ObjectType, Package, Runtime, ThreadContext, Word,
    make_cons, make_string, slot_ref, string_length, string_ref,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_conditions::register(&runtime).unwrap();
    (runtime, ctx)
}

fn builtin(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    package: &str,
    name: &str,
) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, package, name).unwrap()).unwrap()
}

fn symbol(ctx: &mut ThreadContext, runtime: &Runtime, package: &str, name: &str) -> Word {
    let package = runtime.ensure_package(ctx, package).unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn text(ctx: &ThreadContext, value: Word) -> String {
    (0..string_length(ctx, value).unwrap())
        .map(|index| string_ref(ctx, value, index).unwrap())
        .collect()
}

fn symbol_text(ctx: &ThreadContext, value: Word) -> String {
    text(ctx, ncl_object::symbol_name(ctx, value).unwrap())
}

fn return_marker(
    _runtime: std::ptr::NonNull<()>,
    _ctx: &mut ThreadContext,
    _function: Word,
    _arguments: &[Word],
) -> Result<Word, ObjectError> {
    Ok(Word::fixnum(77))
}

fn class_name(ctx: &ThreadContext, condition: Word) -> String {
    let class = condition_class_of(ctx, condition).unwrap();
    text(
        ctx,
        ncl_conditions::condition_class_name(ctx, class).unwrap(),
    )
}

#[test]
fn conversion_type_error_expected_type_covers_every_object_type_name() {
    let (runtime, mut ctx) = setup();
    let types = [
        (ObjectType::Fixnum, "FIXNUM"),
        (ObjectType::Character, "CHARACTER"),
        (ObjectType::Cons, "CONS"),
        (ObjectType::Symbol, "SYMBOL"),
        (ObjectType::String, "STRING"),
        (ObjectType::SimpleVector, "SIMPLE-VECTOR"),
        (ObjectType::SpecializedArray, "SPECIALIZED-ARRAY"),
        (ObjectType::Array, "ARRAY"),
        (ObjectType::HashTable, "HASH-TABLE"),
        (ObjectType::Function, "FUNCTION"),
        (ObjectType::Closure, "CLOSURE"),
        (ObjectType::Instance, "INSTANCE"),
        (ObjectType::Structure, "STRUCTURE-OBJECT"),
        (ObjectType::Bignum, "BIGNUM"),
        (ObjectType::Ratio, "RATIO"),
        (ObjectType::DoubleFloat, "DOUBLE-FLOAT"),
        (ObjectType::Complex, "COMPLEX"),
        (ObjectType::Package, "PACKAGE"),
        (ObjectType::Readtable, "READTABLE"),
        (ObjectType::Stream, "STREAM"),
        (ObjectType::Code, "CODE"),
    ];
    for (expected, expected_name) in types {
        let condition = ncl_conditions::condition_from_lisp_error(
            &mut ctx,
            &runtime,
            LispError::TypeError {
                datum: Word::fixnum(13),
                expected,
            },
        )
        .unwrap();
        let expected_type = slot_ref(&ctx, ncl_object::Instance::from_word(condition), 1).unwrap();
        assert_eq!(symbol_text(&ctx, expected_type), expected_name);
        assert_eq!(class_name(&ctx, condition), "TYPE-ERROR");
    }
}

#[test]
fn handler_class_matrix_matches_exact_parent_and_unrelated_classes() {
    let (runtime, mut ctx) = setup();
    let error = condition_class(&mut ctx, &runtime, "ERROR").unwrap();
    let warning = condition_class(&mut ctx, &runtime, "WARNING").unwrap();
    let type_error = condition_class(&mut ctx, &runtime, "TYPE-ERROR").unwrap();
    let program_error = condition_class(&mut ctx, &runtime, "PROGRAM-ERROR").unwrap();
    let type_condition = make_condition(&mut ctx, &runtime, type_error, &[]).unwrap();
    let program_condition = make_condition(&mut ctx, &runtime, program_error, &[]).unwrap();
    let warning_condition = make_condition(&mut ctx, &runtime, warning, &[]).unwrap();
    let callback = builtin(&runtime, &mut ctx, "COMMON-LISP", "CONTINUE");

    let broad = push_handler(&mut ctx, &runtime, error, callback.as_word()).unwrap();
    ctx.set_evaluator_runtime(std::ptr::NonNull::<()>::dangling().as_ptr());
    assert_eq!(signal_matched(&mut ctx, type_condition), Ok(true));
    assert_eq!(signal_matched(&mut ctx, program_condition), Ok(true));
    assert_eq!(signal_matched(&mut ctx, warning_condition), Ok(false));
    pop_handler(&mut ctx, &runtime, broad);

    let exact = push_handler(&mut ctx, &runtime, type_error, callback.as_word()).unwrap();
    assert_eq!(signal_matched(&mut ctx, type_condition), Ok(true));
    assert_eq!(signal_matched(&mut ctx, program_condition), Ok(false));
    pop_handler(&mut ctx, &runtime, exact);
    assert_eq!(signal_matched(&mut ctx, type_condition), Ok(false));
}

#[test]
fn custom_slots_select_supplied_initarg_initform_and_nil_defaults() {
    let (runtime, mut ctx) = setup();
    let define = builtin(&runtime, &mut ctx, "NCL-EXT", "DEFINE-CONDITION-CLASS");
    let make = builtin(&runtime, &mut ctx, "COMMON-LISP", "MAKE-CONDITION");
    let slot_ref_builtin = builtin(&runtime, &mut ctx, "NCL-EXT", "CONDITION-SLOT-REF");
    let name = symbol(&mut ctx, &runtime, "NCL-W13", "SLOT-MATRIX");
    let key = symbol(&mut ctx, &runtime, "KEYWORD", "VALUE");
    let initform = builtin(&runtime, &mut ctx, "COMMON-LISP", "CONTINUE");
    let first_spec = make_cons(&mut ctx, &runtime, key, initform.as_word()).unwrap();
    let second_spec = make_cons(&mut ctx, &runtime, Word::NIL, Word::NIL).unwrap();
    let specs_tail = make_cons(&mut ctx, &runtime, second_spec, Word::NIL).unwrap();
    let specs = make_cons(&mut ctx, &runtime, first_spec, specs_tail).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, define, &[name, Word::NIL, specs, Word::NIL]),
        Ok(name)
    );

    ctx.set_condition_handler_invoker(return_marker);
    ctx.set_evaluator_runtime(std::ptr::NonNull::<()>::dangling().as_ptr());
    let supplied = runtime
        .call_builtin(&mut ctx, make, &[name, key, Word::fixnum(5)])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_ref_builtin, &[supplied, Word::fixnum(0)]),
        Ok(Word::fixnum(5))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_ref_builtin, &[supplied, Word::fixnum(1)]),
        Ok(Word::NIL)
    );

    let defaults = runtime.call_builtin(&mut ctx, make, &[name]).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_ref_builtin, &[defaults, Word::fixnum(0)]),
        Ok(Word::fixnum(77))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_ref_builtin, &[defaults, Word::fixnum(1)]),
        Ok(Word::NIL)
    );
    assert_eq!(
        condition_report(&ctx, defaults),
        Some("SLOT-MATRIX condition".to_owned())
    );
}

#[test]
fn restart_matrix_preserves_innermost_order_and_name_comparisons() {
    let (runtime, mut ctx) = setup();
    let first_name =
        make_string(&mut ctx, &runtime, &"W13-FIRST".chars().collect::<Vec<_>>()).unwrap();
    let second_name = make_string(
        &mut ctx,
        &runtime,
        &"W13-SECOND".chars().collect::<Vec<_>>(),
    )
    .unwrap();
    let first = push_restart(
        &mut ctx,
        &runtime,
        first_name,
        Word::fixnum(1),
        Word::NIL,
        Word::fixnum(1),
        Word::fixnum(0),
    )
    .unwrap();
    let second = push_restart(
        &mut ctx,
        &runtime,
        second_name,
        Word::fixnum(2),
        Word::NIL,
        Word::fixnum(0),
        Word::fixnum(1),
    )
    .unwrap();
    let restarts = ncl_conditions::compute_restarts(&mut ctx, &runtime).unwrap();
    assert_ne!(restarts, Word::NIL);
    let first_found = find_restart(&ctx, first_name).unwrap().unwrap();
    let second_found = find_restart(&ctx, second_name).unwrap().unwrap();
    assert_ne!(first_found, second_found);
    let same_text =
        make_string(&mut ctx, &runtime, &"W13-FIRST".chars().collect::<Vec<_>>()).unwrap();
    let different_length =
        make_string(&mut ctx, &runtime, &"W13-F".chars().collect::<Vec<_>>()).unwrap();
    assert!(find_restart(&ctx, same_text).unwrap().is_some());
    assert_eq!(find_restart(&ctx, different_length), Ok(None));
    let second_current = find_restart(&ctx, second_name).unwrap().unwrap();
    assert_eq!(
        ncl_conditions::invoke_restart(&mut ctx, second_current, &[]),
        Ok(Word::fixnum(2))
    );
    assert!(ctx.take_non_local_exit());
    pop_restart(&mut ctx, second);
    pop_restart(&mut ctx, first);
}
