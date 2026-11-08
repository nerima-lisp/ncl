//! Public form construction exercises verifier paths missed by the existing fixtures.

#![deny(
    unreachable_pub,
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::as_conversions
)]

use std::collections::{HashMap, HashSet};
use std::error::Error;

use ncl_hir::binding::LocalFunctionKind;
use ncl_hir::cl_type::BuiltinType;
use ncl_hir::control::{ExitTarget, Terminator};
use ncl_hir::node::{
    CallTarget, LetBinding, LetKind, LocalFunctionDefinition, MultipleValueOperation, NodeKind,
    TagbodyTag,
};
use ncl_hir::verify::{FactEpochs, VerifyInput, VerifyResources, verify};
use ncl_hir::{
    CleanupId, CompilationUnitId, FormId, HirForm, HirFormBuilder, Literal, SourceFiles,
    SourceOriginId, SourceOrigins, SymbolId, SymbolTable, TypeId, TypeSpecConstId, ValueId,
    VerifyError,
};

fn form_builder(form: FormId) -> Result<HirFormBuilder, Box<dyn Error>> {
    let mut files = SourceFiles::new(form.compilation_unit());
    let file = files.add("verifier-coverage-followup.lisp", "x")?;
    let span = files.span(file, 0, 1)?;
    let mut origins = SourceOrigins::new(form);
    let origin = origins.add_source(&files, file, span)?;
    Ok(HirFormBuilder::new(form, origins, origin)?)
}

fn symbol_table(form: FormId, names: &[&str]) -> Result<SymbolTable, Box<dyn Error>> {
    let mut symbols = SymbolTable::new(form.compilation_unit());
    for name in names {
        symbols.intern(form, Some("CL"), name, None)?;
    }
    Ok(symbols)
}

fn verify_with_resources(
    hir: &HirForm,
    form: FormId,
    resources: VerifyResources<'_>,
) -> Result<(), VerifyError> {
    verify(VerifyInput {
        form,
        nodes: hir.nodes(),
        types: hir.types(),
        bindings: hir.bindings(),
        control_flow: hir.control_flow(),
        origins: hir.origins(),
        facts: hir.facts(),
        epochs: FactEpochs::default(),
        policy: hir.policy(),
        resources,
    })
}

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
fn lexical_references_reject_bindings_hidden_in_sibling_scopes() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(79), 0);
    let mut builder = form_builder(form)?;
    let root = builder.root_scope();
    let child = builder.bindings_mut().push_scope(root)?;
    let name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    builder.register_symbol(name)?;
    let binding = builder.bindings_mut().bind_variable(child, name)?;
    let reference = builder.alloc_node(
        root,
        NodeKind::VariableReference {
            target: ncl_hir::node::VariableTarget::Lexical(binding),
        },
    )?;

    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &["VARIABLE"])?),
        Err(VerifyError::InvisibleVariable {
            node: reference,
            binding,
        })
    );
    Ok(())
}

#[test]
fn calls_reject_arguments_outside_the_call_scope() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(80), 0);
    let mut builder = form_builder(form)?;
    let root = builder.root_scope();
    let child_scope = builder.bindings_mut().push_scope(root)?;
    let name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    builder.register_symbol(name)?;
    let argument = builder.alloc_node(child_scope, NodeKind::Literal(Literal::True))?;
    builder.alloc_node(
        root,
        NodeKind::Call {
            target: CallTarget::Global(name),
            arguments: vec![argument],
        },
    )?;

    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &["FUNCTION"])?),
        Err(VerifyError::InvalidScope(child_scope))
    );
    Ok(())
}

