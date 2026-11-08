use super::*;
use ncl_object::{make_code_object, make_simple_fun, make_string, make_symbol, symbol_function};
use ncl_sys::{CodeError, CodeObjectMetadata, SafepointMap, seal_code};

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
