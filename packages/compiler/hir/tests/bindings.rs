//! Tests for semantic nodes, lexical binding identity, and local CFG structure.

#![deny(
    unreachable_pub,
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::as_conversions
)]
#![expect(
    unused_crate_dependencies,
    reason = "This target includes source modules by path to test private invariants"
)]

#[path = "../src/arena.rs"]
/// The arena implementation shared by these tests.
pub mod arena;
#[path = "../src/binding.rs"]
/// The lexical binding implementation under test.
pub mod binding;
#[path = "../src/control.rs"]
/// The control-flow graph implementation under test.
pub mod control;
#[path = "../src/ids.rs"]
/// Typed identities shared by the HIR modules.
pub mod ids;
#[path = "../src/node.rs"]
/// Semantic HIR nodes under test.
pub mod node;

use std::error::Error;

use binding::{BindingEnvironment, BindingError, BindingKind, BindingUseKind, LocalFunctionKind};
use control::{ControlError, ControlFlowBuilder, ExitTarget, Terminator};
use ids::{BindingId, CompilationUnitId, FormId, IdError, NodeId, SourceOriginId, SymbolId};
use node::{CallTarget, MultipleValueOperation, Node, NodeArena, NodeKind, VariableTarget};

const fn form(index: u32) -> FormId {
    FormId::new(CompilationUnitId::new(0), index)
}

fn symbol(index: usize) -> Result<SymbolId, IdError> {
    SymbolId::try_from_index(CompilationUnitId::new(0), index)
}

fn node_id(owner: FormId, index: usize) -> Result<NodeId, IdError> {
    NodeId::try_from_index(owner, index)
}

fn function_node(owner: FormId, nodes: &mut NodeArena) -> Result<NodeId, IdError> {
    let scope = BindingEnvironment::new(owner).root_scope();
    nodes.allocate(Node {
        scope,
        origin: SourceOriginId::try_from_index(owner, 0)?,
        kind: NodeKind::Lambda {
            parameters: Vec::new(),
            body_scope: scope,
            body: Vec::new(),
        },
    })
}

#[test]
fn flet_definitions_use_outer_scope_while_the_body_uses_local_bindings()
-> Result<(), Box<dyn Error>> {
    let owner = form(0);
    let mut bindings = BindingEnvironment::new(owner);
    let root = bindings.root_scope();
    let local_name = symbol(0)?;
    let sibling_name = symbol(1)?;
    let outer = bindings.bind_function(root, local_name)?;
    let group = bindings.bind_local_functions(
        root,
        LocalFunctionKind::Flet,
        &[local_name, sibling_name],
    )?;
    let first = group.definitions.first();
    let second = group.definitions.get(1);
    assert!(first.is_some());
    assert!(second.is_some());
    if let (Some(first), Some(second)) = (first, second) {
        assert_eq!(first.definition_scope, root);
        assert_eq!(
            bindings.resolve_function(first.definition_scope, local_name),
            Ok(Some(outer)),
            "a FLET body resolves the outer binding with the same name"
        );
        assert_eq!(
            bindings.resolve_function(second.definition_scope, sibling_name),
            Ok(None),
            "a FLET definition cannot see its sibling"
        );
        assert_eq!(
            bindings.resolve_function(group.body_scope, local_name),
            Ok(Some(first.binding))
        );
        assert_eq!(
            bindings.resolve_function(group.body_scope, sibling_name),
            Ok(Some(second.binding))
        );
        assert_ne!(first.binding, outer);
    }
    Ok(())
}