#[test]
fn local_function_references_must_be_visible_at_their_use_site() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(81), 0);
    let mut builder = form_builder(form)?;
    let root = builder.root_scope();
    let child = builder.bindings_mut().push_scope(root)?;
    let name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    builder.register_symbol(name)?;
    let group =
        builder
            .bindings_mut()
            .bind_local_functions(child, LocalFunctionKind::Flet, &[name])?;
    let definition = group
        .definitions
        .first()
        .copied()
        .ok_or("FLET definition was not created")?;
    let reference = builder.alloc_node(
        root,
        NodeKind::FunctionReference {
            target: CallTarget::Local(definition.binding),
        },
    )?;

    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &["LOCAL-FUNCTION"])?),
        Err(VerifyError::InvisibleFunction {
            node: reference,
            binding: definition.binding,
        })
    );
    Ok(())
}

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
fn semantic_node_references_reject_missing_symbols_nodes_blocks_scopes_and_values()
-> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(84), 0);
    let mut builder = form_builder(form)?;
    let missing_symbol = SymbolId::try_from_index(form.compilation_unit(), 4)?;
    builder.alloc_node(
        builder.root_scope(),
        NodeKind::Literal(Literal::Symbol(missing_symbol)),
    )?;
    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &[])?),
        Err(VerifyError::InvalidSymbol(missing_symbol))
    );

    let form = FormId::new(CompilationUnitId::new(84), 1);
    let mut builder = form_builder(form)?;
    let missing_node = ncl_hir::NodeId::try_from_index(form, 9)?;
    let function = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    builder.register_symbol(function)?;
    builder.alloc_node(
        builder.root_scope(),
        NodeKind::Call {
            target: CallTarget::Global(function),
            arguments: vec![missing_node],
        },
    )?;
    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &["FUNCTION"])?),
        Err(VerifyError::InvalidNode(missing_node))
    );

    let form = FormId::new(CompilationUnitId::new(84), 2);
    let mut builder = form_builder(form)?;
    let missing_block = ncl_hir::BlockId::try_from_index(form, 9)?;
    builder.alloc_node(
        builder.root_scope(),
        NodeKind::Go {
            target: missing_block,
        },
    )?;
    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &[])?),
        Err(VerifyError::InvalidBlock(missing_block))
    );

    let form = FormId::new(CompilationUnitId::new(84), 3);
    let mut builder = form_builder(form)?;
    let parent = builder.root_scope();
    let node_scope = builder.bindings_mut().push_scope(parent)?;
    let body_scope = builder.bindings_mut().push_scope(parent)?;
    builder.alloc_node(
        node_scope,
        NodeKind::Sequence {
            body_scope,
            forms: Vec::new(),
        },
    )?;
    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &[])?),
        Err(VerifyError::InvalidScope(body_scope))
    );

    let form = FormId::new(CompilationUnitId::new(84), 4);
    let mut builder = form_builder(form)?;
    let missing_value = ValueId::try_from_index(form, 0)?;
    let accepted_type = builder.types_mut().named(BuiltinType::Integer)?;
    let expected_type = builder.add_type_spec_constant(Literal::True)?;
    let source_origin = SourceOriginId::try_from_index(form, 0)?;
    builder.alloc_node(
        builder.root_scope(),
        NodeKind::CheckType {
            value: missing_value,
            accepted_type,
            expected_type,
            source_origin,
        },
    )?;
    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &[])?),
        Err(VerifyError::InvalidValue(missing_value))
    );
    Ok(())
}

#[test]
fn multiple_value_variants_accept_well_scoped_operands() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(85), 0);
    let mut builder = form_builder(form)?;
    let root = builder.root_scope();
    let body_scope = builder.bindings_mut().push_scope(root)?;
    let name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    builder.register_symbol(name)?;
    let binding = builder.bindings_mut().bind_variable(body_scope, name)?;
    let value = builder.alloc_node(root, NodeKind::Literal(Literal::True))?;
    builder.alloc_node(
        root,
        NodeKind::MultipleValues(MultipleValueOperation::Produce {
            values: vec![value],
        }),
    )?;
    builder.alloc_node(
        root,
        NodeKind::MultipleValues(MultipleValueOperation::Return {
            values: vec![value],
        }),
    )?;
    builder.alloc_node(
        root,
        NodeKind::MultipleValues(MultipleValueOperation::Prog1 {
            first: value,
            rest: vec![value],
        }),
    )?;
    builder.alloc_node(
        root,
        NodeKind::MultipleValues(MultipleValueOperation::Bind {
            value,
            bindings: vec![binding],
            body_scope,
            body: Vec::new(),
        }),
    )?;
    builder.alloc_node(
        root,
        NodeKind::MultipleValues(MultipleValueOperation::Call {
            function: value,
            arguments: vec![value],
        }),
    )?;

    let hir = builder.finish()?;
    assert_eq!(hir.verify(&symbol_table(form, &["VALUE"])?), Ok(()));
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

