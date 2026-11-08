//! Tests for the verified HIR-to-MIR lowering boundary.

use std::error::Error;

use ncl_hir::{
    CompilationUnitId, FormId, HirFormBuilder, Literal, NodeKind, SourceFiles, SourceOrigins,
    SymbolTable, Terminator as HirTerminator, ValueId,
};
use ncl_ir::{ArrayElementType, OpKind, Prim, StructureKind, Terminator as MirTerminator};

#[test]
fn check_type_lowers_a_dominating_producer_and_runtime_failure_path() -> Result<(), Box<dyn Error>>
{
    let compilation_unit = CompilationUnitId::new(0);
    let form_id = FormId::new(compilation_unit, 0);
    let mut files = SourceFiles::new(compilation_unit);
    let file = files.add("check-type.lisp", "nil")?;
    let span = files.span(file, 0, 3)?;
    let mut origins = SourceOrigins::new(form_id);
    let origin = origins.add_source(&files, file, span)?;
    let mut builder = HirFormBuilder::new(form_id, origins, origin)?;
    let scope = builder.root_scope();
    let producer = builder.alloc_node(scope, NodeKind::Literal(Literal::Nil))?;
    let value = ValueId::try_from_index(form_id, 0)?;
    builder.register_value_producer(value, producer)?;
    let expected_type = builder.add_type_spec_constant(Literal::True)?;
    let accepted_type = builder.types_mut().any();
    let check = builder.alloc_node(
        scope,
        NodeKind::CheckType {
            value,
            accepted_type,
            expected_type,
            source_origin: origin,
        },
    )?;
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
    let check_block = flow.create_block(tagbody)?;
    let producer_block = flow.create_block(tagbody)?;
    let success = flow.create_block(tagbody)?;
    let failure = flow.create_block(tagbody)?;
    flow.append_operation(producer_block, producer)?;
    flow.set_terminator(entry, HirTerminator::Jump(producer_block))?;
    flow.set_terminator(producer_block, HirTerminator::Jump(check_block))?;
    flow.set_terminator(
        check_block,
        HirTerminator::Conditional {
            condition: check,
            if_true: success,
            if_false: failure,
        },
    )?;
    flow.set_terminator(success, HirTerminator::Return { values: Vec::new() })?;
    flow.set_terminator(failure, HirTerminator::Return { values: Vec::new() })?;
    builder.add_root(tagbody)?;

    let form = builder.finish()?;
    let symbols = SymbolTable::new(compilation_unit);
    // HIR's SSA contract requires dominance, not same-block placement (docs/src/design/hir.md).
    let lowered = ncl_hir::lower::lower(&form, &symbols)?;
    assert!(lowered.entry.blocks.iter().any(|block| {
        block.ops.iter().any(|operation| {
            matches!(
                &operation.kind,
                OpKind::Prim {
                    op: Prim::Typep,
                    ..
                }
            )
        })
    }));
    assert!(lowered.entry.blocks.iter().any(|block| {
        block.ops.iter().any(|operation| {
            matches!(
                &operation.kind,
                OpKind::Builtin { name, args, .. }
                    if name == "ncl_signal_type_error" && args.len() == 3
            )
        })
    }));
    assert!(
        lowered
            .entry
            .blocks
            .iter()
            .any(|block| matches!(&block.terminator, MirTerminator::Throw { .. }))
    );
    assert!(
        lowered
            .entry
            .constants
            .iter()
            .any(|constant| matches!(constant, ncl_ir::Constant::Fixnum(0)))
    );
    assert!(
        lowered
            .entry
            .constants
            .iter()
            .filter(|constant| **constant == ncl_ir::Constant::T)
            .count()
            >= 2
    );
    Ok(())
}

