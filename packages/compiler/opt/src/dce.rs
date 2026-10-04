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
            | OpKind::Convert { value, .. }
            | OpKind::MakeValueCell { value } => vec![*value],
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
            OpKind::CallClosure {
                closure,
                args,
                named_symbol,
            } => std::iter::once(*closure)
                .chain(named_symbol.iter().copied())
                .chain(args.iter().copied())
                .collect(),
            OpKind::Compare { left, right, .. } => vec![*left, *right],
            OpKind::Const { .. }
            | OpKind::Alloc { .. }
            | OpKind::LoadArg { .. }
            | OpKind::LoadCapture { .. }
            | OpKind::LoadFunctionObject
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

    const fn is_pure(kind: &OpKind) -> bool {
        match kind {
            OpKind::Const { .. }
            | OpKind::Move { .. }
            | OpKind::Load { .. }
            | OpKind::LoadField { .. }
            | OpKind::LoadArg { .. }
            | OpKind::LoadCapture { .. }
            | OpKind::LoadFunctionObject
            | OpKind::Compare { .. }
            | OpKind::Convert { .. } => true,
            OpKind::Prim { op, condition, .. } => {
                condition.is_none()
                    && matches!(
                        op,
                        ncl_ir::Prim::FixnumLt
                            | ncl_ir::Prim::FixnumLe
                            | ncl_ir::Prim::FixnumEq
                            | ncl_ir::Prim::Eq
                            | ncl_ir::Prim::Eql
                            | ncl_ir::Prim::Typep
                            | ncl_ir::Prim::CharacterPredicate(_)
                    )
            }
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
            | OpKind::LeaveHandler { .. }
            | OpKind::MakeValueCell { .. } => false,
        }
    }

    fn eliminate(function: &mut Function, reachable: &HashSet<BlockId>) -> bool {
        let mut live = HashSet::new();
        for tag in function
            .handler_regions
            .iter()
            .filter_map(|region| region.catch_tag)
        {
            live.insert(tag);
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use ncl_ir::{BasicBlock, ConstantIndex, FunctionId, Op, Param, Prim, Ty};

    fn function(blocks: Vec<BasicBlock>) -> Function {
        Function {
            id: FunctionId(9),
            name: "dce-private".into(),
            params: vec![Param {
                name: "x".into(),
                ty: Ty::Word,
            }],
            return_types: vec![],
            blocks,
            locals: vec![],
            constants: vec![],
            handler_regions: vec![],
            debug: vec![],
        }
    }

    #[test]
    fn covers_reachability_operands_and_purity_shapes() {
        let terminal = [
            Terminator::CallReturn {
                function: ValueId(0),
                args: vec![ValueId(1)],
            },
            Terminator::TailCall {
                function: ValueId(0),
                args: vec![ValueId(1)],
            },
            Terminator::Return {
                values: vec![ValueId(1)],
            },
            Terminator::Throw {
                condition: ValueId(1),
            },
            Terminator::Unreachable,
        ];
        for term in terminal {
            assert!(DeadCodeElimination::successors(&term).is_empty());
            assert!(
                !DeadCodeElimination::terminator_operands(&term).is_empty()
                    || matches!(term, Terminator::Unreachable)
            );
        }
        assert!(DeadCodeElimination::is_pure(&OpKind::Prim {
            op: Prim::FixnumEq,
            args: vec![],
            condition: None
        }));
        assert!(!DeadCodeElimination::is_pure(&OpKind::Prim {
            op: Prim::Car,
            args: vec![],
            condition: None
        }));
        assert!(!DeadCodeElimination::is_pure(&OpKind::Prim {
            op: Prim::FixnumEq,
            args: vec![],
            condition: Some(BlockId(1))
        }));
        assert!(!DeadCodeElimination::is_pure(&OpKind::Store {
            address: ValueId(0),
            value: ValueId(1)
        }));
        assert_eq!(
            DeadCodeElimination::operands(&OpKind::Const {
                result: ConstantIndex(0)
            }),
            Vec::<ValueId>::new()
        );

        let blocks = vec![
            BasicBlock {
                id: BlockId(0),
                params: vec![],
                ops: vec![Op {
                    results: vec![(ValueId(2), Ty::Word)],
                    kind: OpKind::Move { value: ValueId(0) },
                    loc: None,
                }],
                terminator: Terminator::Jump {
                    target: BlockId(2),
                    args: vec![],
                },
            },
            BasicBlock {
                id: BlockId(1),
                params: vec![],
                ops: vec![],
                terminator: Terminator::Return { values: vec![] },
            },
            BasicBlock {
                id: BlockId(2),
                params: vec![],
                ops: vec![],
                terminator: Terminator::Return { values: vec![] },
            },
        ];
        let mut function = function(blocks);
        let reachable = DeadCodeElimination::reachable_blocks(&function);
        assert!(reachable.contains(&BlockId(0)));
        assert!(reachable.contains(&BlockId(2)));
        assert!(!reachable.contains(&BlockId(1)));
        assert!(DeadCodeElimination::eliminate(&mut function, &reachable));
        assert_eq!(function.blocks.len(), 2);
    }
}