#[test]
fn multiple_value_bind_requires_a_distinct_body_scope_for_bindings() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(75), 0);
    let mut builder = form_builder(form)?;
    let scope = builder.root_scope();
    let name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    builder.register_symbol(name)?;
    let binding = builder.bindings_mut().bind_variable(scope, name)?;
    let value = builder.alloc_node(scope, NodeKind::Literal(Literal::Nil))?;
    let operation = builder.alloc_node(
        scope,
        NodeKind::MultipleValues(MultipleValueOperation::Bind {
            value,
            bindings: vec![binding],
            body_scope: scope,
            body: Vec::new(),
        }),
    )?;

    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &["VALUE"])?),
        Err(VerifyError::InvalidBindingScopes(operation))
    );
    Ok(())
}

#[test]
fn parallel_let_bindings_must_be_lexical_variables() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(76), 0);
    let mut builder = form_builder(form)?;
    let parent = builder.root_scope();
    let body_scope = builder.bindings_mut().push_scope(parent)?;
    let name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    builder.register_symbol(name)?;
    let binding = builder.bindings_mut().bind_function(body_scope, name)?;
    builder.alloc_node(
        parent,
        NodeKind::Let {
            kind: LetKind::Parallel,
            bindings: vec![LetBinding {
                binding,
                initializer: None,
            }],
            initializer_scopes: vec![parent],
            binding_scopes: vec![body_scope],
            body_scope,
            body: Vec::new(),
        },
    )?;

    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &["FUNCTION"])?),
        Err(VerifyError::InvalidBinding(binding))
    );
    Ok(())
}

#[test]
fn verify_input_requires_matching_arena_and_resource_owners() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(86), 0);
    let builder = form_builder(form)?;
    let hir = builder.finish()?;
    let values = HashSet::new();
    let producers = HashMap::new();
    let constants = hir.type_spec_constant_ids();
    let valid_resources = VerifyResources::new(
        form.compilation_unit(),
        hir.symbol_ids(),
        &values,
        &producers,
        constants,
    );
    assert_eq!(verify_with_resources(&hir, form, valid_resources), Ok(()));

    let foreign_form = FormId::new(form.compilation_unit(), 1);
    let mismatched_form_resources = VerifyResources::new(
        form.compilation_unit(),
        hir.symbol_ids(),
        &values,
        &producers,
        constants,
    );
    assert_eq!(
        verify_with_resources(&hir, foreign_form, mismatched_form_resources),
        Err(VerifyError::WrongForm)
    );

    let foreign_symbol = SymbolId::try_from_index(CompilationUnitId::new(87), 0)?;
    let foreign_symbols = HashSet::from([foreign_symbol]);
    let mismatched_resource_resources = VerifyResources::new(
        form.compilation_unit(),
        &foreign_symbols,
        &values,
        &producers,
        constants,
    );
    assert_eq!(
        verify_with_resources(&hir, form, mismatched_resource_resources),
        Err(VerifyError::WrongForm)
    );
    Ok(())
}

