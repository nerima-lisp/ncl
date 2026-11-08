//! Tests for form-local HIR verification and fact invalidation.

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

use ncl_hir::binding::{BindingEnvironment, LocalFunctionKind, ScopeId};
use ncl_hir::cl_type::TypeArena;
use ncl_hir::control::{self, ControlError, ControlFlowBuilder, ControlFlowGraph, Terminator};
use ncl_hir::ids::{
    CleanupId, CompilationUnitId, FormId, IdError, NodeId, SourceOriginId, SymbolId,
    TypeSpecConstId, ValueId,
};
use ncl_hir::node::{
    self, CallTarget, LocalFunctionDefinition, Node, NodeArena, NodeKind, VariableTarget,
};
use ncl_hir::policy::OptimizationPolicy;
use ncl_hir::source::{self, SourceFiles, SourceOrigins};
use ncl_hir::verify::{
    FactAuthorization, FactCertainty, FactDomain, FactEpochs, FactProvenance, FactRecord,
    FactStore, PassInvalidations, VerificationBuild, VerificationConfig, VerifyError, VerifyInput,
    VerifyResources, verify,
};

#[path = "verifier/structural.rs"]
mod structural;

const fn form(index: u32) -> FormId {
    FormId::new(CompilationUnitId::new(0), index)
}

struct Fixture {
    form: FormId,
    origin: SourceOriginId,
    root: ScopeId,
    nodes: NodeArena,
    types: TypeArena,
    bindings: BindingEnvironment,
    origins: SourceOrigins,
    facts: FactStore,
    symbols: HashSet<SymbolId>,
    values: HashSet<ValueId>,
    value_producers: HashMap<ValueId, NodeId>,
    constants: HashSet<TypeSpecConstId>,
}

impl Fixture {
    fn new(index: u32) -> Result<Self, Box<dyn Error>> {
        let form = form(index);
        let compilation_unit = form.compilation_unit();
        let mut files = SourceFiles::new(compilation_unit);
        let file = files.add("hir-test.lisp", "x")?;
        let span = files.span(file, 0, 1)?;
        let mut origins = SourceOrigins::new(form);
        let origin = origins.add_source(&files, file, span)?;
        let bindings = BindingEnvironment::new(form);
        let root = bindings.root_scope();
        Ok(Self {
            form,
            origin,
            root,
            nodes: NodeArena::new(form),
            types: TypeArena::new(form)?,
            bindings,
            origins,
            facts: FactStore::new(form),
            symbols: HashSet::new(),
            values: HashSet::new(),
            value_producers: HashMap::new(),
            constants: HashSet::new(),
        })
    }

    fn add_node(&mut self, scope: ScopeId, kind: NodeKind) -> Result<NodeId, IdError> {
        self.nodes.allocate(Node {
            scope,
            origin: self.origin,
            kind,
        })
    }

    fn insert_symbols_through(&mut self, index: usize) -> Result<SymbolId, IdError> {
        let compilation_unit = self.form.compilation_unit();
        for position in 0..=index {
            let symbol = SymbolId::try_from_index(compilation_unit, position)?;
            let _ = self.symbols.insert(symbol);
        }
        SymbolId::try_from_index(compilation_unit, index)
    }

    fn add_lambda(&mut self, body_scope: ScopeId) -> Result<NodeId, IdError> {
        self.add_node(
            self.root,
            NodeKind::Lambda {
                parameters: Vec::new(),
                body_scope,
                body: Vec::new(),
            },
        )
    }

    fn verify(&self, cfg: &ControlFlowGraph) -> Result<(), VerifyError> {
        verify(VerifyInput {
            form: self.form,
            nodes: &self.nodes,
            types: &self.types,
            bindings: &self.bindings,
            control_flow: cfg,
            origins: &self.origins,
            facts: &self.facts,
            epochs: FactEpochs::default(),
            policy: OptimizationPolicy::default(),
            resources: VerifyResources::new(
                self.form.compilation_unit(),
                &self.symbols,
                &self.values,
                &self.value_producers,
                &self.constants,
            ),
        })
    }
}