#[test]
fn labels_definitions_and_body_see_every_local_function() -> Result<(), Box<dyn Error>> {
    let owner = form(1);
    let mut bindings = BindingEnvironment::new(owner);
    let [first_name, second_name] = [symbol(2)?, symbol(3)?];
    let names = [first_name, second_name];
    let group =
        bindings.bind_local_functions(bindings.root_scope(), LocalFunctionKind::Labels, &names)?;
    let first = group.definitions.first();
    let second = group.definitions.get(1);
    assert!(first.is_some());
    assert!(second.is_some());
    if let (Some(first), Some(second)) = (first, second) {
        for scope in [
            first.definition_scope,
            second.definition_scope,
            group.body_scope,
        ] {
            assert_eq!(
                bindings.resolve_function(scope, first_name),
                Ok(Some(first.binding))
            );
            assert_eq!(
                bindings.resolve_function(scope, second_name),
                Ok(Some(second.binding))
            );
        }
    }
    Ok(())
}

#[test]
fn binding_ids_distinguish_namespaces_and_record_reads_and_writes() -> Result<(), Box<dyn Error>> {
    let owner = form(2);
    let mut bindings = BindingEnvironment::new(owner);
    let root = bindings.root_scope();
    let name = symbol(4)?;
    assert_eq!(root.form(), owner);
    assert_eq!(root.index(), 0);
    let variable = bindings.bind_variable(root, name)?;
    let function = bindings.bind_function(root, name)?;
    let inner_scope = bindings.push_scope(root)?;
    assert_ne!(variable, function);
    let shadow = bindings.bind_variable(inner_scope, name)?;
    assert_ne!(variable, shadow);
    assert_eq!(bindings.resolve_variable(root, name), Ok(Some(variable)));
    assert_eq!(
        bindings.resolve_variable(inner_scope, name),
        Ok(Some(shadow))
    );
    assert_eq!(
        bindings.resolve_function(inner_scope, name),
        Ok(Some(function))
    );
    assert_ne!(
        VariableTarget::Lexical(variable),
        VariableTarget::Special(name)
    );
    assert_ne!(VariableTarget::Special(name), VariableTarget::Global(name));
    bindings.record_use(variable, node_id(owner, 0)?, BindingUseKind::Read)?;
    bindings.record_use(variable, node_id(owner, 1)?, BindingUseKind::Write)?;
    let record = bindings.binding(variable);
    assert!(record.is_some());
    if let Some(record) = record {
        assert_eq!(record.kind(), BindingKind::LexicalVariable);
        assert_eq!(record.symbol(), name);
        assert_eq!(record.reads(), &[node_id(owner, 0)?]);
        assert_eq!(record.writes(), &[node_id(owner, 1)?]);
    }
    assert_eq!(
        bindings.binding(function).map(binding::Binding::kind),
        Some(BindingKind::LocalFunction)
    );
    assert_eq!(
        bindings.binding(function).map(binding::Binding::id),
        Some(function)
    );
    Ok(())
}

#[test]
fn invalid_scopes_bindings_and_duplicate_names_are_rejected() -> Result<(), Box<dyn Error>> {
    let owner = form(7);
    let other_form = form(8);
    let mut bindings = BindingEnvironment::new(owner);
    let root = bindings.root_scope();
    let name = symbol(8)?;
    let first = bindings.bind_variable(root, name)?;
    assert_eq!(
        bindings.bind_variable(root, name),
        Err(BindingError::DuplicateName {
            scope: root,
            symbol: name,
            namespace: binding::BindingNamespace::Variable,
        })
    );
    let foreign_scope = BindingEnvironment::new(other_form).root_scope();
    assert_eq!(
        bindings.resolve_variable(foreign_scope, name),
        Err(BindingError::UnknownScope(foreign_scope))
    );
    let foreign_node = node_id(other_form, 0)?;
    assert_eq!(
        bindings.record_use(first, foreign_node, BindingUseKind::Read),
        Err(BindingError::ForeignUse(foreign_node))
    );
    let unknown_binding = BindingId::try_from_index(owner, 99)?;
    assert_eq!(bindings.binding(unknown_binding), None);
    assert_eq!(
        bindings.record_use(unknown_binding, node_id(owner, 0)?, BindingUseKind::Read),
        Err(BindingError::UnknownBinding(unknown_binding))
    );
    let duplicate_function = symbol(9)?;
    assert_eq!(
        bindings.bind_local_functions(
            root,
            LocalFunctionKind::Flet,
            &[duplicate_function, duplicate_function],
        ),
        Err(BindingError::DuplicateName {
            scope: root,
            symbol: duplicate_function,
            namespace: binding::BindingNamespace::Function,
        })
    );
    assert_eq!(
        bindings.resolve_function(root, duplicate_function),
        Ok(None)
    );
    Ok(())
}