#[test]
fn verify_input_requires_each_value_producer_to_be_in_the_value_table() -> Result<(), Box<dyn Error>>
{
    let form = FormId::new(CompilationUnitId::new(88), 0);
    let mut builder = form_builder(form)?;
    let producer = builder.alloc_node(builder.root_scope(), NodeKind::Literal(Literal::True))?;
    let hir = builder.finish()?;
    let value = ValueId::try_from_index(form, 0)?;
    let values = HashSet::new();
    let producers = HashMap::from([(value, producer)]);
    let resources = VerifyResources::new(
        form.compilation_unit(),
        hir.symbol_ids(),
        &values,
        &producers,
        hir.type_spec_constant_ids(),
    );

    assert_eq!(
        verify_with_resources(&hir, form, resources),
        Err(VerifyError::InvalidValue(value))
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
fn lambda_parameters_require_a_distinct_body_scope() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(90), 0);
    let mut builder = form_builder(form)?;
    let scope = builder.root_scope();
    let name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    builder.register_symbol(name)?;
    let parameter = builder.bindings_mut().bind_variable(scope, name)?;
    let lambda = builder.alloc_node(
        scope,
        NodeKind::Lambda {
            parameters: vec![parameter],
            body_scope: scope,
            body: Vec::new(),
        },
    )?;

    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &["PARAMETER"])?),
        Err(VerifyError::InvalidBindingScopes(lambda))
    );
    Ok(())
}

#[test]
fn let_initializer_node_must_match_its_declared_scope() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(91), 0);
    let mut builder = form_builder(form)?;
    let parent = builder.root_scope();
    let body_scope = builder.bindings_mut().push_scope(parent)?;
    let name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    builder.register_symbol(name)?;
    let binding = builder.bindings_mut().bind_variable(body_scope, name)?;
    let initializer = builder.alloc_node(body_scope, NodeKind::Literal(Literal::True))?;
    let let_node = builder.alloc_node(
        parent,
        NodeKind::Let {
            kind: LetKind::Parallel,
            bindings: vec![LetBinding {
                binding,
                initializer: Some(initializer),
            }],
            initializer_scopes: vec![parent],
            binding_scopes: vec![body_scope],
            body_scope,
            body: Vec::new(),
        },
    )?;

    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &["VARIABLE"])?),
        Err(VerifyError::InvalidBindingScopes(let_node))
    );
    Ok(())
}

#[test]
fn local_function_definitions_must_preserve_their_binding_scopes() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(92), 0);
    let mut builder = form_builder(form)?;
    let scope = builder.root_scope();
    let name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    builder.register_symbol(name)?;
    let group =
        builder
            .bindings_mut()
            .bind_local_functions(scope, LocalFunctionKind::Flet, &[name])?;
    let definition = group
        .definitions
        .first()
        .copied()
        .ok_or("FLET definition was not created")?;
    let collapsed = builder.alloc_node(
        scope,
        NodeKind::LocalFunctions {
            kind: LocalFunctionKind::Flet,
            body_scope: scope,
            definitions: vec![LocalFunctionDefinition {
                binding: definition.binding,
                definition_scope: definition.definition_scope,
                parameters: Vec::new(),
                body_scope: scope,
                body: Vec::new(),
            }],
            body: Vec::new(),
        },
    )?;
    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &["LOCAL-FUNCTION"])?),
        Err(VerifyError::InvalidBindingScopes(collapsed))
    );

    let form = FormId::new(CompilationUnitId::new(92), 1);
    let mut builder = form_builder(form)?;
    let scope = builder.root_scope();
    let name = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    builder.register_symbol(name)?;
    let group =
        builder
            .bindings_mut()
            .bind_local_functions(scope, LocalFunctionKind::Flet, &[name])?;
    let definition = group
        .definitions
        .first()
        .copied()
        .ok_or("FLET definition was not created")?;
    let wrong_definition_scope = builder.alloc_node(
        scope,
        NodeKind::LocalFunctions {
            kind: LocalFunctionKind::Flet,
            body_scope: group.body_scope,
            definitions: vec![LocalFunctionDefinition {
                binding: definition.binding,
                definition_scope: group.body_scope,
                parameters: Vec::new(),
                body_scope: group.body_scope,
                body: Vec::new(),
            }],
            body: Vec::new(),
        },
    )?;
    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &["LOCAL-FUNCTION"])?),
        Err(VerifyError::InvalidBindingScopes(wrong_definition_scope))
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
