//! Control-flow graph construction, local edges, and error outcomes.

use std::error::Error;

use ncl_hir::control::{ControlError, ControlFlowBuilder, ExitTarget, Terminator};
use ncl_hir::ids::{BlockId, CleanupId, CompilationUnitId, FormId, IdError, NodeId};

const fn form(index: u32) -> FormId {
    FormId::new(CompilationUnitId::new(12), index)
}

fn node(owner: FormId, index: usize) -> Result<NodeId, IdError> {
    NodeId::try_from_index(owner, index)
}

#[test]
fn function_and_block_builders_reject_foreign_unknown_and_duplicate_identities()
-> Result<(), Box<dyn Error>> {
    let owner = form(0);
    let function = node(owner, 0)?;
    let unknown_function = node(owner, 1)?;
    let foreign_function = node(form(1), 0)?;
    let mut builder = ControlFlowBuilder::new(owner);

    assert_eq!(
        builder.begin_function(foreign_function),
        Err(ControlError::ForeignFunction(foreign_function))
    );
    builder.begin_function(function)?;
    assert_eq!(
        builder.begin_function(function),
        Err(ControlError::DuplicateFunction(function))
    );
    assert_eq!(
        builder.create_block(unknown_function),
        Err(ControlError::UnknownFunction(unknown_function))
    );
    assert_eq!(
        builder.create_block(foreign_function),
        Err(ControlError::ForeignFunction(foreign_function))
    );

    let block = builder.create_block(function)?;
    let foreign_block = BlockId::try_from_index(form(1), 0)?;
    let unknown_block = BlockId::try_from_index(owner, 8)?;
    assert_eq!(
        builder.append_operation(foreign_block, node(owner, 2)?),
        Err(ControlError::ForeignBlock(foreign_block))
    );
    assert_eq!(
        builder.append_operation(unknown_block, node(owner, 2)?),
        Err(ControlError::UnknownBlock(unknown_block))
    );
    assert_eq!(
        builder.append_operation(block, node(form(1), 2)?),
        Err(ControlError::ForeignNode(node(form(1), 2)?))
    );
    assert_eq!(
        builder.set_terminator(block, Terminator::Jump(foreign_block)),
        Err(ControlError::ForeignBlock(foreign_block))
    );
    assert_eq!(
        builder.set_terminator(
            block,
            Terminator::Conditional {
                condition: node(form(1), 3)?,
                if_true: block,
                if_false: block,
            }
        ),
        Err(ControlError::ForeignNode(node(form(1), 3)?))
    );
    assert_eq!(
        builder.set_terminator(
            block,
            Terminator::Return {
                values: vec![node(form(1), 4)?],
            }
        ),
        Err(ControlError::ForeignNode(node(form(1), 4)?))
    );
    assert_eq!(
        builder.set_terminator(
            block,
            Terminator::NonLocalExit {
                target: ExitTarget::Tag(foreign_block),
                values: Vec::new(),
                cleanups: Vec::new(),
            }
        ),
        Err(ControlError::ForeignBlock(foreign_block))
    );
    let foreign_cleanup = CleanupId::try_from_index(form(1), 0)?;
    assert_eq!(
        builder.set_terminator(
            block,
            Terminator::NonLocalExit {
                target: ExitTarget::Function(function),
                values: Vec::new(),
                cleanups: vec![foreign_cleanup],
            }
        ),
        Err(ControlError::ForeignCleanup(foreign_cleanup))
    );

    builder.set_terminator(block, Terminator::Return { values: Vec::new() })?;
    assert_eq!(
        builder.append_operation(block, node(owner, 5)?),
        Err(ControlError::BlockAlreadyTerminated(block))
    );
    assert_eq!(
        builder.set_terminator(block, Terminator::Unreachable),
        Err(ControlError::BlockAlreadyTerminated(block))
    );
    assert!(
        ControlError::UnknownBlock(unknown_block)
            .to_string()
            .starts_with("control-flow error: UnknownBlock")
    );
    Ok(())
}

#[test]
fn graph_finish_requires_entries_terminators_and_successors_in_the_same_function()
-> Result<(), Box<dyn Error>> {
    let owner = form(2);
    let first = node(owner, 0)?;
    let mut no_entry = ControlFlowBuilder::new(owner);
    no_entry.begin_function(first)?;
    assert!(matches!(
        no_entry.finish(),
        Err(ControlError::MissingEntry(function)) if function == first
    ));

    let mut unterminated = ControlFlowBuilder::new(owner);
    unterminated.begin_function(first)?;
    let entry = unterminated.create_block(first)?;
    assert!(matches!(
        unterminated.finish(),
        Err(ControlError::MissingTerminator(block)) if block == entry
    ));

    let second = node(owner, 1)?;
    let mut cross_function = ControlFlowBuilder::new(owner);
    cross_function.begin_function(first)?;
    let source = cross_function.create_block(first)?;
    cross_function.begin_function(second)?;
    let target = cross_function.create_block(second)?;
    cross_function.set_terminator(source, Terminator::Jump(target))?;
    cross_function.set_terminator(target, Terminator::Return { values: Vec::new() })?;
    assert!(matches!(
        cross_function.finish(),
        Err(ControlError::InvalidSuccessor { source: actual_source, target: actual_target })
            if actual_source == source && actual_target == target
    ));
    Ok(())
}

#[test]
fn completed_flow_exposes_block_contents_and_unreachable_dominators() -> Result<(), Box<dyn Error>>
{
    let owner = form(3);
    let function = node(owner, 0)?;
    let operation = node(owner, 1)?;
    let mut builder = ControlFlowBuilder::new(owner);
    builder.begin_function(function)?;
    let entry = builder.create_block(function)?;
    let dead = builder.create_block(function)?;
    builder.append_operation(entry, operation)?;
    builder.set_terminator(entry, Terminator::Return { values: Vec::new() })?;
    builder.set_terminator(dead, Terminator::Unreachable)?;
    let graph = builder.finish()?;

    let flow = graph
        .function(function)
        .ok_or("function flow should be present")?;
    assert_eq!(flow.function(), function);
    assert_eq!(flow.entry(), entry);
    assert_eq!(flow.blocks(), &[entry, dead]);
    assert_eq!(graph.functions().len(), 1);
    let entry_block = graph.block(entry).ok_or("entry block should be present")?;
    assert_eq!(entry_block.function(), function);
    assert_eq!(entry_block.operations(), &[operation]);
    assert_eq!(
        entry_block.terminator(),
        &Terminator::Return { values: Vec::new() }
    );
    assert_eq!(
        graph
            .block(dead)
            .map(ncl_hir::control::BasicBlock::function),
        Some(function)
    );
    let dominators = graph
        .dominators(function)
        .ok_or("dominator sets should be present")?;
    assert!(
        dominators
            .get(&entry)
            .is_some_and(|set| set.len() == 1 && set.contains(&entry))
    );
    assert!(
        dominators
            .get(&dead)
            .is_some_and(|set| set.len() == 1 && set.contains(&dead))
    );
    assert_eq!(graph.dominators(node(owner, 9)?), None);
    assert_eq!(
        ControlError::from(IdError::IndexOverflow { index: usize::MAX }),
        ControlError::Id(IdError::IndexOverflow { index: usize::MAX })
    );
    Ok(())
}
