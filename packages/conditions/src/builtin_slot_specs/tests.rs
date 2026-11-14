use super::install;
use ncl_object::{HeapRootedWord, ObjectError, Runtime, ThreadContext};

#[test]
fn install_reports_an_unregistered_builtin_class() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

    assert_eq!(install(&runtime, &mut ctx), Err(ObjectError::Layout));
    Ok(())
}

#[test]
fn install_reports_a_missing_type_error_class() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    for name in ["SIMPLE-CONDITION", "SIMPLE-ERROR", "SIMPLE-WARNING"] {
        crate::class::install_class(&mut ctx, &runtime, name)?;
    }

    assert_eq!(install(&runtime, &mut ctx), Err(ObjectError::Layout));
    Ok(())
}

#[test]
fn install_preserves_slot_keywords_under_gc_stress() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    crate::register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let class = runtime
        .class(&mut ctx, "TYPE-ERROR")
        .ok_or(ObjectError::Layout)?;
    let class = HeapRootedWord::new(&ctx, class)?;
    let datum = crate::slots::keyword(&mut ctx, &runtime, "DATUM")?;
    let datum = HeapRootedWord::new(&ctx, datum)?;
    let expected_type = crate::slots::keyword(&mut ctx, &runtime, "EXPECTED-TYPE")?;
    let expected_type = HeapRootedWord::new(&ctx, expected_type)?;

    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    install(&runtime, &mut ctx)?;
    let specs = crate::slots::slot_specs(&ctx, class.get())?;
    assert_eq!(specs.len(), 4);
    assert_eq!(specs.get(0).map(HeapRootedWord::get), Some(datum.get()));
    assert_eq!(
        specs.get(2).map(HeapRootedWord::get),
        Some(expected_type.get())
    );
    Ok(())
}

#[test]
fn install_propagates_missing_single_slot_classes() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    for name in [
        "SIMPLE-CONDITION",
        "SIMPLE-ERROR",
        "SIMPLE-WARNING",
        "TYPE-ERROR",
        "ARITHMETIC-ERROR",
        "DIVISION-BY-ZERO",
        "FLOATING-POINT-OVERFLOW",
        "FLOATING-POINT-UNDERFLOW",
        "FLOATING-POINT-INVALID-OPERATION",
        "FLOATING-POINT-INEXACT",
        "CELL-ERROR",
        "UNBOUND-VARIABLE",
        "UNDEFINED-FUNCTION",
        "UNBOUND-SLOT",
    ] {
        crate::class::install_class(&mut ctx, &runtime, name)?;
    }

    assert_eq!(install(&runtime, &mut ctx), Err(ObjectError::Layout));
    crate::class::install_class(&mut ctx, &runtime, "PACKAGE-ERROR")?;
    assert_eq!(install(&runtime, &mut ctx), Err(ObjectError::Layout));
    crate::class::install_class(&mut ctx, &runtime, "STREAM-ERROR")?;
    assert_eq!(install(&runtime, &mut ctx), Err(ObjectError::Layout));
    crate::class::install_class(&mut ctx, &runtime, "FILE-ERROR")?;
    install(&runtime, &mut ctx)?;

    for (class_name, keyword_name) in [
        ("PACKAGE-ERROR", "PACKAGE"),
        ("STREAM-ERROR", "STREAM"),
        ("FILE-ERROR", "PATHNAME"),
    ] {
        let class = runtime
            .class(&mut ctx, class_name)
            .ok_or(ObjectError::Layout)?;
        let class = HeapRootedWord::new(&ctx, class)?;
        let keyword = crate::slots::keyword(&mut ctx, &runtime, keyword_name)?;
        let keyword = HeapRootedWord::new(&ctx, keyword)?;
        let specs = crate::slots::slot_specs(&ctx, class.get())?;
        assert_eq!(specs.len(), 2);
        assert_eq!(specs.get(0).map(HeapRootedWord::get), Some(keyword.get()));
        assert_eq!(
            specs.get(1).map(HeapRootedWord::get),
            Some(ncl_object::Word::NIL)
        );
    }

    let unbound_slot = runtime
        .class(&mut ctx, "UNBOUND-SLOT")
        .ok_or(ObjectError::Layout)?;
    let unbound_slot = HeapRootedWord::new(&ctx, unbound_slot)?;
    let specs = crate::slots::slot_specs(&ctx, unbound_slot.get())?;
    assert_eq!(specs.len(), 4);
    let name = crate::slots::keyword(&mut ctx, &runtime, "NAME")?;
    let name = HeapRootedWord::new(&ctx, name)?;
    let instance = crate::slots::keyword(&mut ctx, &runtime, "INSTANCE")?;
    let instance = HeapRootedWord::new(&ctx, instance)?;
    assert_eq!(specs.get(0).map(HeapRootedWord::get), Some(name.get()));
    assert_eq!(
        specs.get(1).map(HeapRootedWord::get),
        Some(ncl_object::Word::NIL)
    );
    assert_eq!(specs.get(2).map(HeapRootedWord::get), Some(instance.get()));
    assert_eq!(
        specs.get(3).map(HeapRootedWord::get),
        Some(ncl_object::Word::NIL)
    );
    Ok(())
}