fn return_cfg(form: FormId, function: NodeId) -> Result<ControlFlowGraph, ControlError> {
    let mut builder = ControlFlowBuilder::new(form);
    builder.begin_function(function)?;
    let entry = builder.create_block(function)?;
    builder.set_terminator(entry, Terminator::Return { values: Vec::new() })?;
    builder.finish()
}

fn conditional_cfg(
    form: FormId,
    function: NodeId,
    condition: NodeId,
    same_edge: bool,
    operations: &[NodeId],
    alternate_operations: &[NodeId],
) -> Result<ControlFlowGraph, ControlError> {
    let mut builder = ControlFlowBuilder::new(form);
    builder.begin_function(function)?;
    let entry = builder.create_block(function)?;
    let if_true = builder.create_block(function)?;
    let if_false = builder.create_block(function)?;
    for operation in operations {
        builder.append_operation(entry, *operation)?;
    }
    for operation in alternate_operations {
        builder.append_operation(if_true, *operation)?;
    }
    builder.set_terminator(
        entry,
        Terminator::Conditional {
            condition,
            if_true,
            if_false: if same_edge { if_true } else { if_false },
        },
    )?;
    builder.set_terminator(if_true, Terminator::Return { values: Vec::new() })?;
    builder.set_terminator(if_false, Terminator::Return { values: Vec::new() })?;
    builder.finish()
}

#[test]
fn ordinary_conditional_may_share_a_successor() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new(0)?;
    let condition = fixture.add_node(fixture.root, NodeKind::Literal(node::Literal::True))?;
    let function = fixture.add_lambda(fixture.root)?;
    let cfg = conditional_cfg(fixture.form, function, condition, true, &[], &[])?;
    assert_eq!(fixture.verify(&cfg), Ok(()));
    Ok(())
}

#[test]
fn lexical_variable_must_resolve_to_the_exact_visible_binding() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new(1)?;
    let symbol = SymbolId::try_from_index(fixture.form.compilation_unit(), 0)?;
    let _ = fixture.symbols.insert(symbol);
    let child = fixture.bindings.push_scope(fixture.root)?;
    let binding = fixture.bindings.bind_variable(child, symbol)?;
    let function = fixture.add_lambda(fixture.root)?;
    let _ = fixture.add_node(
        fixture.root,
        NodeKind::VariableReference {
            target: VariableTarget::Lexical(binding),
        },
    )?;
    let cfg = return_cfg(fixture.form, function)?;
    assert!(matches!(
        fixture.verify(&cfg),
        Err(VerifyError::InvisibleVariable { .. })
    ));
    Ok(())
}

#[test]
fn flet_definition_cannot_reference_its_local_function_binding() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new(2)?;
    let symbol = fixture.insert_symbols_through(1)?;
    let group =
        fixture
            .bindings
            .bind_local_functions(fixture.root, LocalFunctionKind::Flet, &[symbol])?;
    let definition = group
        .definitions
        .first()
        .copied()
        .ok_or("missing local function")?;
    let function = fixture.add_lambda(fixture.root)?;
    let call = fixture.add_node(
        fixture.root,
        NodeKind::Call {
            target: CallTarget::Local(definition.binding),
            arguments: Vec::new(),
        },
    )?;
    let group_node = fixture.add_node(
        fixture.root,
        NodeKind::LocalFunctions {
            kind: LocalFunctionKind::Flet,
            body_scope: group.body_scope,
            definitions: vec![LocalFunctionDefinition {
                binding: definition.binding,
                definition_scope: fixture.root,
                parameters: Vec::new(),
                body_scope: fixture.root,
                body: vec![call],
            }],
            body: Vec::new(),
        },
    )?;
    let _ = group_node;
    let cfg = return_cfg(fixture.form, function)?;
    assert!(matches!(
        fixture.verify(&cfg),
        Err(VerifyError::InvisibleFunction { .. })
    ));
    Ok(())
}