#[test]
fn cached_preheader_condition_lowers_in_a_reachable_cfg_block() -> Result<(), Box<dyn Error>> {
    let compilation_unit = CompilationUnitId::new(0);
    let form_id = FormId::new(compilation_unit, 7);
    let mut files = SourceFiles::new(compilation_unit);
    let file = files.add("control-flow.lisp", "t")?;
    let span = files.span(file, 0, 1)?;
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
    builder.add_root(condition)?;
    let flow = builder.control_flow_mut();
    flow.begin_function(tagbody)?;
    let entry = flow.create_block(tagbody)?;
    let if_true = flow.create_block(tagbody)?;
    let if_false = flow.create_block(tagbody)?;
    flow.set_terminator(
        entry,
        HirTerminator::Conditional {
            condition,
            if_true,
            if_false,
        },
    )?;
    flow.set_terminator(if_true, HirTerminator::Return { values: Vec::new() })?;
    flow.set_terminator(if_false, HirTerminator::Return { values: Vec::new() })?;
    builder.add_root(tagbody)?;

    let form = builder.finish()?;
    let symbols = SymbolTable::new(compilation_unit);
    let lowered = ncl_hir::lower::lower(&form, &symbols)?;
    assert!(
        lowered
            .entry
            .blocks
            .iter()
            .any(|block| { matches!(&block.terminator, MirTerminator::Branch { .. }) })
    );
    Ok(())
}

#[test]
fn check_type_rejects_a_value_missing_from_one_join_predecessor() -> Result<(), Box<dyn Error>> {
    let compilation_unit = CompilationUnitId::new(0);
    let form_id = FormId::new(compilation_unit, 4);
    let mut files = SourceFiles::new(compilation_unit);
    let file = files.add("check-type.lisp", "nil")?;
    let span = files.span(file, 0, 3)?;
    let mut origins = SourceOrigins::new(form_id);
    let origin = origins.add_source(&files, file, span)?;
    let mut builder = HirFormBuilder::new(form_id, origins, origin)?;
    let scope = builder.root_scope();
    let producer = builder.alloc_node(scope, NodeKind::Literal(Literal::Nil))?;
    let value = ValueId::try_from_index(form_id, 0)?;
    builder.register_value_producer(value, producer)?;
    let expected_type = builder.add_type_spec_constant(Literal::True)?;
    let accepted_type = builder.types_mut().any();
    let check = builder.alloc_node(
        scope,
        NodeKind::CheckType {
            value,
            accepted_type,
            expected_type,
            source_origin: origin,
        },
    )?;
    let branch = builder.alloc_node(scope, NodeKind::Literal(Literal::True))?;
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
    let left = flow.create_block(tagbody)?;
    let right = flow.create_block(tagbody)?;
    let join = flow.create_block(tagbody)?;
    let success = flow.create_block(tagbody)?;
    let failure = flow.create_block(tagbody)?;
    flow.set_terminator(
        entry,
        HirTerminator::Conditional {
            condition: branch,
            if_true: left,
            if_false: right,
        },
    )?;
    flow.append_operation(left, producer)?;
    flow.set_terminator(left, HirTerminator::Jump(join))?;
    flow.set_terminator(right, HirTerminator::Jump(join))?;
    flow.set_terminator(
        join,
        HirTerminator::Conditional {
            condition: check,
            if_true: success,
            if_false: failure,
        },
    )?;
    flow.set_terminator(success, HirTerminator::Return { values: Vec::new() })?;
    flow.set_terminator(failure, HirTerminator::Return { values: Vec::new() })?;
    builder.add_root(tagbody)?;

    let form = builder.finish()?;
    let symbols = SymbolTable::new(compilation_unit);
    // Only the left predecessor defines this value, so it cannot reach the join (docs/src/design/hir.md).
    assert!(matches!(
        form.verify(&symbols),
        Err(ncl_hir::verify::VerifyError::InvalidValueProducer {
            check: actual_check,
            producer: actual_producer,
        }) if actual_check == check && actual_producer == producer
    ));
    Ok(())
}

