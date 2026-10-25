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