#[test]
fn labels_definition_can_reference_its_local_function_binding() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new(3)?;
    let symbol = fixture.insert_symbols_through(2)?;
    let group = fixture.bindings.bind_local_functions(
        fixture.root,
        LocalFunctionKind::Labels,
        &[symbol],
    )?;
    let definition = group
        .definitions
        .first()
        .copied()
        .ok_or("missing local function")?;
    let function = fixture.add_lambda(fixture.root)?;
    let call = fixture.add_node(
        group.body_scope,
        NodeKind::Call {
            target: CallTarget::Local(definition.binding),
            arguments: Vec::new(),
        },
    )?;
    let _ = fixture.add_node(
        fixture.root,
        NodeKind::LocalFunctions {
            kind: LocalFunctionKind::Labels,
            body_scope: group.body_scope,
            definitions: vec![LocalFunctionDefinition {
                binding: definition.binding,
                definition_scope: group.body_scope,
                parameters: Vec::new(),
                body_scope: group.body_scope,
                body: vec![call],
            }],
            body: Vec::new(),
        },
    )?;
    let cfg = return_cfg(fixture.form, function)?;
    assert_eq!(fixture.verify(&cfg), Ok(()));
    Ok(())
}

#[test]
fn check_type_requires_distinct_success_and_failure_edges() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new(4)?;
    let value = ValueId::try_from_index(fixture.form, 0)?;
    let producer = fixture.add_node(fixture.root, NodeKind::Literal(node::Literal::Nil))?;
    let expected = TypeSpecConstId::try_from_index(fixture.form, 0)?;
    let _ = fixture.values.insert(value);
    let _ = fixture.constants.insert(expected);
    let check = fixture.add_node(
        fixture.root,
        NodeKind::CheckType {
            value,
            accepted_type: fixture.types.any(),
            expected_type: expected,
            source_origin: fixture.origin,
        },
    )?;
    let function = fixture.add_lambda(fixture.root)?;
    let distinct_edges = conditional_cfg(fixture.form, function, check, false, &[], &[])?;
    assert_eq!(
        fixture.verify(&distinct_edges),
        Err(VerifyError::MissingValueProducer(check))
    );
    let _ = fixture.value_producers.insert(value, check);
    let self_produced = conditional_cfg(fixture.form, function, check, false, &[], &[])?;
    assert_eq!(
        fixture.verify(&self_produced),
        Err(VerifyError::InvalidValueProducer {
            check,
            producer: check,
        })
    );
    let _ = fixture.value_producers.insert(value, producer);
    let missing_operation = conditional_cfg(fixture.form, function, check, false, &[], &[])?;
    assert_eq!(
        fixture.verify(&missing_operation),
        Err(VerifyError::InvalidValueProducer { check, producer })
    );
    let same_edge = conditional_cfg(fixture.form, function, check, true, &[producer], &[])?;
    assert_eq!(
        fixture.verify(&same_edge),
        Err(VerifyError::InvalidCheckEdges(check))
    );
    let non_dominating = conditional_cfg(fixture.form, function, check, false, &[], &[producer])?;
    assert_eq!(
        fixture.verify(&non_dominating),
        Err(VerifyError::InvalidValueProducer { check, producer })
    );
    let duplicate_producer = conditional_cfg(
        fixture.form,
        function,
        check,
        false,
        &[producer, producer],
        &[],
    )?;
    assert_eq!(
        fixture.verify(&duplicate_producer),
        Err(VerifyError::InvalidValueProducer { check, producer })
    );
    let distinct_edges = conditional_cfg(fixture.form, function, check, false, &[producer], &[])?;
    assert_eq!(fixture.verify(&distinct_edges), Ok(()));
    Ok(())
}

#[test]
fn verifier_checks_symbols_hidden_inside_type_nodes() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new(5)?;
    let absent_symbol = SymbolId::try_from_index(fixture.form.compilation_unit(), 3)?;
    let _type = fixture.types.class(absent_symbol)?;
    let function = fixture.add_lambda(fixture.root)?;
    let cfg = return_cfg(fixture.form, function)?;
    assert_eq!(
        fixture.verify(&cfg),
        Err(VerifyError::InvalidSymbol(absent_symbol))
    );
    Ok(())
}

