use super::*;

#[test]
fn type_assertions_require_resolved_type_and_source_specifier_ids() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(82), 0);
    let mut builder = form_builder(form)?;
    let scope = builder.root_scope();
    let value = builder.alloc_node(scope, NodeKind::Literal(Literal::True))?;
    let missing_type = TypeId::try_from_index(form, 9)?;
    let missing_constant = TypeSpecConstId::try_from_index(form, 9)?;
    builder.alloc_node(
        scope,
        NodeKind::TypeAssertion {
            value,
            asserted_type: missing_type,
            expected_type: missing_constant,
        },
    )?;

    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &[])?),
        Err(VerifyError::InvalidType(missing_type))
    );

    let form = FormId::new(CompilationUnitId::new(82), 1);
    let mut builder = form_builder(form)?;
    let scope = builder.root_scope();
    let value = builder.alloc_node(scope, NodeKind::Literal(Literal::True))?;
    let valid_type = builder.types_mut().named(BuiltinType::Integer)?;
    let missing_constant = TypeSpecConstId::try_from_index(form, 9)?;
    builder.alloc_node(
        scope,
        NodeKind::TypeAssertion {
            value,
            asserted_type: valid_type,
            expected_type: missing_constant,
        },
    )?;

    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &[])?),
        Err(VerifyError::InvalidTypeSpecConstant(missing_constant))
    );
    Ok(())
}

#[test]
fn check_type_diagnostic_origin_must_exist_in_the_form() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(83), 0);
    let mut builder = form_builder(form)?;
    let scope = builder.root_scope();
    let producer = builder.alloc_node(scope, NodeKind::Literal(Literal::True))?;
    let value = ValueId::try_from_index(form, 0)?;
    builder.register_value_producer(value, producer)?;
    let accepted_type = builder.types_mut().named(BuiltinType::Integer)?;
    let expected_type = builder.add_type_spec_constant(Literal::True)?;
    let missing_origin = SourceOriginId::try_from_index(form, 1)?;
    builder.alloc_node(
        scope,
        NodeKind::CheckType {
            value,
            accepted_type,
            expected_type,
            source_origin: missing_origin,
        },
    )?;

    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &[])?),
        Err(VerifyError::InvalidOrigin(missing_origin))
    );
    Ok(())
}

#[test]
fn check_type_without_a_conditional_cfg_edge_is_rejected() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(73), 0);
    let mut builder = form_builder(form)?;
    let scope = builder.root_scope();
    let producer = builder.alloc_node(scope, NodeKind::Literal(Literal::True))?;
    let value = ValueId::try_from_index(form, 0)?;
    builder.register_value_producer(value, producer)?;
    let expected_type = builder.add_type_spec_constant(Literal::True)?;
    let accepted_type = builder.types_mut().named(BuiltinType::Integer)?;
    let source_origin = SourceOriginId::try_from_index(form, 0)?;
    let check = builder.alloc_node(
        scope,
        NodeKind::CheckType {
            value,
            accepted_type,
            expected_type,
            source_origin,
        },
    )?;
    let function = builder.alloc_node(
        scope,
        NodeKind::Lambda {
            parameters: Vec::new(),
            body_scope: scope,
            body: Vec::new(),
        },
    )?;
    {
        let cfg = builder.control_flow_mut();
        cfg.begin_function(function)?;
        let block = cfg.create_block(function)?;
        cfg.append_operation(block, producer)?;
        cfg.append_operation(block, check)?;
        cfg.set_terminator(block, Terminator::Return { values: Vec::new() })?;
    }

    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &[])?),
        Err(VerifyError::InvalidCheckEdges(check))
    );
    Ok(())
}

#[test]
fn one_check_value_cannot_be_produced_in_multiple_cfg_blocks() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(74), 0);
    let mut builder = form_builder(form)?;
    let scope = builder.root_scope();
    let producer = builder.alloc_node(scope, NodeKind::Literal(Literal::True))?;
    let value = ValueId::try_from_index(form, 0)?;
    builder.register_value_producer(value, producer)?;
    let expected_type = builder.add_type_spec_constant(Literal::True)?;
    let accepted_type = builder.types_mut().named(BuiltinType::Integer)?;
    let source_origin = SourceOriginId::try_from_index(form, 0)?;
    let check = builder.alloc_node(
        scope,
        NodeKind::CheckType {
            value,
            accepted_type,
            expected_type,
            source_origin,
        },
    )?;
    let function = builder.alloc_node(
        scope,
        NodeKind::Lambda {
            parameters: Vec::new(),
            body_scope: scope,
            body: Vec::new(),
        },
    )?;
    {
        let cfg = builder.control_flow_mut();
        cfg.begin_function(function)?;
        let entry = cfg.create_block(function)?;
        let success = cfg.create_block(function)?;
        let failure = cfg.create_block(function)?;
        cfg.append_operation(entry, producer)?;
        cfg.set_terminator(
            entry,
            Terminator::Conditional {
                condition: check,
                if_true: success,
                if_false: failure,
            },
        )?;
        cfg.append_operation(success, producer)?;
        cfg.set_terminator(success, Terminator::Unreachable)?;
        cfg.set_terminator(failure, Terminator::Unreachable)?;
    }

    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &[])?),
        Err(VerifyError::InvalidValueProducer { check, producer })
    );
    Ok(())
}
