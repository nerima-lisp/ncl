use super::*;

#[test]
fn value_producers_reject_foreign_and_duplicate_identities() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(98), 0);
    let foreign_form = FormId::new(CompilationUnitId::new(99), 0);
    let mut builder = form_builder(form)?;
    let producer = builder.alloc_node(builder.root_scope(), NodeKind::Literal(Literal::True))?;
    let value = ValueId::try_from_index(form, 0)?;
    let foreign_value = ValueId::try_from_index(foreign_form, 0)?;
    let foreign_producer = ncl_hir::NodeId::try_from_index(foreign_form, 1)?;
    let missing_producer = ncl_hir::NodeId::try_from_index(form, 9)?;

    assert_eq!(
        builder.register_value_producer(foreign_value, producer),
        Err(ncl_hir::FormError::ForeignValue(foreign_value))
    );
    assert_eq!(
        builder.register_value_producer(value, foreign_producer),
        Err(ncl_hir::FormError::ForeignNode(foreign_producer))
    );
    assert_eq!(
        builder.register_value_producer(value, missing_producer),
        Err(ncl_hir::FormError::InvalidNode(missing_producer))
    );
    assert_eq!(builder.register_value_producer(value, producer), Ok(()));
    assert_eq!(
        builder.register_value_producer(value, producer),
        Err(ncl_hir::FormError::DuplicateValueProducer(value))
    );
    Ok(())
}

#[test]
fn type_specifier_arrays_preserve_nested_symbol_identity() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(100), 0);
    let array_element = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    let specialized_element = SymbolId::try_from_index(form.compilation_unit(), 1)?;
    let mut builder = form_builder(form)?;
    builder.add_type_spec_constant(Literal::Array {
        identity: 0,
        dimensions: vec![1],
        element_type: LiteralElementType::T,
        elements: vec![Literal::Symbol(array_element)],
    })?;
    builder.add_type_spec_constant(Literal::SpecializedArray {
        identity: 1,
        element_type: LiteralElementType::T,
        elements: vec![Literal::Symbol(specialized_element)],
    })?;

    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(
            form,
            &["ARRAY-ELEMENT", "SPECIALIZED-ELEMENT"]
        )?),
        Ok(())
    );
    Ok(())
}

#[test]
fn lambda_signatures_reject_duplicate_parameters_and_self_scope_defaults()
-> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(101), 0);
    let mut builder = form_builder(form)?;
    let root = builder.root_scope();
    let body_scope = builder.bindings_mut().push_scope(root)?;
    let name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    builder.register_symbol(name)?;
    let binding = builder.bindings_mut().bind_variable(body_scope, name)?;
    let lambda = builder.alloc_node(
        root,
        NodeKind::Lambda {
            parameters: vec![binding],
            body_scope,
            body: Vec::new(),
        },
    )?;
    assert_eq!(
        builder.set_lambda_signature_for_lambda(
            lambda,
            LambdaSignature {
                required: vec![binding],
                optional: vec![LambdaOptional {
                    binding,
                    default: None,
                    initializer_scope: root,
                    supplied_p: None,
                }],
                ..empty_lambda_signature()
            },
        ),
        Err(ncl_hir::FormError::InvalidFunctionSignature)
    );

    let form = FormId::new(CompilationUnitId::new(101), 1);
    let mut builder = form_builder(form)?;
    let root = builder.root_scope();
    let body_scope = builder.bindings_mut().push_scope(root)?;
    let required_name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    let optional_name = SymbolId::try_from_index(form.compilation_unit(), 1)?;
    builder.register_symbol(required_name)?;
    builder.register_symbol(optional_name)?;
    let required = builder
        .bindings_mut()
        .bind_variable(body_scope, required_name)?;
    let optional = builder
        .bindings_mut()
        .bind_variable(body_scope, optional_name)?;
    let lambda = builder.alloc_node(
        root,
        NodeKind::Lambda {
            parameters: vec![required, optional],
            body_scope,
            body: Vec::new(),
        },
    )?;
    assert_eq!(
        builder.set_lambda_signature_for_lambda(
            lambda,
            LambdaSignature {
                required: vec![required],
                optional: vec![LambdaOptional {
                    binding: optional,
                    default: None,
                    initializer_scope: body_scope,
                    supplied_p: None,
                }],
                ..empty_lambda_signature()
            },
        ),
        Err(ncl_hir::FormError::InvalidFunctionSignature)
    );
    Ok(())
}

#[test]
fn optional_and_keyword_lambda_signatures_verify_with_outer_defaults() -> Result<(), Box<dyn Error>>
{
    let form = FormId::new(CompilationUnitId::new(102), 0);
    let mut builder = form_builder(form)?;
    let root = builder.root_scope();
    let body_scope = builder.bindings_mut().push_scope(root)?;
    let optional_name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    let rest_name = SymbolId::try_from_index(form.compilation_unit(), 1)?;
    builder.register_symbol(optional_name)?;
    builder.register_symbol(rest_name)?;
    let optional = builder
        .bindings_mut()
        .bind_variable(body_scope, optional_name)?;
    let rest = builder
        .bindings_mut()
        .bind_variable(body_scope, rest_name)?;
    let default = builder.alloc_node(root, NodeKind::Literal(Literal::True))?;
    let lambda = builder.alloc_node(
        root,
        NodeKind::Lambda {
            parameters: vec![optional, rest],
            body_scope,
            body: Vec::new(),
        },
    )?;
    builder.set_lambda_signature_for_lambda(
        lambda,
        LambdaSignature {
            optional: vec![LambdaOptional {
                binding: optional,
                default: Some(default),
                initializer_scope: root,
                supplied_p: None,
            }],
            rest: Some(rest),
            ..empty_lambda_signature()
        },
    )?;
    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &["OPTIONAL", "REST"])?),
        Ok(())
    );

    let form = FormId::new(CompilationUnitId::new(102), 1);
    let mut builder = form_builder(form)?;
    let root = builder.root_scope();
    let body_scope = builder.bindings_mut().push_scope(root)?;
    let name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    let keyword = SymbolId::try_from_index(form.compilation_unit(), 1)?;
    builder.register_symbol(name)?;
    let binding = builder.bindings_mut().bind_variable(body_scope, name)?;
    let default = builder.alloc_node(root, NodeKind::Literal(Literal::Nil))?;
    let lambda = builder.alloc_node(
        root,
        NodeKind::Lambda {
            parameters: vec![binding],
            body_scope,
            body: Vec::new(),
        },
    )?;
    builder.set_lambda_signature_for_lambda(
        lambda,
        LambdaSignature {
            keys: vec![LambdaKey {
                keyword,
                binding,
                default: Some(default),
                initializer_scope: root,
                supplied_p: None,
            }],
            ..empty_lambda_signature()
        },
    )?;
    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &["KEY-VALUE", "KEYWORD"])?),
        Ok(())
    );
    Ok(())
}

#[test]
fn verify_errors_keep_their_display_and_id_conversion_contracts() -> Result<(), Box<dyn Error>> {
    let id_error = IdError::IndexOverflow { index: usize::MAX };
    assert_eq!(VerifyError::from(id_error), VerifyError::Id(id_error));
    let value = ValueId::try_from_index(FormId::new(CompilationUnitId::new(104), 0), 0)?;
    let error = VerifyError::InvalidValue(value);
    assert_eq!(
        error.to_string(),
        format!("HIR verification failed: {error:?}")
    );
    Ok(())
}