#[test]
fn unknown_fact_cannot_authorize_check_elision() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new(6)?;
    let value = ValueId::try_from_index(fixture.form, 0)?;
    let expected = TypeSpecConstId::try_from_index(fixture.form, 0)?;
    let _ = fixture.values.insert(value);
    let _ = fixture.constants.insert(expected);
    let any = fixture.types.any();
    let fact = FactRecord::new(
        FactDomain::Type,
        0,
        fixture.origin,
        FactProvenance::Inferred,
        FactCertainty::Unknown,
        FactAuthorization::ElideTypeCheck {
            value,
            value_type: any,
            accepted_type: any,
            expected_type: expected,
            equivalent_guard: None,
        },
    );
    let _ = fixture.facts.add(fact)?;
    let function = fixture.add_lambda(fixture.root)?;
    let cfg = return_cfg(fixture.form, function)?;
    assert!(matches!(
        fixture.verify(&cfg),
        Err(VerifyError::UnprovenCheckElision(_))
    ));
    Ok(())
}

#[test]
fn declared_types_alone_cannot_authorize_check_elision() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new(7)?;
    let value = ValueId::try_from_index(fixture.form, 0)?;
    let expected = TypeSpecConstId::try_from_index(fixture.form, 0)?;
    let _ = fixture.values.insert(value);
    let _ = fixture.constants.insert(expected);
    let any = fixture.types.any();
    let fact = FactRecord::new(
        FactDomain::Type,
        0,
        fixture.origin,
        FactProvenance::Declared,
        FactCertainty::Proven,
        FactAuthorization::ElideTypeCheck {
            value,
            value_type: any,
            accepted_type: any,
            expected_type: expected,
            equivalent_guard: None,
        },
    );
    let _ = fixture.facts.add(fact)?;
    let function = fixture.add_lambda(fixture.root)?;
    let cfg = return_cfg(fixture.form, function)?;
    assert!(matches!(
        fixture.verify(&cfg),
        Err(VerifyError::InvalidCheckElision(_))
    ));
    Ok(())
}

#[test]
fn stale_facts_are_rejected_after_declared_domain_invalidation() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new(8)?;
    let advanced = FactEpochs::default().after_pass(&PassInvalidations::new([FactDomain::Type]))?;
    let fact = FactRecord::new(
        FactDomain::Type,
        0,
        fixture.origin,
        FactProvenance::Inferred,
        FactCertainty::Proven,
        FactAuthorization::Informational,
    );
    let _ = fixture.facts.add(fact)?;
    let function = fixture.add_lambda(fixture.root)?;
    let cfg = return_cfg(fixture.form, function)?;
    let input = VerifyInput {
        form: fixture.form,
        nodes: &fixture.nodes,
        types: &fixture.types,
        bindings: &fixture.bindings,
        control_flow: &cfg,
        origins: &fixture.origins,
        facts: &fixture.facts,
        epochs: advanced,
        policy: OptimizationPolicy::default(),
        resources: VerifyResources::new(
            fixture.form.compilation_unit(),
            &fixture.symbols,
            &fixture.values,
            &fixture.value_producers,
            &fixture.constants,
        ),
    };
    assert!(matches!(verify(input), Err(VerifyError::StaleFact(_))));
    Ok(())
}

#[test]
fn verifier_defaults_follow_d10_gates_and_keep_release_opt_in() {
    assert!(VerificationConfig::new(VerificationBuild::Test, true, true, false).enabled());
    assert!(VerificationConfig::new(VerificationBuild::Ci, true, true, false).enabled());
    assert!(VerificationConfig::new(VerificationBuild::Release, false, true, false).enabled());
    assert!(!VerificationConfig::new(VerificationBuild::Release, true, true, false).enabled());
    assert!(VerificationConfig::new(VerificationBuild::Release, true, true, true).enabled());
}
