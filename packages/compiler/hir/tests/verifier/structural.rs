//! Additional scope, ID, cleanup, and source-origin verifier cases.

use std::error::Error;

use super::control;
use super::node;
use super::source;
use super::{
    CleanupId, CompilationUnitId, FactAuthorization, FactCertainty, FactDomain, FactProvenance,
    FactRecord, Fixture, Node, NodeKind, SourceOriginId, SymbolId, TypeSpecConstId, ValueId,
    VerifyError, conditional_cfg, form, return_cfg,
};

#[test]
fn unreferenced_bindings_still_need_valid_symbol_ids() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new(9)?;
    let foreign = SymbolId::try_from_index(CompilationUnitId::new(1), 0)?;
    let _ = fixture.bindings.bind_variable(fixture.root, foreign)?;
    let function = fixture.add_lambda(fixture.root)?;
    let cfg = return_cfg(fixture.form, function)?;
    assert_eq!(
        fixture.verify(&cfg),
        Err(VerifyError::InvalidSymbol(foreign))
    );
    Ok(())
}

#[test]
fn resource_views_reject_sparse_identity_indices() -> Result<(), Box<dyn Error>> {
    let mut symbol_fixture = Fixture::new(10)?;
    let sparse_symbol = SymbolId::try_from_index(symbol_fixture.form.compilation_unit(), 1)?;
    let _ = symbol_fixture.symbols.insert(sparse_symbol);
    let function = symbol_fixture.add_lambda(symbol_fixture.root)?;
    let cfg = return_cfg(symbol_fixture.form, function)?;
    assert_eq!(
        symbol_fixture.verify(&cfg),
        Err(VerifyError::InvalidSymbol(sparse_symbol))
    );

    let mut value_fixture = Fixture::new(11)?;
    let sparse_value = ValueId::try_from_index(value_fixture.form, 1)?;
    let _ = value_fixture.values.insert(sparse_value);
    let function = value_fixture.add_lambda(value_fixture.root)?;
    let cfg = return_cfg(value_fixture.form, function)?;
    assert_eq!(
        value_fixture.verify(&cfg),
        Err(VerifyError::InvalidValue(sparse_value))
    );

    let mut constant_fixture = Fixture::new(12)?;
    let sparse_constant = TypeSpecConstId::try_from_index(constant_fixture.form, 1)?;
    let _ = constant_fixture.constants.insert(sparse_constant);
    let function = constant_fixture.add_lambda(constant_fixture.root)?;
    let cfg = return_cfg(constant_fixture.form, function)?;
    assert_eq!(
        constant_fixture.verify(&cfg),
        Err(VerifyError::InvalidTypeSpecConstant(sparse_constant))
    );
    Ok(())
}

#[test]
fn a_nonbinder_body_cannot_change_scope() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new(13)?;
    let child_scope = fixture.bindings.push_scope(fixture.root)?;
    let child = fixture.add_node(child_scope, NodeKind::Literal(node::Literal::Nil))?;
    let function = fixture.add_node(
        fixture.root,
        NodeKind::Lambda {
            parameters: Vec::new(),
            body_scope: fixture.root,
            body: vec![child],
        },
    )?;
    let cfg = return_cfg(fixture.form, function)?;
    assert!(matches!(
        fixture.verify(&cfg),
        Err(VerifyError::InvalidScope(_))
    ));
    Ok(())
}

#[test]
fn cleanup_references_can_precede_their_region_node() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new(14)?;
    let function = fixture.add_lambda(fixture.root)?;
    let cleanup = CleanupId::try_from_index(fixture.form, 0)?;
    let _ = fixture.add_node(
        fixture.root,
        NodeKind::NonLocalExit {
            target: control::ExitTarget::Function(function),
            values: Vec::new(),
            cleanups: vec![cleanup],
        },
    )?;
    let protected = fixture.add_node(fixture.root, NodeKind::Literal(node::Literal::Nil))?;
    let _ = fixture.add_node(
        fixture.root,
        NodeKind::UnwindProtect {
            cleanup,
            protected,
            cleanup_forms: Vec::new(),
        },
    )?;
    let cfg = return_cfg(fixture.form, function)?;
    assert_eq!(fixture.verify(&cfg), Ok(()));
    Ok(())
}

#[test]
fn node_rejects_an_unknown_source_origin() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new(15)?;
    let function = fixture.add_lambda(fixture.root)?;
    let missing = SourceOriginId::try_from_index(fixture.form, 1)?;
    fixture.nodes.allocate(Node {
        scope: fixture.root,
        origin: missing,
        kind: NodeKind::Literal(node::Literal::Nil),
    })?;
    let cfg = return_cfg(fixture.form, function)?;
    assert_eq!(
        fixture.verify(&cfg),
        Err(VerifyError::InvalidOrigin(missing))
    );
    Ok(())
}