#[test]
fn check_type_rejects_a_producer_from_a_non_dominating_block() -> Result<(), Box<dyn Error>> {
    let compilation_unit = CompilationUnitId::new(0);
    let form_id = FormId::new(compilation_unit, 3);
    let mut files = SourceFiles::new(compilation_unit);
    let file = files.add("check-type.lisp", "nil")?;
    let span = files.span(file, 0, 3)?;
    let mut origins = SourceOrigins::new(form_id);
    let origin = origins.add_source(&files, file, span)?;
    let mut builder = HirFormBuilder::new(form_id, origins, origin)?;
    let scope = builder.root_scope();
    let producer = builder.alloc_node(scope, NodeKind::Literal(Literal::Nil))?;
    let value = ValueId::try_from_index(form_id, 0)?;
    builder.register_value_producer(value, producer)?;
    let expected_type = builder.add_type_spec_constant(Literal::True)?;
    let accepted_type = builder.types_mut().any();
    let check = builder.alloc_node(
        scope,
        NodeKind::CheckType {
            value,
            accepted_type,
            expected_type,
            source_origin: origin,
        },
    )?;
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
    let check_block = flow.create_block(tagbody)?;
    let success = flow.create_block(tagbody)?;
    let failure = flow.create_block(tagbody)?;
    flow.set_terminator(entry, HirTerminator::Jump(check_block))?;
    flow.append_operation(success, producer)?;
    flow.set_terminator(
        check_block,
        HirTerminator::Conditional {
            condition: check,
            if_true: success,
            if_false: failure,
        },
    )?;
    flow.set_terminator(success, HirTerminator::Return { values: Vec::new() })?;
    flow.set_terminator(failure, HirTerminator::Return { values: Vec::new() })?;
    builder.add_root(tagbody)?;

    let form = builder.finish()?;
    let symbols = SymbolTable::new(compilation_unit);
    assert!(matches!(
        form.verify(&symbols),
        Err(ncl_hir::verify::VerifyError::InvalidValueProducer {
            check: actual_check,
            producer: actual_producer,
        }) if actual_check == check && actual_producer == producer
    ));
    Ok(())
}

#[test]
fn cached_preheader_value_is_rejected_in_unreachable_check_block() -> Result<(), Box<dyn Error>> {
    let compilation_unit = CompilationUnitId::new(0);
    let form_id = FormId::new(compilation_unit, 5);
    let mut files = SourceFiles::new(compilation_unit);
    let file = files.add("check-type.lisp", "nil")?;
    let span = files.span(file, 0, 3)?;
    let mut origins = SourceOrigins::new(form_id);
    let origin = origins.add_source(&files, file, span)?;
    let mut builder = HirFormBuilder::new(form_id, origins, origin)?;
    let scope = builder.root_scope();
    let producer = builder.alloc_node(scope, NodeKind::Literal(Literal::Nil))?;
    let value = ValueId::try_from_index(form_id, 0)?;
    builder.register_value_producer(value, producer)?;
    let expected_type = builder.add_type_spec_constant(Literal::True)?;
    let accepted_type = builder.types_mut().any();
    let check = builder.alloc_node(
        scope,
        NodeKind::CheckType {
            value,
            accepted_type,
            expected_type,
            source_origin: origin,
        },
    )?;
    let tagbody = builder.alloc_node(
        scope,
        NodeKind::Tagbody {
            tags: Vec::new(),
            forms: Vec::new(),
        },
    )?;
    builder.add_root(producer)?;
    let flow = builder.control_flow_mut();
    flow.begin_function(tagbody)?;
    let entry = flow.create_block(tagbody)?;
    let unreachable = flow.create_block(tagbody)?;
    let success = flow.create_block(tagbody)?;
    let failure = flow.create_block(tagbody)?;
    flow.set_terminator(entry, HirTerminator::Return { values: Vec::new() })?;
    flow.append_operation(unreachable, producer)?;
    flow.set_terminator(
        unreachable,
        HirTerminator::Conditional {
            condition: check,
            if_true: success,
            if_false: failure,
        },
    )?;
    flow.set_terminator(success, HirTerminator::Return { values: Vec::new() })?;
    flow.set_terminator(failure, HirTerminator::Return { values: Vec::new() })?;
    builder.add_root(tagbody)?;

    let form = builder.finish()?;
    let symbols = SymbolTable::new(compilation_unit);
    assert!(form.verify(&symbols).is_ok());
    assert!(matches!(
        ncl_hir::lower::lower(&form, &symbols),
        Err(ncl_hir::lower::LowerError::NodeValueNotAvailableInBlock {
            node: actual_node,
            use_block: Some(actual_block),
            definition_block: None,
        }) if actual_node == producer && actual_block == unreachable
    ));
    Ok(())
}

