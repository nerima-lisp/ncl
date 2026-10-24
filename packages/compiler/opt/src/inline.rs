use super::remap::{next_value, remap_kind, remap_op_values, remap_term_values};
use super::{
    EffectSet, Evidence, FunctionFacts, FunctionPass, Module, PassResult, ScalarFact, ScalarType,
    is_scalar_value, proven_scalar_word, value_types,
};
use ncl_ir::{Constant, ConstantIndex, Function, Op, OpKind, Terminator, Ty, ValueId};
use std::collections::HashMap;

/// The conservative first optimization: inline direct calls to small leaf functions.
#[derive(Clone, Debug)]
pub struct InlineDirectCalls {
    /// Maximum number of callee operations that may be copied.
    pub max_ops: usize,
}

impl Default for InlineDirectCalls {
    fn default() -> Self {
        Self { max_ops: 32 }
    }
}

impl InlineDirectCalls {
    fn seed_callee_facts(
        callee: &Function,
        args: &[ValueId],
        caller_types: &HashMap<ValueId, Ty>,
        caller_facts: &FunctionFacts,
    ) -> FunctionFacts {
        let mut facts = FunctionFacts::analyze(callee);
        for (index, (parameter, argument)) in callee.params.iter().zip(args).enumerate() {
            if parameter.ty != Ty::Word || caller_types.get(argument) != Some(&Ty::Word) {
                continue;
            }
            if let Some(
                fact @ ScalarFact::Known {
                    ty: ScalarType::Fixnum | ScalarType::Boolean,
                    evidence: Evidence::Proven,
                },
            ) = caller_facts.values.get(argument).copied()
                && let Ok(index) = u32::try_from(index)
            {
                facts.values.insert(ValueId(index), fact);
            }
        }

        if let Some(block) = callee.blocks.first() {
            for op in &block.ops {
                let source = match &op.kind {
                    OpKind::Move { value } | OpKind::TypeHint { value, .. } => *value,
                    // check-added-lines: allow(wildcard) Only these two operations preserve a scalar fact.
                    _ => continue,
                };
                let Some(fact) = facts.values.get(&source).copied() else {
                    continue;
                };
                if !proven_scalar_word(Some(&fact)) {
                    continue;
                }
                for (result, ty) in &op.results {
                    if *ty == Ty::Word {
                        facts.values.insert(*result, fact);
                    }
                }
            }
        }
        facts
    }

    fn scalar_candidate(
        caller_types: &HashMap<ValueId, Ty>,
        caller_facts: &FunctionFacts,
        call: &Op,
        callee: &Function,
    ) -> bool {
        let OpKind::Call { args, .. } = &call.kind else {
            return false;
        };
        let Some(block) = callee.blocks.first() else {
            return false;
        };
        if !callee.handler_regions.is_empty()
            || callee.blocks.len() != 1
            || !block.params.is_empty()
            || args.len() != callee.params.len()
            || call.results.len() != callee.return_types.len()
        {
            return false;
        }

        if args
            .iter()
            .zip(&callee.params)
            .any(|(argument, parameter)| {
                caller_types.get(argument) != Some(&parameter.ty)
                    || !is_scalar_value(caller_types, caller_facts, *argument)
            })
            || call
                .results
                .iter()
                .zip(&callee.return_types)
                .any(|((_, result_ty), return_ty)| result_ty != return_ty)
        {
            return false;
        }

        let facts = Self::seed_callee_facts(callee, args, caller_types, caller_facts);
        let types = value_types(callee);
        for (index, op) in block.ops.iter().enumerate() {
            if facts
                .effects
                .get(&(block.id, index))
                .is_some_and(|effects| *effects != EffectSet::default())
                || op.results.len() != 1
            {
                return false;
            }
            let operands = match &op.kind {
                OpKind::Const { .. } => &[],
                OpKind::Move { value } | OpKind::TypeHint { value, .. } => {
                    std::slice::from_ref(value)
                }
                OpKind::Prim {
                    op,
                    args,
                    condition: None,
                } if op.is_pure() => args,
                OpKind::Compare { left, right, .. } => {
                    if !is_scalar_value(&types, &facts, *left)
                        || !is_scalar_value(&types, &facts, *right)
                    {
                        return false;
                    }
                    &[]
                }
                OpKind::Convert {
                    op: ncl_ir::Convert::AddressToWord | ncl_ir::Convert::WordToAddress,
                    ..
                } => return false,
                OpKind::Convert { value, .. } => std::slice::from_ref(value),
                // check-added-lines: allow(wildcard) Inlining accepts only the explicit scalar operation whitelist.
                _ => return false,
            };
            let Some((result, _)) = op.results.first() else {
                return false;
            };
            if operands
                .iter()
                .any(|value| !is_scalar_value(&types, &facts, *value))
                || !is_scalar_value(&types, &facts, *result)
            {
                return false;
            }
        }

        let Terminator::Return { values } = &block.terminator else {
            return false;
        };
        values.len() == call.results.len()
            && values
                .iter()
                .zip(&callee.return_types)
                .all(|(value, return_ty)| {
                    types.get(value) == Some(return_ty) && is_scalar_value(&types, &facts, *value)
                })
    }