#[test]
fn an_equivalent_guard_must_be_an_existing_checked_type_node() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new(17)?;
    let value = ValueId::try_from_index(fixture.form, 0)?;
    let expected = TypeSpecConstId::try_from_index(fixture.form, 0)?;
    let _ = fixture.values.insert(value);
    let _ = fixture.constants.insert(expected);
    let accepted_type = fixture.types.empty();
    let check = fixture.add_node(
        fixture.root,
        NodeKind::CheckType {
            value,
            accepted_type,
            expected_type: expected,
            source_origin: fixture.origin,
        },
    )?;
    let function = fixture.add_lambda(fixture.root)?;
    let cfg = conditional_cfg(fixture.form, function, check, false)?;
    let _ = fixture.facts.add(FactRecord::new(
        FactDomain::Type,
        0,
        fixture.origin,
        FactProvenance::Guard,
        FactCertainty::Proven,
        FactAuthorization::ElideTypeCheck {
            value,
            value_type: fixture.types.any(),
            accepted_type,
            expected_type: expected,
            equivalent_guard: Some(check),
        },
    ))?;
    assert_eq!(fixture.verify(&cfg), Ok(()));
    Ok(())
}

#[test]
fn multiple_value_bind_uses_a_private_body_scope() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new(18)?;
    let symbol = SymbolId::try_from_index(fixture.form.compilation_unit(), 4)?;
    let _ = fixture.symbols.insert(symbol);
    let body_scope = fixture.bindings.push_scope(fixture.root)?;
    let binding = fixture.bindings.bind_variable(body_scope, symbol)?;
    let reference = fixture.add_node(
        body_scope,
        NodeKind::VariableReference {
            target: node::VariableTarget::Lexical(binding),
        },
    )?;
    let value = fixture.add_node(fixture.root, NodeKind::Literal(node::Literal::Nil))?;
    let _ = fixture.add_node(
        fixture.root,
        NodeKind::MultipleValues(MultipleValueOperation::Bind {
            value,
            bindings: vec![binding],
            body_scope,
            body: vec![reference],
        }),
    )?;
    let function = fixture.add_lambda(fixture.root)?;
    let cfg = return_cfg(fixture.form, function)?;
    assert_eq!(fixture.verify(&cfg), Ok(()));

    let mut invalid = Fixture::new(19)?;
    let symbol = SymbolId::try_from_index(invalid.form.compilation_unit(), 5)?;
    let _ = invalid.symbols.insert(symbol);
    let body_scope = invalid.bindings.push_scope(invalid.root)?;
    let binding = invalid.bindings.bind_variable(body_scope, symbol)?;
    let reference = invalid.add_node(
        invalid.root,
        NodeKind::VariableReference {
            target: node::VariableTarget::Lexical(binding),
        },
    )?;
    let value = invalid.add_node(invalid.root, NodeKind::Literal(node::Literal::Nil))?;
    let _ = invalid.add_node(
        invalid.root,
        NodeKind::MultipleValues(MultipleValueOperation::Bind {
            value,
            bindings: vec![binding],
            body_scope,
            body: vec![reference],
        }),
    )?;
    let function = invalid.add_lambda(invalid.root)?;
    let cfg = return_cfg(invalid.form, function)?;
    assert!(matches!(
        invalid.verify(&cfg),
        Err(VerifyError::InvalidScope(_))
    ));
    Ok(())
}

#[test]
fn nonempty_binders_cannot_reuse_the_parent_scope() -> Result<(), Box<dyn Error>> {
    let mut lambda = Fixture::new(20)?;
    let symbol = SymbolId::try_from_index(lambda.form.compilation_unit(), 6)?;
    let _ = lambda.symbols.insert(symbol);
    let parameter = lambda.bindings.bind_variable(lambda.root, symbol)?;
    let function = lambda.add_node(
        lambda.root,
        NodeKind::Lambda {
            parameters: vec![parameter],
            body_scope: lambda.root,
            body: Vec::new(),
        },
    )?;
    let cfg = return_cfg(lambda.form, function)?;
    assert!(matches!(
        lambda.verify(&cfg),
        Err(VerifyError::InvalidBindingScopes(_))
    ));

    let mut parallel = Fixture::new(21)?;
    let symbol = SymbolId::try_from_index(parallel.form.compilation_unit(), 7)?;
    let _ = parallel.symbols.insert(symbol);
    let binding = parallel.bindings.bind_variable(parallel.root, symbol)?;
    let _ = parallel.add_node(
        parallel.root,
        NodeKind::Let {
            kind: node::LetKind::Parallel,
            bindings: vec![node::LetBinding {
                binding,
                initializer: None,
            }],
            initializer_scopes: vec![parallel.root],
            binding_scopes: vec![parallel.root],
            body_scope: parallel.root,
            body: Vec::new(),
        },
    )?;
    let function = parallel.add_lambda(parallel.root)?;
    let cfg = return_cfg(parallel.form, function)?;
    assert!(matches!(
        parallel.verify(&cfg),
        Err(VerifyError::InvalidBindingScopes(_))
    ));
    Ok(())
}

#[test]
fn source_expansion_requires_an_existing_parent() -> Result<(), Box<dyn Error>> {
    let form = form(16);
    let mut files = source::SourceFiles::new(form.compilation_unit());
    let file = files.add("hir-test.lisp", "x")?;
    let span = files.span(file, 0, 1)?;
    let mut origins = source::SourceOrigins::new(form);
    let missing_parent = SourceOriginId::try_from_index(form, 0)?;
    let transform = source::SourceTransform::new(
        source::SourceTransformKind::MacroExpansion,
        SymbolId::try_from_index(form.compilation_unit(), 0)?,
    );
    assert!(matches!(
        origins.add_expansion(&files, file, span, missing_parent, transform),
        Err(source::SourceError::UnknownOrigin(_))
    ));
    assert!(origins.is_empty());
    Ok(())
}
