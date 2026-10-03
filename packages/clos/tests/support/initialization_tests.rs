#![allow(clippy::expect_used)]

use super::*;

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("context");
    (runtime, ctx)
}

#[test]
fn initarg_adapter_rejects_a_missing_initarg_list() {
    let args = BuiltinArgs::new(&[]);
    assert_eq!(initarg_adapter(&args), Err(ObjectError::TypeError));
}

#[test]
fn resolve_class_reports_the_pending_type_error_for_non_class_values() {
    let (runtime, mut ctx) = setup();
    assert_eq!(
        resolve_class(&mut ctx, &runtime, Word::fixnum(7)),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        ctx.take_pending_lisp_error(),
        Some(LispError::TypeError {
            datum: Word::fixnum(7),
            expected: ObjectType::SimpleVector,
        })
    );
}

#[test]
fn class_slots_accepts_an_empty_effective_slot_field() {
    let (runtime, mut ctx) = setup();
    let class =
        ncl_object::make_simple_vector(&mut ctx, &runtime, &[Word::NIL, Word::NIL, Word::NIL])
            .expect("class");
    assert_eq!(class_slots(&ctx, class), Ok(Vec::new()));
}

#[test]
fn initialization_callbacks_reject_missing_required_arguments() {
    let (runtime, mut ctx) = setup();
    let args = BuiltinArgs::new(&[]);
    let mut values = MultipleValues::new();
    assert_eq!(
        make_instance_builtin(&mut ctx, &runtime, &args, &mut values),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        initialize_instance_builtin(&mut ctx, &runtime, &args, &mut values),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        shared_initialize_builtin(&mut ctx, &runtime, &args, &mut values),
        Err(ObjectError::TypeError)
    );
}
