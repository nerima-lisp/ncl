//! Tests for HIR multiple-value lowering.

use std::error::Error;

use ncl_hir::binding::BindingUseKind;
use ncl_hir::{
    CompilationUnitId, FormId, HirFormBuilder, Literal, MultipleValueOperation, NodeKind,
    SourceFiles, SourceOrigins, SymbolTable, Terminator as HirTerminator,
};
use ncl_ir::{Constant, Function, OpKind, Prim};

fn assert_source_order_results(function: &Function) {
    assert!(function.constants.contains(&Constant::T));
    assert!(function.constants.contains(&Constant::Nil));
    let cars = function
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter(|operation| matches!(&operation.kind, OpKind::Prim { op: Prim::Car, .. }))
        .filter_map(|operation| operation.results.first().map(|(value, _)| *value))
        .collect::<Vec<_>>();
    let cdr_count = function
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter(|operation| matches!(&operation.kind, OpKind::Prim { op: Prim::Cdr, .. }))
        .count();
    assert_eq!(cars.len(), 2);
    assert_eq!(cdr_count, 2);
    assert!(function.blocks.iter().flat_map(|block| &block.ops).any(
        |operation| matches!(&operation.kind, OpKind::SetMultipleValues { values } if values == &cars)
    ));
}

#[test]
fn multiple_value_bind_assigns_values_in_source_order() -> Result<(), Box<dyn Error>> {
    let compilation_unit = CompilationUnitId::new(12);
    let form_id = FormId::new(compilation_unit, 0);
    let mut files = SourceFiles::new(compilation_unit);
    let file = files.add(
        "multiple-values.lisp",
        "(multiple-value-bind (a b) x (values a b))",
    )?;
    let span = files.span(file, 0, 42)?;
    let mut origins = SourceOrigins::new(form_id);
    let origin = origins.add_source(&files, file, span)?;
    let mut builder = HirFormBuilder::new(form_id, origins, origin)?;
    let root = builder.root_scope();
    let body_scope = builder.bindings_mut().push_scope(root)?;
    let mut symbols = SymbolTable::new(compilation_unit);
    let first_symbol = symbols.intern(form_id, Some("COMMON-LISP-USER"), "FIRST-VALUE", None)?;
    let second_symbol = symbols.intern(form_id, Some("COMMON-LISP-USER"), "SECOND-VALUE", None)?;
    builder.register_symbol(first_symbol)?;
    builder.register_symbol(second_symbol)?;
    let first_binding = builder
        .bindings_mut()
        .bind_variable(body_scope, first_symbol)?;
    let second_binding = builder
        .bindings_mut()
        .bind_variable(body_scope, second_symbol)?;

    let first_value = builder.alloc_node(root, NodeKind::Literal(Literal::True))?;
    let second_value = builder.alloc_node(root, NodeKind::Literal(Literal::Nil))?;
    let values = builder.alloc_node(
        root,
        NodeKind::MultipleValues(MultipleValueOperation::Produce {
            values: vec![first_value, second_value],
        }),
    )?;
    let first_reference = builder.alloc_node(
        body_scope,
        NodeKind::VariableReference {
            target: ncl_hir::VariableTarget::Lexical(first_binding),
        },
    )?;
    let second_reference = builder.alloc_node(
        body_scope,
        NodeKind::VariableReference {
            target: ncl_hir::VariableTarget::Lexical(second_binding),
        },
    )?;
    builder
        .bindings_mut()
        .record_use(first_binding, first_reference, BindingUseKind::Read)?;
    builder
        .bindings_mut()
        .record_use(second_binding, second_reference, BindingUseKind::Read)?;
    let body_values = builder.alloc_node(
        body_scope,
        NodeKind::MultipleValues(MultipleValueOperation::Produce {
            values: vec![first_reference, second_reference],
        }),
    )?;
    let bind = builder.alloc_node(
        root,
        NodeKind::MultipleValues(MultipleValueOperation::Bind {
            value: values,
            bindings: vec![first_binding, second_binding],
            body_scope,
            body: vec![body_values],
        }),
    )?;
    let function = builder.alloc_node(
        root,
        NodeKind::Tagbody {
            tags: Vec::new(),
            forms: Vec::new(),
        },
    )?;
    let flow = builder.control_flow_mut();
    flow.begin_function(function)?;
    let entry = flow.create_block(function)?;
    flow.append_operation(entry, bind)?;
    flow.set_terminator(entry, HirTerminator::Return { values: Vec::new() })?;
    builder.add_root(function)?;

    let form = builder.finish()?;
    let lowered = ncl_hir::lower::lower(&form, &symbols)?;

    // CLHS MULTIPLE-VALUE-BIND assigns each variable its corresponding returned value.
    // https://www.lispworks.com/documentation/HyperSpec/Body/m_multip.htm
    assert_source_order_results(&lowered.entry);
    Ok(())
}