#[test]
fn nodes_keep_global_intrinsic_and_multiple_value_operations_distinct() -> Result<(), Box<dyn Error>>
{
    let owner = form(3);
    let symbol = symbol(5)?;
    let origin = SourceOriginId::try_from_index(owner, 0)?;
    let scope = BindingEnvironment::new(owner).root_scope();
    let global = Node {
        scope,
        origin,
        kind: NodeKind::Call {
            target: CallTarget::Global(symbol),
            arguments: Vec::new(),
        },
    };
    let intrinsic = Node {
        scope,
        origin,
        kind: NodeKind::Call {
            target: CallTarget::Intrinsic(symbol),
            arguments: Vec::new(),
        },
    };
    assert_ne!(global.kind, intrinsic.kind);
    let closure = NodeKind::ClosureReference {
        function: node_id(owner, 4)?,
    };
    assert!(matches!(closure, NodeKind::ClosureReference { .. }));
    let dynamic_binding = NodeKind::DynamicBinding {
        symbols: node_id(owner, 5)?,
        values: node_id(owner, 6)?,
        body: vec![node_id(owner, 7)?],
    };
    assert!(matches!(dynamic_binding, NodeKind::DynamicBinding { .. }));

    let values = NodeKind::MultipleValues(MultipleValueOperation::Produce {
        values: vec![node_id(owner, 0)?, node_id(owner, 1)?],
    });
    assert!(matches!(
        values,
        NodeKind::MultipleValues(MultipleValueOperation::Produce { values }) if values.len() == 2
    ));
    let exit = NodeKind::NonLocalExit {
        target: ExitTarget::Dynamic(node_id(owner, 2)?),
        values: vec![node_id(owner, 3)?],
        cleanups: Vec::new(),
    };
    assert!(matches!(exit, NodeKind::NonLocalExit { .. }));
    Ok(())
}

#[test]
fn nodes_retain_assignments_local_functions_and_cleanup_regions() -> Result<(), Box<dyn Error>> {
    let owner = form(9);
    let mut bindings = BindingEnvironment::new(owner);
    let root = bindings.root_scope();
    let variable = bindings.bind_variable(root, symbol(10)?)?;
    let group = bindings.bind_local_functions(root, LocalFunctionKind::Labels, &[symbol(11)?])?;
    let definition = group.definitions.first();
    assert!(definition.is_some());
    if let Some(definition) = definition {
        let local_functions = NodeKind::LocalFunctions {
            kind: LocalFunctionKind::Labels,
            body_scope: group.body_scope,
            definitions: vec![node::LocalFunctionDefinition {
                binding: definition.binding,
                definition_scope: definition.definition_scope,
                parameters: vec![variable],
                body_scope: bindings.push_scope(definition.definition_scope)?,
                body: vec![node_id(owner, 1)?],
            }],
            body: vec![node_id(owner, 2)?],
        };
        assert!(matches!(
            local_functions,
            NodeKind::LocalFunctions {
                kind: LocalFunctionKind::Labels,
                definitions,
                ..
            } if definitions.len() == 1
        ));
    }
    let assignment = NodeKind::Assignment {
        target: VariableTarget::Lexical(variable),
        value: node_id(owner, 3)?,
    };
    assert!(matches!(assignment, NodeKind::Assignment { .. }));
    let cleanup = ids::CleanupId::try_from_index(owner, 0)?;
    let unwind = NodeKind::UnwindProtect {
        cleanup,
        protected: node_id(owner, 4)?,
        cleanup_forms: vec![node_id(owner, 5)?],
    };
    assert!(matches!(unwind, NodeKind::UnwindProtect { .. }));
    Ok(())
}

