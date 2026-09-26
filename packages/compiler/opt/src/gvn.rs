use ncl_ir::{BasicBlock, BlockId, Function, Op, OpKind, Terminator, ValueId};
use std::collections::{HashMap, HashSet};

use crate::{FunctionPass, Module, PassError, PassResult};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum Expression {
    Const(ncl_ir::ConstantIndex),
    Move(ValueId),
    Load(ValueId),
    LoadField(ValueId, u32),
    Prim(String, Vec<ValueId>, Option<BlockId>),
    Compare(String, ValueId, ValueId),
    Convert(String, ValueId),
}

#[derive(Clone, Debug, Default)]
struct Tables {
    expressions: HashMap<Expression, ValueId>,
    loads: HashMap<Expression, ValueId>,
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

    fn invalidate_loads(&mut self) {
        self.loads.clear();
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
                    if *id == entry || predecessors[id].is_empty() {
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
                let mut preds = predecessors[&id]
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
            let Some(parent) = dominators[&id]
                .iter()
                .filter(|candidate| **candidate != id)
                .filter(|candidate| {
                    dominators[&id]
                        .iter()
                        .filter(|other| **other != id && **other != **candidate)
                        .all(|other| !dominators[other].contains(candidate))
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
        mut tables: Tables,
    ) -> bool {
        let mut changed = false;
        let old_ops = std::mem::take(&mut block.ops);
        let mut new_ops = Vec::with_capacity(old_ops.len());
        for mut op in old_ops {
            Self::rewrite_op(&mut op, &tables);
            let key = Self::expression(&op);
            let load = Self::is_load(&op.kind);
            let pure = key.is_some();
            if Self::invalidates_memory(&op.kind) {
                tables.invalidate_loads();
            }
            if let Some(key) = key {
                let table = if load {
                    &mut tables.loads
                } else {
                    &mut tables.expressions
                };
                if op.results.len() == 1 {
                    if let Some(&existing) = table.get(&key) {
                        tables.replacements.insert(op.results[0].0, existing);
                        changed = true;
                        continue;
                    }
                    table.insert(key, op.results[0].0);
                }
            }
            if !pure && Self::invalidates_memory(&op.kind) {
                tables.invalidate_loads();
            }
            new_ops.push(op);
        }
        block.ops = new_ops;
        Self::rewrite_term(&mut block.terminator, &tables);
        for child in children.get(&block.id).into_iter().flatten().copied() {
            if let Some(mut child_block) = blocks.remove(&child) {
                changed |= Self::process_block(&mut child_block, children, blocks, tables.clone());
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
            | OpKind::Convert { value, .. } => v(value),
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
            OpKind::CallClosure { closure, args } => {
                v(closure);
                args.iter_mut().for_each(v);
            }
            OpKind::SetMultipleValues { values } => values.iter_mut().for_each(v),
            OpKind::Const { .. }
            | OpKind::Alloc { .. }
            | OpKind::LoadArg { .. }
            | OpKind::Safepoint
            | OpKind::EnterHandler { .. }
            | OpKind::LeaveHandler { .. } => {}
        }
    }

    fn expression(op: &Op) -> Option<Expression> {
        Some(match &op.kind {
            OpKind::Const { result } => Expression::Const(*result),
            OpKind::Move { value } => Expression::Move(*value),
            OpKind::Load { address } => Expression::Load(*address),
            OpKind::LoadField { object, field } => Expression::LoadField(*object, *field),
            OpKind::Prim {
                op,
                args,
                condition,
            } if op.is_pure() => Expression::Prim(format!("{op:?}"), args.clone(), *condition),
            OpKind::Compare { op, left, right } => {
                Expression::Compare(format!("{op:?}"), *left, *right)
            }
            OpKind::Convert { op, value } => Expression::Convert(format!("{op:?}"), *value),
            _ => return None,
        })
    }

    const fn is_load(kind: &OpKind) -> bool {
        matches!(kind, OpKind::Load { .. } | OpKind::LoadField { .. })
            || matches!(
                kind,
                OpKind::Prim { op, .. } if op.reads_memory()
            )
    }

    const fn invalidates_memory(kind: &OpKind) -> bool {
        match kind {
            OpKind::Prim { op, .. } => op.invalidates_memory(),
            OpKind::Store { .. }
            | OpKind::StoreField { .. }
            | OpKind::Call { .. }
            | OpKind::CallIndirect { .. }
            | OpKind::CallClosure { .. }
            | OpKind::Builtin { .. }
            | OpKind::MakeClosure { .. } => true,
            OpKind::Const { .. }
            | OpKind::Move { .. }
            | OpKind::Load { .. }
            | OpKind::LoadField { .. }
            | OpKind::Alloc { .. }
            | OpKind::LoadArg { .. }
            | OpKind::Compare { .. }
            | OpKind::Convert { .. }
            | OpKind::SetMultipleValues { .. }
            | OpKind::Safepoint
            | OpKind::EnterHandler { .. }
            | OpKind::LeaveHandler { .. } => false,
        }
    }
}

impl FunctionPass for GlobalValueNumbering {
    fn name(&self) -> &'static str {
        "global-value-numbering"
    }

    fn run(&mut self, function: &mut Function, _module: &Module) -> PassResult {
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
        let changed = Self::process_block(&mut entry_block, &tree, &mut blocks, Tables::default());
        function.blocks = std::iter::once(entry_block)
            .chain(blocks.into_values())
            .collect();
        ncl_ir::verify(function)
            .map_err(|errors| PassError::new(self.name(), format!("{errors:?}")))?;
        Ok(changed)
    }
}
