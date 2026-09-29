use super::{Constant, ConstantIndex, Function, Op, OpKind, Terminator, ValueId};
use std::collections::HashMap;

pub fn next_value(function: &Function) -> u32 {
    let max = function
        .blocks
        .iter()
        .flat_map(|b| {
            b.params
                .iter()
                .map(|p| p.value)
                .chain(b.ops.iter().flat_map(|o| o.results.iter().map(|(v, _)| *v)))
        })
        .map(|v| v.0)
        .max()
        .unwrap_or(0);
    max.saturating_add(1)
}

pub fn remap_kind(
    kind: &OpKind,
    values: &HashMap<ValueId, ValueId>,
    constants: &mut HashMap<ConstantIndex, ConstantIndex>,
    caller_constants: &mut Vec<Constant>,
    callee: &Function,
) -> OpKind {
    let v = |value: ValueId| values.get(&value).copied().unwrap_or(value);
    match kind {
        OpKind::Const { result } => {
            if usize::try_from(result.0)
                .ok()
                .and_then(|index| callee.constants.get(index))
                .is_none()
            {
                return kind.clone();
            }
            let mapped = remap_constant(*result, constants, caller_constants, callee);
            OpKind::Const { result: mapped }
        }
        OpKind::Move { value } => OpKind::Move { value: v(*value) },
        OpKind::Load { address } => OpKind::Load {
            address: v(*address),
        },
        OpKind::Store { address, value } => OpKind::Store {
            address: v(*address),
            value: v(*value),
        },
        OpKind::LoadField { object, field } => OpKind::LoadField {
            object: v(*object),
            field: *field,
        },
        OpKind::StoreField {
            object,
            field,
            value,
        } => OpKind::StoreField {
            object: v(*object),
            field: *field,
            value: v(*value),
        },
        OpKind::Alloc { words } => OpKind::Alloc { words: *words },
        OpKind::MakeValueCell { value } => OpKind::MakeValueCell { value: v(*value) },
        OpKind::LoadArg { index } => OpKind::LoadArg { index: *index },
        OpKind::LoadCapture { index } => OpKind::LoadCapture { index: *index },
        OpKind::Builtin { name, args } => OpKind::Builtin {
            name: name.clone(),
            args: args.iter().map(|x| v(*x)).collect(),
        },
        OpKind::Prim {
            op,
            args,
            condition,
        } => OpKind::Prim {
            op: op.clone(),
            args: args.iter().map(|x| v(*x)).collect(),
            condition: *condition,
        },
        OpKind::Compare { op, left, right } => OpKind::Compare {
            op: *op,
            left: v(*left),
            right: v(*right),
        },
        OpKind::Convert { op, value } => OpKind::Convert {
            op: *op,
            value: v(*value),
        },
        other => other.clone(),
    }
}

fn remap_constant(
    index: ConstantIndex,
    constants: &mut HashMap<ConstantIndex, ConstantIndex>,
    caller_constants: &mut Vec<Constant>,
    callee: &Function,
) -> ConstantIndex {
    if let Some(mapped) = constants.get(&index).copied() {
        return mapped;
    }
    let Some(constant) = usize::try_from(index.0)
        .ok()
        .and_then(|position| callee.constants.get(position))
        .cloned()
    else {
        return index;
    };
    #[allow(clippy::manual_unwrap_or, clippy::option_if_let_else)]
    let mapped_index = match u32::try_from(caller_constants.len()) {
        Ok(value) => value,
        Err(_) => u32::MAX,
    };
    let mapped = ConstantIndex(mapped_index);
    constants.insert(index, mapped);
    let slot = caller_constants.len();
    caller_constants.push(Constant::Nil);
    let remapped = match constant {
        Constant::Object(child) => {
            Constant::Object(remap_constant(child, constants, caller_constants, callee))
        }
        Constant::Structure { kind, elements } => Constant::Structure {
            kind,
            elements: elements
                .into_iter()
                .map(|child| remap_constant(child, constants, caller_constants, callee))
                .collect(),
        },
        other => other,
    };
    if let Some(entry) = caller_constants.get_mut(slot) {
        *entry = remapped;
    }
    mapped
}

pub fn remap_op_values(op: &mut Op, replacements: &HashMap<ValueId, ValueId>) {
    let v = |x: &mut ValueId| {
        if let Some(mapped) = replacements.get(x) {
            *x = *mapped;
        }
    };
    match &mut op.kind {
        OpKind::Move { value }
        | OpKind::Load { address: value }
        | OpKind::Convert { value, .. }
        | OpKind::MakeValueCell { value } => v(value),
        OpKind::Store { address, value }
        | OpKind::StoreField {
            object: address,
            value,
            ..
        } => {
            v(address);
            v(value);
        }
        OpKind::LoadField { object, .. } => v(object),
        OpKind::Call { function, args }
        | OpKind::CallIndirect {
            callee: function,
            args,
        } => {
            v(function);
            args.iter_mut().for_each(v);
        }
        OpKind::Builtin { args, .. }
        | OpKind::SetMultipleValues { values: args }
        | OpKind::Prim { args, .. } => args.iter_mut().for_each(v),
        OpKind::MakeClosure { entry, captures } => {
            v(entry);
            captures.iter_mut().for_each(v);
        }
        OpKind::CallClosure {
            closure,
            args,
            named_symbol,
        } => {
            v(closure);
            args.iter_mut().for_each(v);
            if let Some(symbol) = named_symbol {
                v(symbol);
            }
        }
        OpKind::Compare { left, right, .. } => {
            v(left);
            v(right);
        }
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

pub fn remap_term_values(term: &mut Terminator, replacements: &HashMap<ValueId, ValueId>) {
    let v = |x: &mut ValueId| {
        if let Some(mapped) = replacements.get(x) {
            *x = *mapped;
        }
    };
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
            for (_, _, args) in cases.iter_mut() {
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
