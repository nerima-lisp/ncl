use super::*;

#[test]
fn control_nodes_accept_resolved_targets_and_same_scope_children() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(78), 0);
    let mut builder = form_builder(form)?;
    let scope = builder.root_scope();
    let block_name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    let tag_name = SymbolId::try_from_index(form.compilation_unit(), 1)?;
    builder.register_symbol(block_name)?;
    builder.register_symbol(tag_name)?;
    let value = builder.alloc_node(scope, NodeKind::Literal(Literal::True))?;
    let block = builder.alloc_node(
        scope,
        NodeKind::Block {
            name: block_name,
            body: vec![value],
        },
    )?;
    builder.alloc_node(
        scope,
        NodeKind::ReturnFrom {
            target: block,
            value: Some(value),
        },
    )?;
    let tag_block = ncl_hir::BlockId::try_from_index(form, 0)?;
    let go = builder.alloc_node(scope, NodeKind::Go { target: tag_block })?;
    let tagbody = builder.alloc_node(
        scope,
        NodeKind::Tagbody {
            tags: vec![TagbodyTag {
                name: tag_name,
                block: tag_block,
            }],
            forms: vec![go],
        },
    )?;
    let tag = builder.alloc_node(scope, NodeKind::Literal(Literal::Nil))?;
    builder.alloc_node(
        scope,
        NodeKind::Catch {
            tag,
            body: vec![value],
        },
    )?;
    builder.alloc_node(scope, NodeKind::Throw { tag, value })?;
    let cleanup = CleanupId::try_from_index(form, 0)?;
    builder.alloc_node(
        scope,
        NodeKind::UnwindProtect {
            cleanup,
            protected: value,
            cleanup_forms: vec![value],
        },
    )?;
    builder.alloc_node(
        scope,
        NodeKind::NonLocalExit {
            target: ExitTarget::Function(tagbody),
            values: vec![value],
            cleanups: vec![cleanup],
        },
    )?;
    {
        let cfg = builder.control_flow_mut();
        cfg.begin_function(tagbody)?;
        let entry = cfg.create_block(tagbody)?;
        assert_eq!(entry, tag_block);
        cfg.set_terminator(entry, Terminator::Return { values: Vec::new() })?;
    }

    let hir = builder.finish()?;
    assert_eq!(hir.verify(&symbol_table(form, &["BLOCK", "TAG"])?), Ok(()));
    Ok(())
}

#[test]
fn unreachable_function_terminator_is_accepted_by_full_form_verification()
-> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(70), 0);
    let mut builder = form_builder(form)?;
    let function = builder.alloc_node(
        builder.root_scope(),
        NodeKind::Lambda {
            parameters: Vec::new(),
            body_scope: builder.root_scope(),
            body: Vec::new(),
        },
    )?;
    let block = {
        let cfg = builder.control_flow_mut();
        cfg.begin_function(function)?;
        let block = cfg.create_block(function)?;
        cfg.set_terminator(block, Terminator::Unreachable)?;
        block
    };

    let hir = builder.finish()?;
    assert_eq!(
        hir.control_flow()
            .block(block)
            .map(ncl_hir::BasicBlock::terminator),
        Some(&Terminator::Unreachable)
    );
    assert_eq!(hir.verify(&symbol_table(form, &[])?), Ok(()));
    Ok(())
}

#[test]
fn go_target_must_be_a_registered_tagbody_tag() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(71), 0);
    let mut builder = form_builder(form)?;
    let scope = builder.root_scope();
    let function = builder.alloc_node(
        scope,
        NodeKind::Lambda {
            parameters: Vec::new(),
            body_scope: scope,
            body: Vec::new(),
        },
    )?;
    let target = {
        let cfg = builder.control_flow_mut();
        cfg.begin_function(function)?;
        let target = cfg.create_block(function)?;
        cfg.set_terminator(target, Terminator::Unreachable)?;
        target
    };
    builder.alloc_node(scope, NodeKind::Go { target })?;

    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &[])?),
        Err(VerifyError::InvalidTagTarget(target))
    );
    Ok(())
}

#[test]
fn nonlocal_exit_accepts_an_existing_tag_target_block() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(77), 0);
    let mut builder = form_builder(form)?;
    let scope = builder.root_scope();
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
        let target = cfg.create_block(function)?;
        cfg.set_terminator(
            entry,
            Terminator::NonLocalExit {
                target: ExitTarget::Tag(target),
                values: Vec::new(),
                cleanups: Vec::new(),
            },
        )?;
        cfg.set_terminator(target, Terminator::Unreachable)?;
    }

    let hir = builder.finish()?;
    assert_eq!(hir.verify(&symbol_table(form, &[])?), Ok(()));
    Ok(())
}

