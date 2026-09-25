//! Structural, SSA, type, and safepoint checks for IR functions.
#![allow(missing_docs)]
#![allow(clippy::all)]
#![allow(clippy::too_many_lines)]

use crate::{BasicBlock, BlockId, Function, HandlerRegionId, Op, OpKind, Terminator, Ty, ValueId};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// A precise reason why an IR function is invalid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerifyError {
    DuplicateBlock(BlockId),
    MissingBlock(BlockId),
    MissingTerminator(BlockId),
    SuccessorArity(BlockId),
    SuccessorType(BlockId),
    UndefinedValue(ValueId),
    DuplicateValue(ValueId),
    ConstantOutOfBounds(BlockId),
    ReturnArity(BlockId),
    TypeMismatch(BlockId),
    HandlerTarget(BlockId),
    DuplicateHandlerRegion(HandlerRegionId),
    MissingHandlerRegion(HandlerRegionId),
    HandlerNesting(HandlerRegionId),
    HandlerMismatch(BlockId),
    HandlerUnbalanced(BlockId),
    /// A warning represented in the existing error channel for API compatibility.
    SafepointWarning(BlockId),
}
impl Display for VerifyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl Error for VerifyError {}

type Definition = (Ty, BlockId, usize);

/// Verifies block structure, SSA dominance, operation types, and safepoint placement.
///
/// `SafepointWarning` uses the existing error result channel so the public API remains
/// `Result<(), Vec<VerifyError>>`; callers should treat that variant as a warning.
///
/// # Errors
///
/// Returns every detected structural, SSA, type, and safepoint violation.
pub fn verify(function: &Function) -> Result<(), Vec<VerifyError>> {
    let mut errors = Vec::new();
    let mut blocks = HashMap::new();
    for block in &function.blocks {
        if blocks.insert(block.id, block).is_some() {
            errors.push(VerifyError::DuplicateBlock(block.id));
        }
    }
    let entry = function.blocks.first().map(|block| block.id);
    let mut definitions = HashMap::new();
    if let Some(entry) = entry {
        for (index, parameter) in function.params.iter().enumerate() {
            insert_definition(
                ValueId(u32::try_from(index).unwrap_or(u32::MAX)),
                (parameter.ty, entry, 0),
                &mut definitions,
                &mut errors,
            );
        }
    }
    for block in &function.blocks {
        for (index, parameter) in block.params.iter().enumerate() {
            insert_definition(
                parameter.value,
                (parameter.ty, block.id, index),
                &mut definitions,
                &mut errors,
            );
        }
        for (index, op) in block.ops.iter().enumerate() {
            for (result_index, (value, ty)) in op.results.iter().enumerate() {
                insert_definition(
                    *value,
                    (*ty, block.id, block.params.len() + index + result_index + 1),
                    &mut definitions,
                    &mut errors,
                );
            }
        }
    }
    let predecessors = predecessors(function, &blocks);
    let dominators = compute_dominators(function, &blocks, &predecessors);
    for block in &function.blocks {
        for (index, op) in block.ops.iter().enumerate() {
            verify_detail::check_op(
                op,
                block,
                index + block.params.len() + 1,
                function,
                &blocks,
                &definitions,
                &dominators,
                &mut errors,
            );
        }
        verify_detail::check_terminator(
            &block.terminator,
            block,
            function,
            &blocks,
            &definitions,
            &dominators,
            &mut errors,
        );
        if matches!(block.terminator, Terminator::Unreachable) {
            errors.push(VerifyError::MissingTerminator(block.id));
        }
        check_safepoints(block, &dominators, &mut errors);
    }
    for region in &function.handler_regions {
        for block in region
            .protected
            .iter()
            .chain(std::iter::once(&region.handler))
            .chain(region.cleanup.iter())
        {
            if !blocks.contains_key(block) {
                errors.push(VerifyError::HandlerTarget(*block));
            }
        }
        if let Some(tag) = region.catch_tag {
            if definitions.get(&tag).map(|(ty, _, _)| *ty) != Some(Ty::Word) {
                errors.push(VerifyError::TypeMismatch(region.handler));
            }
        }
        match region.kind {
            crate::HandlerKind::Catch => {
                if region.catch_tag.is_none()
                    || region.cleanup.is_some()
                    || !region.binding_targets.is_empty()
                {
                    errors.push(VerifyError::HandlerMismatch(region.handler));
                }
            }
            crate::HandlerKind::UnwindProtect => {
                if region.cleanup.is_none()
                    || region.catch_tag.is_some()
                    || !region.binding_targets.is_empty()
                {
                    errors.push(VerifyError::HandlerMismatch(region.handler));
                }
            }
            crate::HandlerKind::Progv => {
                if region.binding_targets.is_empty() || region.catch_tag.is_some() {
                    errors.push(VerifyError::HandlerMismatch(region.handler));
                }
            }
        }
        for target in &region.binding_targets {
            if definitions.get(target).map(|(ty, _, _)| *ty) != Some(Ty::Word) {
                errors.push(VerifyError::TypeMismatch(region.handler));
            }
        }
    }
    let mut regions = HashMap::new();
    for region in &function.handler_regions {
        if regions.insert(region.id, region).is_some() {
            errors.push(VerifyError::DuplicateHandlerRegion(region.id));
        }
    }
    for region in &function.handler_regions {
        if let Some(parent) = region.parent {
            let Some(parent_region) = regions.get(&parent) else {
                errors.push(VerifyError::MissingHandlerRegion(parent));
                continue;
            };
            let parent_blocks = parent_region
                .protected
                .iter()
                .copied()
                .collect::<HashSet<_>>();
            if region.depth <= parent_region.depth
                || region
                    .protected
                    .iter()
                    .any(|block| !parent_blocks.contains(block))
            {
                errors.push(VerifyError::HandlerNesting(region.id));
            }
        }
    }
    verify_handler_flow(function, &blocks, &regions, &mut errors);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn verify_handler_flow(
    function: &Function,
    blocks: &HashMap<BlockId, &BasicBlock>,
    regions: &HashMap<HandlerRegionId, &crate::HandlerRegion>,
    errors: &mut Vec<VerifyError>,
) {
    let Some(entry) = function.blocks.first().map(|block| block.id) else {
        return;
    };
    let mut incoming = HashMap::<BlockId, Option<Vec<HandlerRegionId>>>::new();
    incoming.insert(entry, Some(Vec::new()));
    let mut work = vec![entry];
    while let Some(id) = work.pop() {
        let Some(block) = blocks.get(&id) else {
            continue;
        };
        let Some(mut stack) = incoming.get(&id).cloned().flatten() else {
            continue;
        };
        for op in &block.ops {
            let region = match op.kind {
                OpKind::EnterHandler { region } => Some((true, region)),
                OpKind::LeaveHandler { region } => Some((false, region)),
                _ => None,
            };
            let Some((enter, region)) = region else {
                continue;
            };
            if !regions.contains_key(&region) {
                errors.push(VerifyError::MissingHandlerRegion(region));
                continue;
            }
            if enter {
                if let Some(parent) = regions[&region].parent {
                    if stack.last().copied() != Some(parent) {
                        errors.push(VerifyError::HandlerMismatch(block.id));
                    }
                }
                stack.push(region);
            } else if stack.pop() != Some(region) {
                errors.push(VerifyError::HandlerMismatch(block.id));
            }
        }
        let normal_exit = matches!(
            block.terminator,
            Terminator::Return { .. } | Terminator::CallReturn { .. } | Terminator::TailCall { .. }
        );
        if normal_exit && !stack.is_empty() {
            errors.push(VerifyError::HandlerUnbalanced(block.id));
        }
        for target in successors(&block.terminator) {
            if !blocks.contains_key(&target) {
                continue;
            }
            match incoming.get(&target) {
                None => {
                    incoming.insert(target, Some(stack.clone()));
                    work.push(target);
                }
                Some(Some(old)) if old != &stack => {
                    incoming.insert(target, None);
                    errors.push(VerifyError::HandlerUnbalanced(target));
                }
                _ => {}
            }
        }
    }
}

fn insert_definition(
    value: ValueId,
    definition: Definition,
    definitions: &mut HashMap<ValueId, Definition>,
    errors: &mut Vec<VerifyError>,
) {
    if definitions.contains_key(&value) {
        errors.push(VerifyError::DuplicateValue(value));
    } else {
        definitions.insert(value, definition);
    }
}

fn successors(term: &Terminator) -> Vec<BlockId> {
    match term {
        Terminator::Jump { target, .. } => vec![*target],
        Terminator::Branch {
            then_target,
            else_target,
            ..
        } => vec![*then_target, *else_target],
        Terminator::Switch { cases, default, .. } => cases
            .iter()
            .map(|(_, target, _)| *target)
            .chain([*default])
            .collect(),
        Terminator::CallReturn { .. }
        | Terminator::TailCall { .. }
        | Terminator::Return { .. }
        | Terminator::Throw { .. }
        | Terminator::Unreachable => Vec::new(),
    }
}

fn predecessors(
    function: &Function,
    blocks: &HashMap<BlockId, &BasicBlock>,
) -> HashMap<BlockId, Vec<BlockId>> {
    let mut result = blocks
        .keys()
        .map(|id| (*id, Vec::new()))
        .collect::<HashMap<_, _>>();
    for block in &function.blocks {
        for target in successors(&block.terminator) {
            if let Some(preds) = result.get_mut(&target) {
                preds.push(block.id);
            }
        }
    }
    result
}

fn compute_dominators(
    function: &Function,
    blocks: &HashMap<BlockId, &BasicBlock>,
    predecessors: &HashMap<BlockId, Vec<BlockId>>,
) -> HashMap<BlockId, HashSet<BlockId>> {
    let all = blocks.keys().copied().collect::<HashSet<_>>();
    let Some(entry) = function.blocks.first().map(|block| block.id) else {
        return HashMap::new();
    };
    let mut dominators = blocks
        .keys()
        .map(|id| {
            (
                *id,
                if *id == entry || predecessors.get(id).is_none_or(Vec::is_empty) {
                    HashSet::from([*id])
                } else {
                    all.clone()
                },
            )
        })
        .collect::<HashMap<_, _>>();
    loop {
        let mut changed = false;
        for id in blocks.keys().copied().filter(|id| *id != entry) {
            let mut incoming = predecessors
                .get(&id)
                .into_iter()
                .flatten()
                .filter_map(|pred| dominators.get(pred));
            let Some(first) = incoming.next() else {
                continue;
            };
            let mut next = first.clone();
            for set in incoming {
                next.retain(|candidate| set.contains(candidate));
            }
            next.insert(id);
            if dominators.get(&id) != Some(&next) {
                dominators.insert(id, next);
                changed = true;
            }
        }
        if !changed {
            return dominators;
        }
    }
}

fn use_value(
    value: ValueId,
    block: BlockId,
    position: usize,
    definitions: &HashMap<ValueId, Definition>,
    dominators: &HashMap<BlockId, HashSet<BlockId>>,
    errors: &mut Vec<VerifyError>,
) -> Option<Ty> {
    let Some((ty, definition_block, definition_position)) = definitions.get(&value) else {
        errors.push(VerifyError::UndefinedValue(value));
        return None;
    };
    if !dominators
        .get(&block)
        .is_some_and(|set| set.contains(definition_block))
        || (*definition_block == block && *definition_position >= position)
    {
        errors.push(VerifyError::UndefinedValue(value));
        return None;
    }
    Some(*ty)
}

fn require_type(actual: Option<Ty>, expected: Ty, block: BlockId, errors: &mut Vec<VerifyError>) {
    if actual != Some(expected) {
        errors.push(VerifyError::TypeMismatch(block));
    }
}
fn require_results(op: &Op, expected: &[Ty], block: BlockId, errors: &mut Vec<VerifyError>) {
    if op.results.len() != expected.len()
        || op
            .results
            .iter()
            .zip(expected)
            .any(|((_, actual), expected)| actual != expected)
    {
        errors.push(VerifyError::TypeMismatch(block));
    }
}
fn require_word_args(
    args: &[ValueId],
    block: &BasicBlock,
    position: usize,
    definitions: &HashMap<ValueId, Definition>,
    dominators: &HashMap<BlockId, HashSet<BlockId>>,
    errors: &mut Vec<VerifyError>,
) {
    for value in args {
        require_type(
            use_value(*value, block.id, position, definitions, dominators, errors),
            Ty::Word,
            block.id,
            errors,
        );
    }
}

#[path = "verify_detail.rs"]
mod verify_detail;

fn check_safepoints(
    block: &BasicBlock,
    dominators: &HashMap<BlockId, HashSet<BlockId>>,
    errors: &mut Vec<VerifyError>,
) {
    for (index, op) in block.ops.iter().enumerate() {
        if matches!(
            op.kind,
            OpKind::Call { .. }
                | OpKind::CallIndirect { .. }
                | OpKind::CallClosure { .. }
                | OpKind::Builtin { .. }
        ) && (index == 0 || !matches!(block.ops[index - 1].kind, OpKind::Safepoint))
        {
            errors.push(VerifyError::SafepointWarning(block.id));
        }
        if matches!(op.kind, OpKind::Alloc { .. })
            && (index + 1 == block.ops.len()
                || !matches!(block.ops[index + 1].kind, OpKind::Safepoint))
        {
            errors.push(VerifyError::SafepointWarning(block.id));
        }
    }
    if successors(&block.terminator).into_iter().any(|target| {
        dominators
            .get(&block.id)
            .is_some_and(|set| set.contains(&target))
    }) && !matches!(block.ops.last().map(|op| &op.kind), Some(OpKind::Safepoint))
    {
        errors.push(VerifyError::SafepointWarning(block.id));
    }
}