    fn callee_for<'a>(caller: &Function, call: &Op, module: &'a Module) -> Option<&'a Function> {
        let OpKind::Call { function, .. } = call.kind else {
            return None;
        };
        let entry = caller.blocks.iter().flat_map(|b| &b.ops).find_map(|op| {
            let [(result, _)] = op.results.as_slice() else {
                return None;
            };
            if *result != function {
                return None;
            }
            match &op.kind {
                OpKind::Const { result } => usize::try_from(result.0)
                    .ok()
                    .and_then(|index| caller.constants.get(index)),
                // check-added-lines: allow(wildcard) Only constant function entries name direct callees.
                _ => None,
            }
        })?;
        let Constant::FunctionEntry(id) = entry else {
            return None;
        };
        module
            .functions
            .iter()
            .find(|candidate| candidate.id == *id)
    }
}

impl FunctionPass for InlineDirectCalls {
    fn name(&self) -> &'static str {
        "inline-direct-calls"
    }

    fn run(&mut self, function: &mut Function, module: &Module) -> PassResult {
        let mut changed = false;
        let mut replacements = HashMap::new();
        let function_view = function.clone();
        let caller_types = value_types(&function_view);
        let caller_facts = FunctionFacts::analyze(&function_view);
        let mut next_value = next_value(function);
        let caller_constants = &mut function.constants;
        for block in &mut function.blocks {
            let old_ops = std::mem::take(&mut block.ops);
            let mut new_ops = Vec::with_capacity(old_ops.len());
            for call in old_ops {
                let Some(callee) = Self::callee_for(&function_view, &call, module) else {
                    new_ops.push(call);
                    continue;
                };
                let Some(callee_block) = callee.blocks.first() else {
                    new_ops.push(call);
                    continue;
                };
                if callee.id == function.id
                    || callee.blocks.len() != 1
                    || callee_block.ops.len() > self.max_ops
                    || !Self::scalar_candidate(&caller_types, &caller_facts, &call, callee)
                {
                    new_ops.push(call);
                    continue;
                }
                let Terminator::Return { values } = &callee_block.terminator else {
                    new_ops.push(call);
                    continue;
                };
                let OpKind::Call { args, .. } = &call.kind else {
                    new_ops.push(call);
                    continue;
                };
                if args.len() != callee.params.len() || values.len() != call.results.len() {
                    new_ops.push(call);
                    continue;
                }
                let Some(mut values_map) = callee
                    .params
                    .iter()
                    .enumerate()
                    .map(|(index, _)| {
                        let id = u32::try_from(index).ok()?;
                        let argument = args.get(index).copied()?;
                        Some((ValueId(id), argument))
                    })
                    .collect::<Option<HashMap<_, _>>>()
                else {
                    new_ops.push(call);
                    continue;
                };
                let mut constants = HashMap::<ConstantIndex, ConstantIndex>::new();
                for op in &callee_block.ops {
                    let kind = remap_kind(
                        &op.kind,
                        &values_map,
                        &mut constants,
                        caller_constants,
                        callee,
                    );
                    let mut results = Vec::with_capacity(op.results.len());
                    for (old, ty) in &op.results {
                        let fresh = ValueId(next_value);
                        next_value = next_value.saturating_add(1);
                        values_map.insert(*old, fresh);
                        results.push((fresh, *ty));
                    }
                    new_ops.push(Op {
                        results,
                        kind,
                        loc: op.loc,
                    });
                }
                let Some(mapped_returns) = values
                    .iter()
                    .map(|value| values_map.get(value).copied())
                    .collect::<Option<Vec<_>>>()
                else {
                    new_ops.push(call);
                    continue;
                };
                for ((result, _), mapped) in call.results.iter().zip(mapped_returns) {
                    replacements.insert(*result, mapped);
                }
                changed = true;
            }
            block.ops = new_ops;
        }
        if changed {
            for block in &mut function.blocks {
                for op in &mut block.ops {
                    remap_op_values(op, &replacements);
                }
                remap_term_values(&mut block.terminator, &replacements);
            }
        }
        Ok(changed)
    }
}
