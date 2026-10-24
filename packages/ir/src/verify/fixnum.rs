use crate::{BlockId, Function, Ty, ValueId};
use std::collections::{HashMap, HashSet};

/// Returns Word values whose fixnum tag follows from their definitions.
///
/// The analysis ignores type declarations. It recognizes in-range fixnum
/// constants, moves, and block parameters whose incoming values are all
/// proven fixnums. Handler inputs without an SSA source remain unknown; catch
/// bindings inherit facts from their registered source values.
#[must_use]
pub fn proven_fixnum_values(function: &Function) -> HashSet<ValueId> {
    let mut i64_fixnums = HashSet::new();
    let mut incoming = HashMap::<(BlockId, usize), Vec<ValueId>>::new();
    let mut unknown_incoming = HashSet::<(BlockId, usize)>::new();
    for block in &function.blocks {
        for op in &block.ops {
            if let crate::OpKind::Const { result } = &op.kind
                && let Some(crate::Constant::Fixnum(value)) = usize::try_from(result.0)
                    .ok()
                    .and_then(|index| function.constants.get(index))
                && (i64::MIN / 2..=i64::MAX / 2).contains(value)
            {
                for (value, ty) in &op.results {
                    if *ty == Ty::I64 {
                        i64_fixnums.insert(*value);
                    }
                }
            }
        }
        let mut add_incoming = |target: BlockId, args: &[ValueId]| {
            for (index, value) in args.iter().copied().enumerate() {
                incoming.entry((target, index)).or_default().push(value);
            }
        };
        match &block.terminator {
            crate::Terminator::Jump { target, args } => add_incoming(*target, args),
            crate::Terminator::Branch {
                then_target,
                then_args,
                else_target,
                else_args,
                ..
            } => {
                add_incoming(*then_target, then_args);
                add_incoming(*else_target, else_args);
            }
            crate::Terminator::Switch {
                cases,
                default,
                default_args,
                ..
            } => {
                for (_, target, args) in cases {
                    add_incoming(*target, args);
                }
                add_incoming(*default, default_args);
            }
            crate::Terminator::CallReturn { .. }
            | crate::Terminator::TailCall { .. }
            | crate::Terminator::Return { .. }
            | crate::Terminator::Throw { .. }
            | crate::Terminator::Unreachable => {}
        }
    }
    for region in &function.handler_regions {
        let Some(handler) = function
            .blocks
            .iter()
            .find(|block| block.id == region.handler)
        else {
            continue;
        };
        match region.kind {
            crate::HandlerKind::Catch => {
                if !handler.params.is_empty() {
                    unknown_incoming.insert((handler.id, 0));
                }
                for index in 1..handler.params.len() {
                    if let Some(value) = region.binding_targets.get(index - 1) {
                        incoming
                            .entry((handler.id, index))
                            .or_default()
                            .push(*value);
                    } else {
                        unknown_incoming.insert((handler.id, index));
                    }
                }
            }
            crate::HandlerKind::UnwindProtect | crate::HandlerKind::Progv => {
                for index in 0..handler.params.len() {
                    unknown_incoming.insert((handler.id, index));
                }
            }
        }
    }

    loop {
        let before = i64_fixnums.len();
        for block in &function.blocks {
            for op in &block.ops {
                if let crate::OpKind::Move { value } = &op.kind
                    && i64_fixnums.contains(value)
                {
                    for (result, ty) in &op.results {
                        if *ty == Ty::I64 {
                            i64_fixnums.insert(*result);
                        }
                    }
                }
            }
        }
        if i64_fixnums.len() == before {
            break;
        }
    }

    let mut proven = HashSet::new();
    let mut conversions = Vec::new();
    for block in &function.blocks {
        for op in &block.ops {
            match &op.kind {
                crate::OpKind::Convert {
                    op: crate::Convert::I64ToWord,
                    value,
                } => conversions.extend(
                    op.results
                        .iter()
                        .filter(|(_, ty)| *ty == Ty::Word)
                        .map(|(result, _)| (*result, *value)),
                ),
                _ => {} // check-added-lines: allow(wildcard) only I64ToWord conversions can carry proven fixnum facts.
            }
        }
    }
    for (result, source) in conversions {
        if i64_fixnums.contains(&source) {
            proven.insert(result);
        }
    }

    loop {
        let before = proven.len();
        for block in &function.blocks {
            for op in &block.ops {
                if let crate::OpKind::Move { value } | crate::OpKind::TypeHint { value, .. } =
                    &op.kind
                    && proven.contains(value)
                {
                    for (result, ty) in &op.results {
                        if *ty == Ty::Word {
                            proven.insert(*result);
                        }
                    }
                }
            }
            for (index, parameter) in block.params.iter().enumerate() {
                if parameter.ty == Ty::Word
                    && !unknown_incoming.contains(&(block.id, index))
                    && incoming.get(&(block.id, index)).is_some_and(|values| {
                        !values.is_empty() && values.iter().all(|value| proven.contains(value))
                    })
                {
                    proven.insert(parameter.value);
                }
            }
        }
        if proven.len() == before {
            break;
        }
    }
    proven
}
