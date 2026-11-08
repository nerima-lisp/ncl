//! Tests for cached values crossing CFG boundaries during lowering.

use std::error::Error;

use ncl_hir::{
    CompilationUnitId, FormId, HirFormBuilder, Literal, NodeKind, SourceFiles, SourceOrigins,
    Terminator,
};

#[test]
fn preheader_value_is_rejected_when_nested_cfg_is_unreachable() -> Result<(), Box<dyn Error>> {
    let compilation_unit = CompilationUnitId::new(0);
    let form_id = FormId::new(compilation_unit, 8);
    let mut files = SourceFiles::new(compilation_unit);
    let file = files.add("nested-control-flow.lisp", "t")?;
    let span = files.span(file, 0, 1)?;
    let mut origins = SourceOrigins::new(form_id);
    let origin = origins.add_source(&files, file, span)?;
    let mut builder = HirFormBuilder::new(form_id, origins, origin)?;
    let scope = builder.root_scope();
    let condition = builder.alloc_node(scope, NodeKind::Literal(Literal::True))?;
    let outer = builder.alloc_node(
        scope,
        NodeKind::Tagbody {
            tags: Vec::new(),
            forms: Vec::new(),
        },
    )?;
    let inner = builder.alloc_node(
        scope,
        NodeKind::Tagbody {
            tags: Vec::new(),
            forms: Vec::new(),
        },
    )?;
    builder.add_root(condition)?;
    let flow = builder.control_flow_mut();
    flow.begin_function(outer)?;
    let outer_entry = flow.create_block(outer)?;
    let outer_unreachable = flow.create_block(outer)?;
    flow.set_terminator(outer_entry, Terminator::Return { values: Vec::new() })?;
    flow.append_operation(outer_unreachable, inner)?;
    flow.set_terminator(outer_unreachable, Terminator::Return { values: Vec::new() })?;

    flow.begin_function(inner)?;
    let inner_entry = flow.create_block(inner)?;
    let if_true = flow.create_block(inner)?;
    let if_false = flow.create_block(inner)?;
    flow.set_terminator(
        inner_entry,
        Terminator::Conditional {
            condition,
            if_true,
            if_false,
        },
    )?;
    flow.set_terminator(if_true, Terminator::Return { values: Vec::new() })?;
    flow.set_terminator(if_false, Terminator::Return { values: Vec::new() })?;
    builder.add_root(outer)?;

    let form = builder.finish()?;
    let symbols = ncl_hir::SymbolTable::new(compilation_unit);
    assert!(form.verify(&symbols).is_ok());
    assert!(matches!(
        ncl_hir::lower::lower(&form, &symbols),
        Err(ncl_hir::lower::LowerError::NodeValueNotAvailableInBlock {
            node: actual_node,
            use_block: Some(actual_block),
            definition_block: None,
        }) if actual_node == condition && actual_block == inner_entry
    ));
    Ok(())
}

#[test]
fn dominating_outer_block_condition_lowers_in_nested_tagbody() -> Result<(), Box<dyn Error>> {
    let compilation_unit = CompilationUnitId::new(0);
    let form_id = FormId::new(compilation_unit, 9);
    let mut files = SourceFiles::new(compilation_unit);
    let file = files.add("nested-control-flow.lisp", "t")?;
    let span = files.span(file, 0, 1)?;
    let mut origins = SourceOrigins::new(form_id);
    let origin = origins.add_source(&files, file, span)?;
    let mut builder = HirFormBuilder::new(form_id, origins, origin)?;
    let scope = builder.root_scope();
    let condition = builder.alloc_node(scope, NodeKind::Literal(Literal::True))?;
    let outer = builder.alloc_node(
        scope,
        NodeKind::Tagbody {
            tags: Vec::new(),
            forms: Vec::new(),
        },
    )?;
    let inner = builder.alloc_node(
        scope,
        NodeKind::Tagbody {
            tags: Vec::new(),
            forms: Vec::new(),
        },
    )?;
    let flow = builder.control_flow_mut();
    flow.begin_function(outer)?;
    let outer_entry = flow.create_block(outer)?;
    flow.append_operation(outer_entry, condition)?;
    flow.append_operation(outer_entry, inner)?;
    flow.set_terminator(outer_entry, Terminator::Return { values: Vec::new() })?;

    flow.begin_function(inner)?;
    let inner_entry = flow.create_block(inner)?;
    let if_true = flow.create_block(inner)?;
    let if_false = flow.create_block(inner)?;
    flow.set_terminator(
        inner_entry,
        Terminator::Conditional {
            condition,
            if_true,
            if_false,
        },
    )?;
    flow.set_terminator(if_true, Terminator::Return { values: Vec::new() })?;
    flow.set_terminator(if_false, Terminator::Return { values: Vec::new() })?;
    builder.add_root(outer)?;

    let form = builder.finish()?;
    let symbols = ncl_hir::SymbolTable::new(compilation_unit);
    assert!(form.verify(&symbols).is_ok());
    let lowered = ncl_hir::lower::lower(&form, &symbols)?;
    assert!(
        lowered
            .entry
            .blocks
            .iter()
            .any(|block| { matches!(&block.terminator, ncl_ir::Terminator::Branch { .. }) })
    );
    Ok(())
}