#[test]
fn return_from_target_must_resolve_to_a_block_node() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(72), 0);
    let mut builder = form_builder(form)?;
    let scope = builder.root_scope();
    let target = builder.alloc_node(scope, NodeKind::Literal(Literal::True))?;
    builder.alloc_node(
        scope,
        NodeKind::ReturnFrom {
            target,
            value: None,
        },
    )?;

    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &[])?),
        Err(VerifyError::InvalidExitTarget(target))
    );
    Ok(())
}

#[test]
fn unwind_protect_cleanup_ids_must_be_unique_within_a_form() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(89), 0);
    let mut builder = form_builder(form)?;
    let scope = builder.root_scope();
    let protected = builder.alloc_node(scope, NodeKind::Literal(Literal::True))?;
    let cleanup = CleanupId::try_from_index(form, 0)?;
    for _ in 0..2 {
        builder.alloc_node(
            scope,
            NodeKind::UnwindProtect {
                cleanup,
                protected,
                cleanup_forms: Vec::new(),
            },
        )?;
    }

    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &[])?),
        Err(VerifyError::InvalidCleanup(cleanup))
    );
    Ok(())
}

#[test]
fn return_terminator_values_must_resolve_to_nodes() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(93), 0);
    let mut builder = form_builder(form)?;
    let scope = builder.root_scope();
    let function = builder.alloc_node(
        scope,
        NodeKind::Lambda {
            parameters: Vec::new(),
            body_scope: scope,
            body: Vec::new(),
        },
    )?;
    let missing_value = ncl_hir::NodeId::try_from_index(form, 9)?;
    {
        let cfg = builder.control_flow_mut();
        cfg.begin_function(function)?;
        let entry = cfg.create_block(function)?;
        cfg.set_terminator(
            entry,
            Terminator::Return {
                values: vec![missing_value],
            },
        )?;
    }

    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &[])?),
        Err(VerifyError::InvalidNode(missing_value))
    );
    Ok(())
}

#[test]
fn tagbody_tags_must_reference_existing_blocks() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(94), 0);
    let mut builder = form_builder(form)?;
    let scope = builder.root_scope();
    let name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    builder.register_symbol(name)?;
    let missing_block = ncl_hir::BlockId::try_from_index(form, 9)?;
    builder.alloc_node(
        scope,
        NodeKind::Tagbody {
            tags: vec![TagbodyTag {
                name,
                block: missing_block,
            }],
            forms: Vec::new(),
        },
    )?;

    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &["TAG"])?),
        Err(VerifyError::InvalidBlock(missing_block))
    );
    Ok(())
}

#[test]
fn control_flow_edges_reject_foreign_and_missing_references() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(95), 0);
    let foreign_form = FormId::new(CompilationUnitId::new(96), 0);
    let function = ncl_hir::NodeId::try_from_index(form, 0)?;
    let condition = ncl_hir::NodeId::try_from_index(form, 1)?;
    let foreign_block = ncl_hir::BlockId::try_from_index(foreign_form, 0)?;
    let mut builder = ControlFlowBuilder::new(form);
    builder.begin_function(function)?;
    let entry = builder.create_block(function)?;

    assert_eq!(
        builder.set_terminator(
            entry,
            Terminator::Conditional {
                condition,
                if_true: foreign_block,
                if_false: entry,
            },
        ),
        Err(ControlError::ForeignBlock(foreign_block))
    );
    let missing_block = ncl_hir::BlockId::try_from_index(form, 9)?;
    builder.set_terminator(entry, Terminator::Jump(missing_block))?;
    assert!(matches!(
        builder.finish(),
        Err(ControlError::InvalidSuccessor { source, target })
            if source == entry && target == missing_block
    ));

    let form = FormId::new(CompilationUnitId::new(95), 1);
    let foreign_form = FormId::new(CompilationUnitId::new(96), 1);
    let function = ncl_hir::NodeId::try_from_index(form, 0)?;
    let foreign_value = ncl_hir::NodeId::try_from_index(foreign_form, 0)?;
    let mut builder = ControlFlowBuilder::new(form);
    builder.begin_function(function)?;
    let entry = builder.create_block(function)?;
    assert_eq!(
        builder.set_terminator(
            entry,
            Terminator::NonLocalExit {
                target: ExitTarget::Function(function),
                values: vec![foreign_value],
                cleanups: Vec::new(),
            },
        ),
        Err(ControlError::ForeignNode(foreign_value))
    );
    Ok(())
}

#[test]
fn nonlocal_exit_terminator_accepts_resolved_values() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(97), 0);
    let mut builder = form_builder(form)?;
    let scope = builder.root_scope();
    let value = builder.alloc_node(scope, NodeKind::Literal(Literal::True))?;
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
        cfg.set_terminator(
            entry,
            Terminator::NonLocalExit {
                target: ExitTarget::Function(function),
                values: vec![value],
                cleanups: Vec::new(),
            },
        )?;
    }

    let hir = builder.finish()?;
    assert_eq!(hir.verify(&symbol_table(form, &[])?), Ok(()));
    Ok(())
}
