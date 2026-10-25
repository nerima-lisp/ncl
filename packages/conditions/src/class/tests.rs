use super::{SUPERCLASS_SLOT, wire_superclasses};
use ncl_object::{HeapRootedWord, ObjectError, Runtime, ThreadContext, Word, simple_vector_ref};

#[test]
fn wire_superclasses_handles_root_classes_and_missing_names() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    crate::register(&runtime)?;
    let condition = runtime
        .class(&mut ctx, "CONDITION")
        .ok_or(ObjectError::Layout)?;
    let condition = HeapRootedWord::new(&ctx, condition)?;

    wire_superclasses(&mut ctx, &runtime, "CONDITION", &[])?;
    assert_eq!(
        simple_vector_ref(&ctx, condition.get(), SUPERCLASS_SLOT),
        Ok(Word::NIL)
    );
    assert_eq!(
        wire_superclasses(&mut ctx, &runtime, "MISSING-CHILD", &[]),
        Err(ObjectError::Layout)
    );
    assert_eq!(
        wire_superclasses(&mut ctx, &runtime, "CONDITION", &["MISSING-PARENT"]),
        Err(ObjectError::Layout)
    );
    Ok(())
}

#[test]
fn wire_superclasses_preserves_single_and_multiple_parents_under_gc_stress()
-> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    crate::register(&runtime)?;
    let condition = runtime
        .class(&mut ctx, "CONDITION")
        .ok_or(ObjectError::Layout)?;
    let condition = HeapRootedWord::new(&ctx, condition)?;
    let simple_condition = runtime
        .class(&mut ctx, "SIMPLE-CONDITION")
        .ok_or(ObjectError::Layout)?;
    let simple_condition = HeapRootedWord::new(&ctx, simple_condition)?;
    let simple_error = runtime
        .class(&mut ctx, "SIMPLE-ERROR")
        .ok_or(ObjectError::Layout)?;
    let simple_error = HeapRootedWord::new(&ctx, simple_error)?;

    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    wire_superclasses(&mut ctx, &runtime, "SIMPLE-CONDITION", &["CONDITION"])?;
    assert_eq!(
        simple_vector_ref(&ctx, simple_condition.get(), SUPERCLASS_SLOT),
        Ok(condition.get())
    );

    wire_superclasses(
        &mut ctx,
        &runtime,
        "SIMPLE-ERROR",
        &["SIMPLE-CONDITION", "CONDITION"],
    )?;
    let parents = simple_vector_ref(&ctx, simple_error.get(), SUPERCLASS_SLOT)?;
    assert_eq!(
        super::direct_parents(&ctx, parents)?,
        [simple_condition.get(), condition.get()]
    );
    Ok(())
}
