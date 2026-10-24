use ncl_ir::{BasicBlock, BlockId, Function, Op, OpKind, Terminator, Ty, ValueId};
use std::collections::{HashMap, HashSet};

use crate::{FunctionFacts, FunctionPass, Module, PassError, PassResult, is_scalar_value};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum Expression {
    Const(ncl_ir::ConstantIndex, String),
    Move(ValueId, String),
    Prim(String, Vec<ValueId>, String),
    Compare(String, ValueId, ValueId, String),
    Convert(String, ValueId, String),
}

#[derive(Clone, Debug, Default)]
struct Tables {
    expressions: HashMap<Expression, ValueId>,
    replacements: HashMap<ValueId, ValueId>,
}

impl Tables {
    fn resolve(&self, mut value: ValueId) -> ValueId {
        let mut seen = HashSet::new();
        while let Some(&next) = self.replacements.get(&value) {
            if !seen.insert(value) {
                break;
            }
            value = next;
        }
        value
    }
}

#[derive(Clone, Debug, Default)]
/// Eliminates redundant pure expressions while walking the dominator tree.
pub struct GlobalValueNumbering;

impl GlobalValueNumbering {
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

    fn dominator_tree(function: &Function) -> HashMap<BlockId, Vec<BlockId>> {
        let blocks = function
            .blocks
            .iter()
            .map(|block| block.id)
            .collect::<HashSet<_>>();
        let mut predecessors = blocks
            .iter()
            .map(|id| (*id, Vec::new()))
            .collect::<HashMap<_, _>>();
        for block in &function.blocks {
            for target in Self::successors(&block.terminator) {
                if let Some(preds) = predecessors.get_mut(&target) {
                    preds.push(block.id);
                }
            }
        }
        let Some(entry) = function.blocks.first().map(|block| block.id) else {
            return HashMap::new();
        };
        let mut dominators = blocks
            .iter()
            .map(|id| {
                (
                    *id,
                    if *id == entry || predecessors.get(id).is_none_or(Vec::is_empty) {
                        HashSet::from([*id])
                    } else {
                        blocks.clone()
                    },
                )
            })
            .collect::<HashMap<_, _>>();
        loop {
            let mut changed = false;
            for id in blocks.iter().copied().filter(|id| *id != entry) {
                let Some(predecessor_ids) = predecessors.get(&id) else {
                    continue;
                };
                let mut preds = predecessor_ids
                    .iter()
                    .filter_map(|pred| dominators.get(pred));
                let Some(first) = preds.next() else {
                    continue;
                };
                let mut next = first.clone();
                for set in preds {
                    next.retain(|candidate| set.contains(candidate));
                }
                next.insert(id);
                if dominators.get(&id) != Some(&next) {
                    dominators.insert(id, next);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        let mut tree = blocks
            .iter()
            .map(|id| (*id, Vec::new()))
            .collect::<HashMap<_, _>>();
        for id in blocks.iter().copied().filter(|id| *id != entry) {
            let Some(dominated) = dominators.get(&id) else {
                continue;
            };
            let Some(parent) = dominated
                .iter()
                .filter(|candidate| **candidate != id)
                .filter(|candidate| {
                    dominated
                        .iter()
                        .filter(|other| **other != id && **other != **candidate)
                        .all(|other| {
                            dominators
                                .get(other)
                                .is_none_or(|set| !set.contains(candidate))
                        })
                })
                .copied()
                .next()
            else {
                continue;
            };
            tree.entry(parent).or_default().push(id);
        }
        tree
    }

    fn rewrite_term(term: &mut Terminator, tables: &Tables) {
        let v = |value: &mut ValueId| *value = tables.resolve(*value);
        match term {
            Terminator::Jump { args, .. } => args.iter_mut().for_each(v),
            Terminator::Branch {
                condition,
                then_args,
                else_args,
                ..
            } => {
                v(condition);
                then_args.iter_mut().for_each(&v);
                else_args.iter_mut().for_each(v);
            }
            Terminator::Switch {
                value,
                cases,
                default_args,
                ..
            } => {
                v(value);
                for (_, _, args) in cases {
                    args.iter_mut().for_each(&v);
                }
                default_args.iter_mut().for_each(v);
            }
            Terminator::CallReturn { function, args } | Terminator::TailCall { function, args } => {
                v(function);
                args.iter_mut().for_each(v);
            }
            Terminator::Return { values } => values.iter_mut().for_each(v),
            Terminator::Throw { condition } => v(condition),
            Terminator::Unreachable => {}
        }
    }

    fn process_block(
        block: &mut BasicBlock,
        children: &HashMap<BlockId, Vec<BlockId>>,
        blocks: &mut HashMap<BlockId, BasicBlock>,
        types: &HashMap<ValueId, Ty>,
        facts: &FunctionFacts,
        mut tables: Tables,
    ) -> bool {
        let mut changed = false;
        let old_ops = std::mem::take(&mut block.ops);
        let mut new_ops = Vec::with_capacity(old_ops.len());
        for (index, mut op) in old_ops.into_iter().enumerate() {
            Self::rewrite_op(&mut op, &tables);
            let key = Self::expression(&op, block.id, index, types, facts);
            if let Some(key) = key {
                if let Some(&existing) = tables.expressions.get(&key) {
                    let Some(&(result, _)) = op.results.first() else {
                        continue;
                    };
                    tables.replacements.insert(result, existing);
                    changed = true;
                    continue;
                }
                let Some((result, _)) = op.results.first() else {
                    new_ops.push(op);
                    continue;
                };
                tables.expressions.insert(key, *result);
            }
            new_ops.push(op);
        }
        block.ops = new_ops;
        Self::rewrite_term(&mut block.terminator, &tables);
        for child in children.get(&block.id).into_iter().flatten().copied() {
            if let Some(mut child_block) = blocks.remove(&child) {
                changed |= Self::process_block(
                    &mut child_block,
                    children,
                    blocks,
                    types,
                    facts,
                    tables.clone(),
                );
                blocks.insert(child, child_block);
            }
        }
        changed
    }

    fn rewrite_op(op: &mut Op, tables: &Tables) {
        let v = |value: &mut ValueId| *value = tables.resolve(*value);
        match &mut op.kind {
            OpKind::Move { value }
            | OpKind::Load { address: value }
            | OpKind::Convert { value, .. }
            | OpKind::MakeValueCell { value }
            | OpKind::TypeHint { value, .. } => v(value),
            OpKind::LoadField { object, .. } => v(object),
            OpKind::Store { address, value }
            | OpKind::StoreField {
                object: address,
                value,
                ..
            } => {
                v(address);
                v(value);
            }
            OpKind::Prim { args, .. } | OpKind::Builtin { args, .. } => args.iter_mut().for_each(v),
            OpKind::Compare { left, right, .. } => {
                v(left);
                v(right);
            }
            OpKind::Call { function, args }
            | OpKind::CallIndirect {
                callee: function,
                args,
            } => {
                v(function);
                args.iter_mut().for_each(v);
            }
            OpKind::MakeClosure { entry, captures } => {
                v(entry);
                captures.iter_mut().for_each(v);
            }
            OpKind::CallClosure { closure, args, .. } => {
                v(closure);
                args.iter_mut().for_each(v);
            }
            OpKind::SetMultipleValues { values } => values.iter_mut().for_each(v),
            OpKind::Const { .. }
            | OpKind::Alloc { .. }
            | OpKind::LoadArg { .. }
            | OpKind::LoadCapture { .. }
            | OpKind::LoadFunctionObject
            | OpKind::Safepoint
            | OpKind::EnterHandler { .. }
            | OpKind::LeaveHandler { .. } => {}
        }
    }

    fn expression(
        op: &Op,
        block: BlockId,
        index: usize,
        types: &HashMap<ValueId, Ty>,
        facts: &FunctionFacts,
    ) -> Option<Expression> {
        let [(result, result_ty)] = op.results.as_slice() else {
            return None;
        };
        if facts
            .effects
            .get(&(block, index))
            .is_some_and(|effects| *effects != crate::EffectSet::default())
            || !is_scalar_value(types, facts, *result)
        {
            return None;
        }
        let operands_are_scalar = match &op.kind {
            OpKind::Const { .. } => true,
            OpKind::Convert {
                op: ncl_ir::Convert::AddressToWord | ncl_ir::Convert::WordToAddress,
                ..
            } => false,
            OpKind::Move { value } | OpKind::Convert { value, .. } => {
                is_scalar_value(types, facts, *value)
            }
            OpKind::Prim {
                op,
                args,
                condition: None,
            } if op.is_pure() => args
                .iter()
                .all(|value| is_scalar_value(types, facts, *value)),
            OpKind::Compare { left, right, .. } => {
                is_scalar_value(types, facts, *left) && is_scalar_value(types, facts, *right)
            }
            // check-added-lines: allow(wildcard) GVN accepts only the explicit scalar expression whitelist.
            _ => false,
        };
        if !operands_are_scalar {
            return None;
        }
        let result_ty = format!("{result_ty:?}");
        Some(match &op.kind {
            OpKind::Const { result } => Expression::Const(*result, result_ty),
            OpKind::Move { value } => Expression::Move(*value, result_ty),
            OpKind::Prim {
                op,
                args,
                condition: None,
            } if op.is_pure() => Expression::Prim(format!("{op:?}"), args.clone(), result_ty),
            OpKind::Compare { op, left, right } => {
                Expression::Compare(format!("{op:?}"), *left, *right, result_ty)
            }
            OpKind::Convert {
                op: ncl_ir::Convert::AddressToWord | ncl_ir::Convert::WordToAddress,
                ..
            } => return None,
            OpKind::Convert { op, value } => {
                Expression::Convert(format!("{op:?}"), *value, result_ty)
            }
            _ => return None,
        })
    }
}

impl FunctionPass for GlobalValueNumbering {
    fn name(&self) -> &'static str {
        "global-value-numbering"
    }

    fn run(&mut self, function: &mut Function, _module: &Module) -> PassResult {
        let block_order = function
            .blocks
            .iter()
            .map(|block| block.id)
            .collect::<Vec<_>>();
        let types = super::value_types(function);
        let facts = FunctionFacts::analyze(function);
        let tree = Self::dominator_tree(function);
        let Some(entry) = function.blocks.first().map(|block| block.id) else {
            ncl_ir::verify(function)
                .map_err(|errors| PassError::new(self.name(), format!("{errors:?}")))?;
            return Ok(false);
        };
        let mut blocks = function
            .blocks
            .drain(..)
            .map(|block| (block.id, block))
            .collect::<HashMap<_, _>>();
        let mut entry_block = blocks.remove(&entry).ok_or_else(|| {
            PassError::new(self.name(), "entry block disappeared while running GVN")
        })?;
        let changed = Self::process_block(
            &mut entry_block,
            &tree,
            &mut blocks,
            &types,
            &facts,
            Tables::default(),
        );
        let mut ordered_blocks = Vec::with_capacity(block_order.len());
        ordered_blocks.push(entry_block);
        for block_id in block_order.into_iter().skip(1) {
            let Some(block) = blocks.remove(&block_id) else {
                return Err(PassError::new(
                    self.name(),
                    "block disappeared while restoring function order",
                ));
            };
            ordered_blocks.push(block);
        }
        function.blocks = ordered_blocks;
        ncl_ir::verify(function)
            .map_err(|errors| PassError::new(self.name(), format!("{errors:?}")))?;
        Ok(changed)
    }
}
