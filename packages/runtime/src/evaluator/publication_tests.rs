use super::*;
use ncl_object::{make_code_object, make_simple_fun, make_string, make_symbol, symbol_function};
use ncl_sys::{CodeError, CodeObjectMetadata, SafepointMap, activate_code_registry, seal_code};

#[test]
fn successful_multi_code_publication_publishes_code_and_activates_registry()
-> Result<(), crate::RuntimeError> {
    let mut runtime = crate::Runtime::new()?;
    let first_function = empty_function(10, "first-published");
    let second_function = empty_function(11, "second-published");
    let first = ncl_sys::alloc_code(8)?;
    let second = ncl_sys::alloc_code(8)?;
    let mut batch = [
        (&first_function, (first, metadata("first-published", 8))),
        (&second_function, (second, metadata("second-published", 8))),
    ];

    runtime.publish_native(&mut batch)?;

    for (_, (code, _)) in &batch {
        // check-added-lines: allow(panic) test-only assertions
        assert!(code.is_published());
        // check-added-lines: allow(panic) test-only assertions
        assert_eq!(
            activate_code_registry(code),
            Err(CodeError::AlreadyActivated)
        );
    }
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(
        runtime
            .context
            .thread_access()
            .unregister_code(&batch[0].1.0)
            .map(|metadata| metadata.function_name),
        Some("first-published".to_owned())
    );
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(
        runtime
            .context
            .thread_access()
            .unregister_code(&batch[1].1.0)
            .map(|metadata| metadata.function_name),
        Some("second-published".to_owned())
    );
    Ok(())
}

#[test]
fn sealing_failure_cleanup_preserves_symbol_function() -> Result<(), crate::RuntimeError> {
    let mut runtime = crate::Runtime::new()?;
    let name = make_string(&mut runtime.context, &runtime.object, &['f'])?;
    let symbol = make_symbol(&mut runtime.context, &runtime.object, name)?;
    let old_code = make_code_object(
        &mut runtime.context,
        &runtime.object,
        17,
        8,
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )?;
    let old_function = make_simple_fun(
        &mut runtime.context,
        &runtime.object,
        17,
        Word::NIL,
        Word::NIL,
        old_code,
    )?
    .as_word();
    runtime
        .context
        .write_object_slot(symbol, ncl_object::symbol_offset::FUNCTION, old_function)?;

    let mut code = ncl_sys::alloc_code(8)?;
    seal_code(&mut code)?;
    let function = ncl_ir::Function {
        id: ncl_ir::FunctionId(1),
        name: "failed-publication".to_owned(),
        params: Vec::new(),
        return_types: Vec::new(),
        blocks: Vec::new(),
        locals: Vec::new(),
        constants: Vec::new(),
        handler_regions: Vec::new(),
        debug: Vec::new(),
    };
    let mut published = [(
        &function,
        (
            code,
            CodeObjectMetadata {
                entry_offset: 0,
                size: 8,
                frame_words: 0,
                function_name: "failed-publication".to_owned(),
                source_locations: Vec::new(),
                constant_slots: Vec::new(),
                safepoint_map: SafepointMap::default(),
                debug_table: Vec::new(),
            },
        ),
    )];
    assert!(matches!(
        runtime.publish_native(&mut published),
        Err(crate::RuntimeError::Code(CodeError::AlreadySealed))
    ));
    assert!(
        runtime
            .context
            .thread_access()
            .unregister_code(&published[0].1.0)
            .is_none()
    );
    assert!(!published[0].1.0.is_published());
    assert_eq!(symbol_function(&runtime.context, symbol), Ok(old_function));
    Ok(())
}

#[test]
fn partial_registration_failure_removes_only_the_new_batch_entry() -> Result<(), crate::RuntimeError>
{
    let mut runtime = crate::Runtime::new()?;
    let fresh = ncl_sys::alloc_code(8)?;
    let existing = ncl_sys::alloc_code(8)?;
    runtime
        .context
        .thread_access()
        .register_code(&existing, metadata("existing", 8))?;
    let function = empty_function(2, "partial-registration");
    let mut batch = [
        (&function, (fresh, metadata("fresh", 8))),
        (&function, (existing, metadata("duplicate", 8))),
    ];

    assert!(matches!(
        runtime.publish_native(&mut batch),
        Err(crate::RuntimeError::Code(CodeError::AlreadyRegistered))
    ));
    assert!(
        runtime
            .context
            .thread_access()
            .unregister_code(&batch[0].1.0)
            .is_none()
    );
    assert!(
        runtime
            .context
            .thread_access()
            .unregister_code(&batch[1].1.0)
            .is_some()
    );
    assert!(!batch[0].1.0.is_published());
    assert!(!batch[1].1.0.is_published());
    Ok(())
}

#[test]
fn multi_code_sealing_failure_removes_every_prepared_entry() -> Result<(), crate::RuntimeError> {
    let mut runtime = crate::Runtime::new()?;
    let first = ncl_sys::alloc_code(8)?;
    let mut already_sealed = ncl_sys::alloc_code(8)?;
    seal_code(&mut already_sealed)?;
    let function = empty_function(3, "multi-code-sealing");
    let mut batch = [
        (&function, (first, metadata("first", 8))),
        (&function, (already_sealed, metadata("already-sealed", 8))),
    ];

    assert!(matches!(
        runtime.publish_native(&mut batch),
        Err(crate::RuntimeError::Code(CodeError::AlreadySealed))
    ));
    for (_, (code, _)) in &batch {
        assert!(!code.is_published());
        assert!(
            runtime
                .context
                .thread_access()
                .unregister_code(code)
                .is_none()
        );
    }
    Ok(())
}

#[test]
fn native_compilation_rejects_frame_words_that_exceed_metadata_capacity()
-> Result<(), crate::RuntimeError> {
    let compilation = native_compilation(0, (u32::from(u16::MAX) + 1) * 8)?;

    // check-added-lines: allow(panic) test-only assertions
    assert!(matches!(
        compilation.into_metadata(),
        Err(crate::RuntimeError::Native(message)) if message == "frame is too large"
    ));
    Ok(())
}

#[test]
fn native_compilation_rejects_entry_address_overflow() -> Result<(), crate::RuntimeError> {
    let compilation = native_compilation(usize::MAX, 0)?;

    // check-added-lines: allow(panic) test-only assertions
    assert!(matches!(
        compilation.entry_address(),
        Err(crate::RuntimeError::Code(CodeError::OutOfBounds))
    ));
    Ok(())
}

fn native_compilation(
    entry_offset: usize,
    frame_size: u32,
) -> Result<NativeCompilation, crate::RuntimeError> {
    Ok(NativeCompilation {
        code: ncl_sys::alloc_code(8)?,
        entry_offset,
        size: 8,
        frame_size,
        function_name: "fixture".to_owned(),
        safepoint_maps: Vec::new(),
    })
}

fn empty_function(id: u32, name: &str) -> ncl_ir::Function {
    ncl_ir::Function {
        id: ncl_ir::FunctionId(id),
        name: name.to_owned(),
        params: Vec::new(),
        return_types: Vec::new(),
        blocks: Vec::new(),
        locals: Vec::new(),
        constants: Vec::new(),
        handler_regions: Vec::new(),
        debug: Vec::new(),
    }
}

fn metadata(name: &str, size: usize) -> CodeObjectMetadata {
    CodeObjectMetadata {
        entry_offset: 0,
        size,
        frame_words: 0,
        function_name: name.to_owned(),
        source_locations: Vec::new(),
        constant_slots: Vec::new(),
        safepoint_map: SafepointMap::default(),
        debug_table: Vec::new(),
    }
}
