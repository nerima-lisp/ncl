//! Tests for HIR control-flow dominance.

use std::error::Error;

use ncl_hir::{
    CompilationUnitId, FormId, HirFormBuilder, Literal, NodeKind, SourceFiles, SourceOrigins,
    Terminator,
};

#[test]
fn loop_header_dominates_its_body_and_exit() -> Result<(), Box<dyn Error>> {
    let compilation_unit = CompilationUnitId::new(0);
    let form_id = FormId::new(compilation_unit, 6);
    let mut files = SourceFiles::new(compilation_unit);
    let file = files.add("dominance.lisp", "(tagbody)")?;
    let span = files.span(file, 0, 9)?;
    let mut origins = SourceOrigins::new(form_id);
    let origin = origins.add_source(&files, file, span)?;
    let mut builder = HirFormBuilder::new(form_id, origins, origin)?;
    let scope = builder.root_scope();
    let condition = builder.alloc_node(scope, NodeKind::Literal(Literal::True))?;
    let tagbody = builder.alloc_node(
        scope,
        NodeKind::Tagbody {
            tags: Vec::new(),
            forms: Vec::new(),
        },
    )?;
    let flow = builder.control_flow_mut();
    flow.begin_function(tagbody)?;
    let entry = flow.create_block(tagbody)?;
    let header = flow.create_block(tagbody)?;
    let body = flow.create_block(tagbody)?;
    let exit = flow.create_block(tagbody)?;
    flow.set_terminator(entry, Terminator::Jump(header))?;
    flow.set_terminator(
        header,
        Terminator::Conditional {
            condition,
            if_true: body,
            if_false: exit,
        },
    )?;
    flow.set_terminator(body, Terminator::Jump(header))?;
    flow.set_terminator(exit, Terminator::Return { values: Vec::new() })?;
    builder.add_root(tagbody)?;

    let form = builder.finish()?;
    // The HIR CFG contract requires dominance and loop analysis (docs/src/design/hir.md).
    let Some(dominators) = form.control_flow().dominators(tagbody) else {
        return Err("missing dominator sets".into());
    };
    assert!(matches!(
        dominators.get(&entry),
        Some(blocks) if blocks.len() == 1 && blocks.contains(&entry)
    ));
    assert!(matches!(
        dominators.get(&header),
        Some(blocks) if blocks.len() == 2 && blocks.contains(&entry) && blocks.contains(&header) && !blocks.contains(&body)
    ));
    assert!(matches!(
        dominators.get(&body),
        Some(blocks) if blocks.len() == 3 && blocks.contains(&entry) && blocks.contains(&header) && blocks.contains(&body)
    ));
    assert!(matches!(
        dominators.get(&exit),
        Some(blocks) if blocks.len() == 3 && blocks.contains(&entry) && blocks.contains(&header) && blocks.contains(&exit)
    ));
    Ok(())
}