#[test]
fn control_flow_requires_one_terminator_and_local_successors() -> Result<(), Box<dyn Error>> {
    let owner = form(4);
    let mut nodes = NodeArena::new(owner);
    let first_function = function_node(owner, &mut nodes)?;
    let second_function = function_node(owner, &mut nodes)?;
    let mut builder = ControlFlowBuilder::new(owner);
    builder.begin_function(first_function)?;
    builder.begin_function(second_function)?;
    let first_entry = builder.create_block(first_function)?;
    let second_entry = builder.create_block(second_function)?;
    builder.set_terminator(first_entry, Terminator::Jump(second_entry))?;
    builder.set_terminator(second_entry, Terminator::Unreachable)?;
    assert_eq!(
        builder.finish().map(|_| ()),
        Err(ControlError::InvalidSuccessor {
            source: first_entry,
            target: second_entry,
        })
    );
    Ok(())
}

#[test]
fn completed_control_flow_exposes_entry_blocks_and_terminators() -> Result<(), Box<dyn Error>> {
    let owner = form(5);
    let mut nodes = NodeArena::new(owner);
    let function = function_node(owner, &mut nodes)?;
    let condition = function_node(owner, &mut nodes)?;
    let operation = function_node(owner, &mut nodes)?;
    let mut builder = ControlFlowBuilder::new(owner);
    builder.begin_function(function)?;
    let entry = builder.create_block(function)?;
    let if_true = builder.create_block(function)?;
    let if_false = builder.create_block(function)?;
    builder.append_operation(entry, operation)?;
    builder.set_terminator(
        entry,
        Terminator::Conditional {
            condition,
            if_true,
            if_false,
        },
    )?;
    builder.set_terminator(
        if_true,
        Terminator::Return {
            values: vec![operation],
        },
    )?;
    builder.set_terminator(
        if_false,
        Terminator::Return {
            values: vec![operation],
        },
    )?;
    let graph = builder.finish()?;
    let flow = graph.function(function);
    assert!(flow.is_some());
    if let Some(flow) = flow {
        assert_eq!(flow.function(), function);
        assert_eq!(flow.entry(), entry);
        assert_eq!(flow.blocks(), &[entry, if_true, if_false]);
    }
    assert_eq!(graph.functions().len(), 1);
    let block = graph.block(entry);
    assert!(block.is_some());
    if let Some(block) = block {
        assert_eq!(block.function(), function);
        assert_eq!(block.operations(), &[operation]);
        assert_eq!(
            block.terminator(),
            &Terminator::Conditional {
                condition,
                if_true,
                if_false,
            }
        );
    }
    Ok(())
}

#[test]
fn control_flow_rejects_missing_terminators_and_duplicate_terminators() -> Result<(), Box<dyn Error>>
{
    let owner = form(6);
    let mut nodes = NodeArena::new(owner);
    let function = function_node(owner, &mut nodes)?;

    let mut missing = ControlFlowBuilder::new(owner);
    missing.begin_function(function)?;
    let block = missing.create_block(function)?;
    assert_eq!(
        missing.finish().map(|_| ()),
        Err(ControlError::MissingTerminator(block))
    );

    let mut duplicate = ControlFlowBuilder::new(owner);
    duplicate.begin_function(function)?;
    let block = duplicate.create_block(function)?;
    duplicate.set_terminator(block, Terminator::Unreachable)?;
    assert_eq!(
        duplicate.set_terminator(block, Terminator::Unreachable),
        Err(ControlError::BlockAlreadyTerminated(block))
    );
    Ok(())
}
