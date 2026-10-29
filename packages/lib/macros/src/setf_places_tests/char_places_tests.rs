use super::*;

#[test]
fn char_and_schar_places_use_char_set_and_validate_arity() {
    let runtime = Runtime::new().unwrap_or_else(super::super::test_operation_failure);
    let mut ctx = ThreadContext::new();
    registry_with_builtins(&runtime, &mut ctx).unwrap_or_else(super::super::test_operation_failure);
    let registry = PlaceRegistry::new(&runtime);
    let string =
        symbol(&mut ctx, &runtime, "STRING").unwrap_or_else(super::super::test_operation_failure);
    let index =
        symbol(&mut ctx, &runtime, "INDEX").unwrap_or_else(super::super::test_operation_failure);
    let value =
        symbol(&mut ctx, &runtime, "VALUE").unwrap_or_else(super::super::test_operation_failure);
    let setter = symbol(&mut ctx, &runtime, "NCL-EXT::CHAR-SET")
        .unwrap_or_else(super::super::test_operation_failure);

    for name in ["CHAR", "SCHAR"] {
        let place = place_form(&mut ctx, &runtime, name, &[string, index])
            .unwrap_or_else(super::super::test_operation_failure);
        let expansion = expand_get_setf_expansion(&mut ctx, &runtime, &registry, place)
            .unwrap_or_else(super::super::test_operation_failure);
        assert_eq!(expansion.temporary_variables.len(), 2);
        assert_eq!(expansion.value_forms, vec![string, index]);
        let store = elements(&mut ctx, expansion.store_form)
            .unwrap_or_else(super::super::test_operation_failure);
        assert_eq!(store[0], setter);
        for arguments in [&[][..], &[string][..], &[string, index, value][..]] {
            let malformed = place_form(&mut ctx, &runtime, name, arguments)
                .unwrap_or_else(super::super::test_operation_failure);
            assert!(matches!(
                expand_get_setf_expansion(&mut ctx, &runtime, &registry, malformed),
                Err(ObjectError::TypeError)
            ));
        }
    }
}
