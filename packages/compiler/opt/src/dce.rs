//! Conservative dead-code elimination for one SSA function.

use ncl_ir::{BlockId, Function, OpKind, Terminator, ValueId};
use std::collections::{HashSet, VecDeque};

use crate::{FunctionPass, Module, PassError, PassResult};

/// Removes unreachable blocks and pure operations whose results are unused.
#[derive(Clone, Debug, Default)]
pub struct DeadCodeElimination;

impl DeadCodeElimination {
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

    fn reachable_blocks(function: &Function) -> HashSet<BlockId> {
        let Some(entry) = function.blocks.first().map(|block| block.id) else {
            return HashSet::new();
        };
        let mut reachable = HashSet::from([entry]);
        let mut work = VecDeque::from([entry]);

        for region in &function.handler_regions {
            for block in region
                .protected
                .iter()
                .chain(std::iter::once(&region.handler))
                .chain(region.cleanup.iter())
            {
                if reachable.insert(*block) {
                    work.push_back(*block);
                }
            }
        }

        while let Some(id) = work.pop_front() {
            let Some(block) = function.blocks.iter().find(|block| block.id == id) else {
                continue;
            };
            let successors =
                Self::successors(&block.terminator)
                    .into_iter()
                    .chain(block.ops.iter().filter_map(|op| match op.kind {
                        OpKind::Prim {
                            condition: Some(target),
                            ..
                        } => Some(target),
                        _ => None,
                    }));
            for target in successors {
                if reachable.insert(target) {
                    work.push_back(target);
                }
            }
        }
        reachable
    }

    fn operands(kind: &OpKind) -> Vec<ValueId> {
        match kind {
            OpKind::Move { value }
            | OpKind::Load { address: value }
            | OpKind::Convert { value, .. } => vec![*value],
            OpKind::Store { address, value }
            | OpKind::StoreField {
                object: address,
                value,
                ..
            } => vec![*address, *value],
            OpKind::LoadField { object, .. } => vec![*object],
            OpKind::Call { function, args }
            | OpKind::CallIndirect {
                callee: function,
                args,
            } => std::iter::once(*function)
                .chain(args.iter().copied())
                .collect(),
            OpKind::Builtin { args, .. }
            | OpKind::SetMultipleValues { values: args }
            | OpKind::Prim { args, .. } => args.clone(),
            OpKind::MakeClosure { entry, captures } => std::iter::once(*entry)
                .chain(captures.iter().copied())
                .collect(),
            OpKind::CallClosure { closure, args } => std::iter::once(*closure)
                .chain(args.iter().copied())
                .collect(),
            OpKind::Compare { left, right, .. } => vec![*left, *right],
            OpKind::Const { .. }
            | OpKind::Alloc { .. }
            | OpKind::LoadArg { .. }
            | OpKind::Safepoint
            | OpKind::EnterHandler { .. }
            | OpKind::LeaveHandler { .. } => Vec::new(),
        }
    }

    fn terminator_operands(term: &Terminator) -> Vec<ValueId> {
        match term {
            Terminator::Jump { args, .. } => args.clone(),
            Terminator::Branch {
                condition,
                then_args,
                else_args,
                ..
            } => std::iter::once(*condition)
                .chain(then_args.iter().copied())
                .chain(else_args.iter().copied())
                .collect(),
            Terminator::Switch {
                value,
                cases,
                default_args,
                ..
            } => std::iter::once(*value)
                .chain(cases.iter().flat_map(|(_, _, args)| args.iter().copied()))
                .chain(default_args.iter().copied())
                .collect(),
            Terminator::CallReturn { function, args } | Terminator::TailCall { function, args } => {
                std::iter::once(*function)
                    .chain(args.iter().copied())
                    .collect()
            }
            Terminator::Return { values } => values.clone(),
            Terminator::Throw { condition } => vec![*condition],
            Terminator::Unreachable => Vec::new(),
        }
    }

    fn is_pure(kind: &OpKind) -> bool {
        match kind {
            OpKind::Const { .. }
            | OpKind::Move { .. }
            | OpKind::Load { .. }
            | OpKind::LoadField { .. }
            | OpKind::LoadArg { .. }
            | OpKind::Compare { .. }
            | OpKind::Convert { .. } => true,
            OpKind::Prim { op, condition, .. } => condition.is_none() && op.is_pure(),
            OpKind::Store { .. }
            | OpKind::StoreField { .. }
            | OpKind::Alloc { .. }
            | OpKind::Call { .. }
            | OpKind::CallIndirect { .. }
            | OpKind::MakeClosure { .. }
            | OpKind::CallClosure { .. }
            | OpKind::Builtin { .. }
            | OpKind::SetMultipleValues { .. }
            | OpKind::Safepoint
            | OpKind::EnterHandler { .. }
            | OpKind::LeaveHandler { .. } => false,
        }
    }

    fn eliminate(function: &mut Function, reachable: &HashSet<BlockId>) -> bool {
        let mut live = HashSet::new();
        for block in function
            .blocks
            .iter()
            .filter(|block| reachable.contains(&block.id))
        {
            live.extend(Self::terminator_operands(&block.terminator));
            for op in &block.ops {
                if !Self::is_pure(&op.kind) || op.results.is_empty() {
                    live.extend(Self::operands(&op.kind));
                }
            }
        }

        let mut changed = true;
        while changed {
            changed = false;
            for block in function
                .blocks
                .iter()
                .filter(|block| reachable.contains(&block.id))
            {
                for op in block.ops.iter().rev() {
                    if !Self::is_pure(&op.kind)
                        || op.results.iter().any(|(value, _)| live.contains(value))
                    {
                        for operand in Self::operands(&op.kind) {
                            changed |= live.insert(operand);
                        }
                    }
                }
            }
        }

        let mut did_change = false;
        for block in &mut function.blocks {
            if !reachable.contains(&block.id) {
                did_change = true;
                continue;
            }
            let old_len = block.ops.len();
            block.ops.retain(|op| {
                !Self::is_pure(&op.kind)
                    || op.results.is_empty()
                    || op.results.iter().any(|(value, _)| live.contains(value))
            });
            did_change |= block.ops.len() != old_len;
        }
        let old_len = function.blocks.len();
        function
            .blocks
            .retain(|block| reachable.contains(&block.id));
        did_change |= function.blocks.len() != old_len;
        did_change
    }
}

impl FunctionPass for DeadCodeElimination {
    fn name(&self) -> &'static str {
        "dead-code-elimination"
    }

    fn run(&mut self, function: &mut Function, _module: &Module) -> PassResult {
        ncl_ir::verify(function)
            .map_err(|errors| PassError::new(self.name(), format!("{errors:?}")))?;
        let reachable = Self::reachable_blocks(function);
        let changed = Self::eliminate(function, &reachable);
        ncl_ir::verify(function)
            .map_err(|errors| PassError::new(self.name(), format!("{errors:?}")))?;
        Ok(changed)
    }
}
