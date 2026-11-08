//! Public form construction rejects invalid verifier inputs and preserves valid metadata.

#![deny(
    unreachable_pub,
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::as_conversions
)]

use std::error::Error;

use ncl_hir::binding::LocalFunctionKind;
use ncl_hir::cl_type::BuiltinType;
use ncl_hir::control::{ExitTarget, Terminator};
use ncl_hir::node::{
    CallTarget, LetBinding, LetKind, LocalFunctionDefinition, NodeKind, TagbodyTag,
};
use ncl_hir::{
    CleanupId, CompilationUnitId, FormError, FormId, HirFormBuilder, LambdaSignature, Literal,
    NodeId, SourceFiles, SourceOriginId, SourceOrigins, SymbolId, SymbolTable, ValueId,
    VerifyError,
};

fn form_builder(form: FormId) -> Result<HirFormBuilder, Box<dyn Error>> {
    let mut files = SourceFiles::new(form.compilation_unit());
    let file = files.add("verifier-coverage.lisp", "x")?;
    let span = files.span(file, 0, 1)?;
    let mut origins = SourceOrigins::new(form);
    let origin = origins.add_source(&files, file, span)?;
    Ok(HirFormBuilder::new(form, origins, origin)?)
}

fn symbol_table(form: FormId, names: &[&str]) -> Result<SymbolTable, Box<dyn Error>> {
    let mut symbols = SymbolTable::new(form.compilation_unit());
    for (index, name) in names.iter().enumerate() {
        let symbol = symbols.intern(form, Some("CL"), name, None)?;
        assert_eq!(
            symbol,
            SymbolId::try_from_index(form.compilation_unit(), index)?
        );
    }
    Ok(symbols)
}

const fn empty_signature() -> LambdaSignature {
    LambdaSignature {
        required: Vec::new(),
        optional: Vec::new(),
        rest: None,
        keys: Vec::new(),
        allow_other_keys: false,
        aux: Vec::new(),
    }
}

#[test]
fn lexical_parameters_and_let_scope_lists_match_their_bindings() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(41), 0);
    let symbol = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    let mut builder = form_builder(form)?;
    builder.register_symbol(symbol)?;
    let root = builder.root_scope();
    let body_scope = builder.bindings_mut().push_scope(root)?;
    let local_function = builder.bindings_mut().bind_function(body_scope, symbol)?;
    let lambda = builder.alloc_node(
        root,
        NodeKind::Lambda {
            parameters: vec![local_function],
            body_scope,
            body: Vec::new(),
        },
    )?;
    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &["F"])?),
        Err(VerifyError::InvalidBindingScopes(lambda))
    );

    let form = FormId::new(CompilationUnitId::new(41), 1);
    let symbol = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    let mut builder = form_builder(form)?;
    builder.register_symbol(symbol)?;
    let root = builder.root_scope();
    let body_scope = builder.bindings_mut().push_scope(root)?;
    let variable = builder.bindings_mut().bind_variable(body_scope, symbol)?;
    let binding = LetBinding {
        binding: variable,
        initializer: None,
    };
    let let_node = builder.alloc_node(
        root,
        NodeKind::Let {
            kind: LetKind::Parallel,
            bindings: vec![binding],
            initializer_scopes: Vec::new(),
            binding_scopes: vec![body_scope],
            body_scope,
            body: Vec::new(),
        },
    )?;
    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &["X"])?),
        Err(VerifyError::InvalidBindingScopes(let_node))
    );
    Ok(())
}

#[test]
fn function_references_reject_missing_symbol_and_computed_function_nodes()
-> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(42), 0);
    let missing_symbol = SymbolId::try_from_index(form.compilation_unit(), 1)?;
    let mut builder = form_builder(form)?;
    builder.alloc_node(
        builder.root_scope(),
        NodeKind::FunctionReference {
            target: CallTarget::Global(missing_symbol),
        },
    )?;
    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &["OTHER"])?),
        Err(VerifyError::InvalidSymbol(missing_symbol))
    );

    let form = FormId::new(CompilationUnitId::new(42), 1);
    let missing_node = NodeId::try_from_index(form, 9)?;
    let mut builder = form_builder(form)?;
    builder.alloc_node(
        builder.root_scope(),
        NodeKind::Call {
            target: CallTarget::Value(missing_node),
            arguments: Vec::new(),
        },
    )?;
    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&SymbolTable::new(form.compilation_unit())),
        Err(VerifyError::InvalidNode(missing_node))
    );
    Ok(())
}

#[test]
fn tagbody_flow_requires_nonvalued_completion() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(43), 0);
    let mut builder = form_builder(form)?;
    let tagbody = builder.alloc_node(
        builder.root_scope(),
        NodeKind::Tagbody {
            tags: Vec::<TagbodyTag>::new(),
            forms: Vec::new(),
        },
    )?;
    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&SymbolTable::new(form.compilation_unit())),
        Err(VerifyError::InvalidFunctionFlow(tagbody))
    );

    let form = FormId::new(CompilationUnitId::new(43), 1);
    let mut builder = form_builder(form)?;
    let scope = builder.root_scope();
    let value = builder.alloc_node(scope, NodeKind::Literal(Literal::True))?;
    let tagbody = builder.alloc_node(
        scope,
        NodeKind::Tagbody {
            tags: Vec::new(),
            forms: Vec::new(),
        },
    )?;
    let cfg = builder.control_flow_mut();
    cfg.begin_function(tagbody)?;
    let block = cfg.create_block(tagbody)?;
    cfg.set_terminator(
        block,
        Terminator::Return {
            values: vec![value],
        },
    )?;
    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&SymbolTable::new(form.compilation_unit())),
        Err(VerifyError::TagbodyReturnValues(block))
    );
    Ok(())
}