#[test]
fn specialized_general_array_lowers_with_general_element_storage() -> Result<(), Box<dyn Error>> {
    let compilation_unit = CompilationUnitId::new(0);
    let form_id = FormId::new(compilation_unit, 1);
    let mut files = SourceFiles::new(compilation_unit);
    let file = files.add("array.lisp", "#()")?;
    let span = files.span(file, 0, 3)?;
    let mut origins = SourceOrigins::new(form_id);
    let origin = origins.add_source(&files, file, span)?;
    let mut builder = HirFormBuilder::new(form_id, origins, origin)?;
    let array = builder.alloc_node(
        builder.root_scope(),
        NodeKind::Literal(Literal::SpecializedArray {
            identity: 1,
            element_type: ncl_hir::LiteralElementType::T,
            elements: Vec::new(),
        }),
    )?;
    builder.add_root(array)?;

    let form = builder.finish()?;
    let symbols = SymbolTable::new(compilation_unit);
    // CLHS 15.1.2 defines general arrays as accepting any object; `T` needs general element storage.
    // https://www.lispworks.com/documentation/HyperSpec/Body/15_ab.htm
    let lowered = ncl_hir::lower::lower(&form, &symbols)?;
    assert!(lowered.entry.constants.iter().any(|constant| matches!(
        constant,
        ncl_ir::Constant::Structure {
            kind: StructureKind::Array {
                dimensions,
                element_type: ArrayElementType::T,
            },
            ..
        } if dimensions == &[0]
    )));
    Ok(())
}

#[test]
fn lambda_without_signature_metadata_lowers_zero_required_parameters() -> Result<(), Box<dyn Error>>
{
    let compilation_unit = CompilationUnitId::new(0);
    let form_id = FormId::new(compilation_unit, 2);
    let mut files = SourceFiles::new(compilation_unit);
    let file = files.add("lambda.lisp", "(lambda () t)")?;
    let span = files.span(file, 0, 13)?;
    let mut origins = SourceOrigins::new(form_id);
    let origin = origins.add_source(&files, file, span)?;
    let mut builder = HirFormBuilder::new(form_id, origins, origin)?;
    let scope = builder.root_scope();
    let body = builder.alloc_node(scope, NodeKind::Literal(Literal::True))?;
    let lambda = builder.alloc_node(
        scope,
        NodeKind::Lambda {
            parameters: Vec::new(),
            body_scope: scope,
            body: vec![body],
        },
    )?;
    builder.add_root(lambda)?;

    let form = builder.finish()?;
    let symbols = SymbolTable::new(compilation_unit);
    // CLHS 3.4 defines the lambda list; `()` has zero required Lisp parameters. NCL passes `argc` separately.
    // https://www.lispworks.com/documentation/HyperSpec/Body/03_d.htm
    let lowered = ncl_hir::lower::lower(&form, &symbols)?;
    let [nested] = lowered.nested.as_slice() else {
        return Err("expected one nested lambda function".into());
    };
    assert_eq!(nested.params.len(), 1);
    let Some(argc) = nested.params.first() else {
        return Err("expected an argument-count parameter".into());
    };
    assert_eq!(argc.name, "argc");
    Ok(())
}
