use super::*;

#[test]
fn registry_preparation_is_unique_and_unregistered_removal_is_empty() -> Result<(), CodeError> {
    let code = alloc_code(8)?;
    let metadata = |function_name| CodeObjectMetadata {
        entry_offset: 0,
        size: code.len(),
        frame_words: 0,
        function_name,
        source_locations: Vec::new(),
        constant_slots: Vec::new(),
        safepoint_map: SafepointMap::default(),
        debug_table: Vec::new(),
    };
    let mut first_registry = CodeRegistry::default();
    let mut second_registry = CodeRegistry::default();

    assert_eq!(
        first_registry.register(&code, metadata("first".to_owned())),
        Ok(())
    );
    assert_eq!(
        second_registry.register(&code, metadata("second".to_owned())),
        Err(CodeError::AlreadyRegistered)
    );
    assert!(
        first_registry
            .find(code.address().saturating_sub(1))
            .is_none()
    );
    assert!(first_registry.find(code.address()).is_none());
    assert!(first_registry.unregister(&code).is_some());
    assert!(second_registry.unregister(&code).is_none());
    Ok(())
}