#[test]
fn check_type_condition_rejects_shared_success_and_failure_edges() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(44), 0);
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
    let cfg = builder.control_flow_mut();
    cfg.begin_function(function)?;
    let entry = cfg.create_block(function)?;
    cfg.append_operation(entry, producer)?;
    cfg.set_terminator(
        entry,
        Terminator::Conditional {
            condition: check,
            if_true: entry,
            if_false: entry,
        },
    )?;

    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&SymbolTable::new(form.compilation_unit())),
        Err(VerifyError::InvalidCheckEdges(check))
    );
    Ok(())
}

#[test]
fn nonlocal_exit_terminators_reject_wrong_target_and_unregistered_cleanup()
-> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(45), 0);
    let mut builder = form_builder(form)?;
    let scope = builder.root_scope();
    let target = builder.alloc_node(scope, NodeKind::Literal(Literal::True))?;
    let function = builder.alloc_node(
        scope,
        NodeKind::Lambda {
            parameters: Vec::new(),
            body_scope: scope,
            body: Vec::new(),
        },
    )?;
    let cfg = builder.control_flow_mut();
    cfg.begin_function(function)?;
    let entry = cfg.create_block(function)?;
    cfg.set_terminator(
        entry,
        Terminator::NonLocalExit {
            target: ExitTarget::Block(target),
            values: Vec::new(),
            cleanups: Vec::new(),
        },
    )?;
    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&SymbolTable::new(form.compilation_unit())),
        Err(VerifyError::InvalidExitTarget(target))
    );

    let form = FormId::new(CompilationUnitId::new(45), 1);
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
    let cleanup = CleanupId::try_from_index(form, 0)?;
    let cfg = builder.control_flow_mut();
    cfg.begin_function(function)?;
    let entry = cfg.create_block(function)?;
    cfg.set_terminator(
        entry,
        Terminator::NonLocalExit {
            target: ExitTarget::Function(function),
            values: Vec::new(),
            cleanups: vec![cleanup],
        },
    )?;
    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&SymbolTable::new(form.compilation_unit())),
        Err(VerifyError::InvalidCleanup(cleanup))
    );
    Ok(())
}

#[test]
fn flet_signature_is_verified_against_its_definition_scope() -> Result<(), Box<dyn Error>> {
    let unit = CompilationUnitId::new(47);
    let form = FormId::new(unit, 0);
    let mut builder = form_builder(form)?;
    let root = builder.root_scope();
    let name = SymbolId::try_from_index(unit, 0)?;
    builder.register_symbol(name)?;
    let group =
        builder
            .bindings_mut()
            .bind_local_functions(root, LocalFunctionKind::Flet, &[name])?;
    let definition = group
        .definitions
        .first()
        .copied()
        .ok_or("FLET definition was not created")?;
    let node = builder.alloc_node(
        root,
        NodeKind::LocalFunctions {
            kind: LocalFunctionKind::Flet,
            body_scope: group.body_scope,
            definitions: vec![LocalFunctionDefinition {
                binding: definition.binding,
                definition_scope: definition.definition_scope,
                parameters: Vec::new(),
                body_scope: root,
                body: Vec::new(),
            }],
            body: Vec::new(),
        },
    )?;
    builder.set_lambda_signature_for_local_function(definition.binding, empty_signature())?;
    builder.add_root(node)?;
    let hir = builder.finish()?;
    assert_eq!(
        hir.verify(&symbol_table(form, &["LOCAL-FUNCTION"])?),
        Ok(())
    );
    Ok(())
}

#[test]
fn lambda_builder_rejects_defaults_outside_the_visible_scope_chain() -> Result<(), Box<dyn Error>> {
    let unit = CompilationUnitId::new(48);
    let form = FormId::new(unit, 0);
    let mut builder = form_builder(form)?;
    let root = builder.root_scope();
    let parameter_scope = builder.bindings_mut().push_scope(root)?;
    let sibling_scope = builder.bindings_mut().push_scope(root)?;
    let symbol = SymbolId::try_from_index(unit, 0)?;
    let binding = builder
        .bindings_mut()
        .bind_variable(parameter_scope, symbol)?;
    let default = builder.alloc_node(sibling_scope, NodeKind::Literal(Literal::True))?;
    let lambda = builder.alloc_node(
        root,
        NodeKind::Lambda {
            parameters: vec![binding],
            body_scope: parameter_scope,
            body: Vec::new(),
        },
    )?;
    let signature = LambdaSignature {
        optional: vec![ncl_hir::LambdaOptional {
            binding,
            default: Some(default),
            initializer_scope: root,
            supplied_p: None,
        }],
        ..empty_signature()
    };
    assert_eq!(
        builder.set_lambda_signature_for_lambda(lambda, signature),
        Err(FormError::InvalidFunctionSignature)
    );
    Ok(())
}

#[path = "verifier/coverage_paths.rs"]
mod coverage_paths;
